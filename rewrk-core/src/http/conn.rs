use std::future::Future;
use std::net::SocketAddr;
use http::{Request, Response};

use hyper::client::conn;
use hyper::client::conn::SendRequest;
use hyper::Body;
use hyper::body::HttpBody;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio::time::{timeout_at, Duration, Instant};

use crate::http::{HttpMode, Scheme};
use crate::recording::{Sample, SampleFactory};
use crate::utils;
use crate::utils::IoUsageTracker;

/// The maximum number of attempts to try connect before aborting.
const RETRY_MAX_DEFAULT: usize = 3;

/// The initial HTTP connector for benchmarking.
pub struct ReWrkConnector {
    addr: SocketAddr,
    mode: HttpMode,
    scheme: Scheme,
    host: String,
    retry_max: usize,

    sample_factory: SampleFactory,
    io_tracker: IoUsageTracker,
}

impl ReWrkConnector {
    /// Create a new connector.
    pub fn new(
        addr: SocketAddr,
        mode: HttpMode,
        scheme: Scheme,
        host: impl Into<String>,
        sample_factory: SampleFactory,
    ) -> Self {
        Self {
            addr,
            mode,
            scheme,
            sample_factory,
            host: host.into(),
            retry_max: RETRY_MAX_DEFAULT,
            io_tracker: utils::IoUsageTracker::new(),
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
        let stream = self.io_tracker.wrap_stream(stream);

        let stream = match self.scheme {
            Scheme::Http => handshake(conn_builder, stream).await?,
            Scheme::Https(ref tls_connector) => {
                let stream = tls_connector.connect(&self.host, stream).await?;
                handshake(conn_builder, stream).await?
            },
        };

        Ok(ReWrkConnection::new(self.io_tracker.clone(), stream, self.sample_factory.clone()))
    }
}

/// An established HTTP connection for benchmarking.
pub struct ReWrkConnection {
    stream: HttpStream,

    io_tracker: IoUsageTracker,
    sample_factory: SampleFactory,
    sample: Sample,
}

impl ReWrkConnection {
    /// Creates a new live connection from an existing stream
    fn new(
        io_tracker: IoUsageTracker,
        stream: HttpStream,
        sample_factory: SampleFactory,
    ) -> Self {
        let sample = sample_factory.new_sample();

        Self {
            io_tracker,
            stream,
            sample_factory,
            sample,
        }
    }

    /// Send a HTTP request and record the relevant metrics
    pub async fn send(&mut self, request: Request<Body>) -> Result<(), hyper::Error> {
        let read_transfer_start = self.io_tracker.get_received_count();
        let write_transfer_start = self.io_tracker.get_written_count();
        let start = Instant::now();

        let mut resp = self.stream.send(request).await?;
        let data_stream = resp.body_mut();
        while let Some(res) = data_stream.data().await {
            res?;
        }

        let elapsed_time = start.elapsed();
        self.sample.record_latency(elapsed_time);

        let read_transfer_end = self.io_tracker.get_received_count();
        let write_transfer_end = self.io_tracker.get_written_count();

        self.sample.record_read_transfer(read_transfer_start, read_transfer_end, elapsed_time);
        self.sample.record_write_transfer(write_transfer_start, write_transfer_end, elapsed_time);

        Ok(())
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