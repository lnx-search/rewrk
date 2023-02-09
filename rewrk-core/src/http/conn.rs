use std::future::Future;
use std::net::SocketAddr;
use hdrhistogram::Histogram;
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
use crate::utils;

/// The maximum number of attempts to try connect before aborting.
const RETRY_MAX_DEFAULT: usize = 3;

/// The initial HTTP connector for benchmarking.
pub struct ReWrkConnector {
    addr: SocketAddr,
    mode: HttpMode,
    scheme: Scheme,
    host: String,
    retry_max: usize,

    io_tracker: utils::UsageTracker,
}

impl ReWrkConnector {
    /// Create a new connector.
    pub fn new(
        addr: SocketAddr,
        mode: HttpMode,
        scheme: Scheme,
        host: impl Into<String>,
    ) -> Self {
        Self {
            addr,
            mode,
            scheme,
            host: host.into(),
            retry_max: RETRY_MAX_DEFAULT,
            io_tracker: utils::UsageTracker::new(),
        }
    }

    /// Set a new max retry attempt.
    pub fn set_retry_max(&mut self, max: usize) {
        self.retry_max = max;
    }

    /// Gets the current total number of bytes sent across the connection.
    pub fn total_bytes_sent(&self) -> u64 {
        self.io_tracker.get_count()
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

        Ok(ReWrkConnection {
            mode: self.mode,
            stream,
        })
    }
}

/// An established HTTP connection for benchmarking.
pub struct ReWrkConnection {
    mode: HttpMode,
    stream: HttpStream,

    latency_hist: Histogram<u32>,
    transfer_hist: Histogram<u32>,
}

impl ReWrkConnection {
    pub fn new(mode: HttpMode, stream: HttpStream) -> Self {
        Self {
            mode,
            stream,

            latency_hist: Histogram::new(600_000_000)
        }
    }

    pub async fn send(&mut self, request: Request<Body>) -> Result<(), hyper::Error> {
        let start = Instant::now();
        let mut resp = self.stream.send(request).await?;



        let data_stream = resp.body_mut();
        while let Some(res) = data_stream.data().await {
            res?;
        }

        let elapsed_time = start.elapsed();

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