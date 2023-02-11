use std::borrow::Cow;
use std::future::Future;
use std::mem;
use std::net::SocketAddr;
use std::sync::Arc;
use http::{Request, Response};
use http::response::Parts;

use hyper::client::conn;
use hyper::client::conn::SendRequest;
use hyper::Body;
use hyper::body::Bytes;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio::time::{timeout_at, Duration, Instant};

use crate::http::{HttpMode, Scheme};
use crate::recording::{Sample, SampleFactory};
use crate::{ResponseValidator, utils};
use crate::utils::IoUsageTracker;
use crate::validator::ValidationError;

/// The maximum number of attempts to try connect before aborting.
const RETRY_MAX_DEFAULT: usize = 3;

/// The initial HTTP connector for benchmarking.
pub struct ReWrkConnector {
    addr: SocketAddr,
    mode: HttpMode,
    scheme: Scheme,
    host: String,
    validator: Arc<dyn ResponseValidator>,
    retry_max: usize,

    sample_factory: SampleFactory,
}

impl ReWrkConnector {
    /// Create a new connector.
    pub fn new(
        addr: SocketAddr,
        mode: HttpMode,
        scheme: Scheme,
        host: impl Into<String>,
        sample_factory: SampleFactory,
        validator: Arc<dyn ResponseValidator>,
    ) -> Self {
        Self {
            addr,
            mode,
            scheme,
            sample_factory,
            validator,
            host: host.into(),
            retry_max: RETRY_MAX_DEFAULT,
        }
    }

    /// Set a new max retry attempt.
    pub fn set_retry_max(&mut self, max: usize) {
        self.retry_max = max;
    }

    /// Establish a new connection using the given connector.
    ///
    /// This will attempt to connect to the URI within the given duration.
    /// If the timeout elapses, `None` is returned.
    pub async fn connect_timeout(
        &self,
        dur: Duration,
    ) -> anyhow::Result<Option<ReWrkConnection>> {
        let deadline = Instant::now() + dur;
        let mut last_error: Option<anyhow::Error> = None;
        let mut attempts_left = self.retry_max;

        loop {
            let result = timeout_at(deadline, self.connect()).await;

            match result {
                Err(_) => {
                    return if let Some(error) = last_error {
                        Err(error)
                    } else {
                        Ok(None)
                    }
                },
                Ok(Err(e)) => {
                    if attempts_left == 0 {
                        return Err(e);
                    }

                    attempts_left -= 1;
                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                },
                Ok(Ok(connection)) => return Ok(Some(connection)),
            }
        }
    }

    /// Establish a new connection using the given connector.
    ///
    /// This method has no timeout and will block until the connection
    /// is established.
    pub async fn connect(&self) -> anyhow::Result<ReWrkConnection> {
        let mut conn_builder = conn::Builder::new();

        if self.mode.is_http2() {
            conn_builder.http2_only(true);
        }

        let stream = TcpStream::connect(self.addr).await?;

        let usage_tracker = IoUsageTracker::new();
        let stream = usage_tracker.wrap_stream(stream);

        let stream = match self.scheme {
            Scheme::Http => handshake(conn_builder, stream).await?,
            Scheme::Https(ref tls_connector) => {
                let stream = tls_connector.connect(&self.host, stream).await?;
                handshake(conn_builder, stream).await?
            },
        };

        Ok(ReWrkConnection::new(usage_tracker, stream, self.sample_factory.clone(), self.validator.clone()))
    }
}

/// An established HTTP connection for benchmarking.
pub struct ReWrkConnection {
    stream: HttpStream,

    io_tracker: IoUsageTracker,
    sample_factory: SampleFactory,
    sample: Sample,
    validator: Arc<dyn ResponseValidator>,

    last_send_sample: Instant,
    should_stop: bool,
}

impl ReWrkConnection {
    /// Creates a new live connection from an existing stream
    fn new(
        io_tracker: IoUsageTracker,
        stream: HttpStream,
        sample_factory: SampleFactory,
        validator: Arc<dyn ResponseValidator>,
    ) -> Self {
        let sample = sample_factory.new_sample();
        let last_send_sample = Instant::now();

        Self {
            io_tracker,
            stream,
            sample_factory,
            sample,
            validator,
            last_send_sample,
            should_stop: false,
        }
    }

    /// Send a HTTP request and record the relevant metrics
    pub async fn send(&mut self, request: Request<Body>) -> Result<(), hyper::Error> {
        let read_transfer_start = self.io_tracker.get_received_count();
        let write_transfer_start = self.io_tracker.get_written_count();
        let start = Instant::now();

        let (head, body) = match self.execute_req(request).await {
            Ok(resp) => resp,
            Err(e) => {
                if e.is_body_write_aborted()
                    || e.is_closed()
                    || e.is_connect()
                {
                    self.sample.record_error(ValidationError::ConnectionAborted);
                    self.should_stop = true;
                } else if e.is_incomplete_message()
                    || e.is_parse()
                    || e.is_parse_too_large()
                    || e.is_parse_status()
                {
                    self.sample.record_error(ValidationError::InvalidBody(Cow::Borrowed("invalid-http-body")));
                } else if e.is_timeout() {
                    self.sample.record_error(ValidationError::Timeout);
                } else {
                    return Err(e);
                }

                return Ok(())
            }
        };

        let elapsed_time = start.elapsed();
        let read_transfer_end = self.io_tracker.get_received_count();
        let write_transfer_end = self.io_tracker.get_written_count();

        if let Err(e) = self.validator.validate(head, body) {
            self.sample.record_error(e);
        } else {
            self.sample.record_latency(elapsed_time);
            self.sample.record_read_transfer(read_transfer_start, read_transfer_end, elapsed_time);
            self.sample.record_write_transfer(write_transfer_start, write_transfer_end, elapsed_time);
        }

        // Submit the sample if it's window interval has elapsed.
        if self.sample_factory.should_submit(self.last_send_sample) {
            let old_sample = mem::replace(&mut self.sample, self.sample_factory.new_sample());
            self.should_stop = self.sample_factory
                .submit_sample(old_sample)
                .is_err();
            self.last_send_sample = Instant::now();
        }

        Ok(())
    }

    async fn execute_req(&mut self, request: Request<Body>) -> Result<(Parts, Bytes), hyper::Error> {
        let resp = self.stream.send(request).await?;
        let (head, body) = resp.into_parts();
        let body = hyper::body::to_bytes(body).await?;
        Ok((head, body))
    }
}


/// Performs the HTTP handshake
async fn handshake<S>(
    conn_builder: conn::Builder,
    stream: S,
) -> Result<HttpStream, hyper::Error>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (send_request, connection) = conn_builder.handshake(stream).await?;
    let connection_task = tokio::spawn(connection);
    Ok(HttpStream {
        conn: send_request,
        waiter: connection_task,
    })
}

/// The established HTTP stream.
pub struct HttpStream {
    /// The live connection to send requests.
    conn: SendRequest<Body>,
    /// The hyper connection task handle.
    waiter: JoinHandle<hyper::Result<()>>,
}

impl HttpStream {
    pub fn send(&mut self, request: Request<Body>) -> impl Future<Output = Result<Response<Body>, hyper::Error>> {
        self.conn.send_request(request)
    }
}