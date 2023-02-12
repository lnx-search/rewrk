use std::future::Future;
use std::net::SocketAddr;

use http::response::Parts;
use http::{header, HeaderValue, Request, Response, Uri};
use hyper::body::Bytes;
use hyper::client::conn;
use hyper::client::conn::SendRequest;
use hyper::Body;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio::time::{timeout_at, Duration, Instant};

use crate::connection::{HttpProtocol, Scheme};
use crate::utils::IoUsageTracker;

/// The maximum number of attempts to try connect before aborting.
const RETRY_MAX_DEFAULT: usize = 3;

#[derive(Clone)]
/// The initial HTTP connector for benchmarking.
pub struct ReWrkConnector {
    uri: Uri,
    host_header: HeaderValue,
    addr: SocketAddr,
    protocol: HttpProtocol,
    scheme: Scheme,
    host: String,
    retry_max: usize,
}

impl ReWrkConnector {
    /// Create a new connector.
    pub fn new(
        uri: Uri,
        host_header: HeaderValue,
        addr: SocketAddr,
        protocol: HttpProtocol,
        scheme: Scheme,
        host: impl Into<String>,
    ) -> Self {
        Self {
            uri,
            host_header,
            addr,
            protocol,
            scheme,
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

        if self.protocol.is_http2() {
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

        Ok(ReWrkConnection::new(
            self.uri.clone(),
            self.host_header.clone(),
            stream,
            usage_tracker,
        ))
    }
}

/// An established HTTP connection for benchmarking.
pub struct ReWrkConnection {
    uri: Uri,
    host_header: HeaderValue,
    stream: HttpStream,
    io_tracker: IoUsageTracker,
}

impl ReWrkConnection {
    #[inline]
    /// Creates a new live connection from an existing stream
    fn new(
        uri: Uri,
        host_header: HeaderValue,
        stream: HttpStream,
        io_tracker: IoUsageTracker,
    ) -> Self {
        Self {
            uri,
            host_header,
            stream,
            io_tracker,
        }
    }

    #[inline]
    pub(crate) fn usage(&self) -> &IoUsageTracker {
        &self.io_tracker
    }

    #[inline]
    /// Executes a request.
    ///
    /// This will override the request host, scheme, port and host headers.
    pub(crate) async fn execute_req(
        &mut self,
        mut request: Request<Body>,
    ) -> Result<(Parts, Bytes), hyper::Error> {
        let request_uri = request.uri();
        let mut builder = Uri::builder()
            .scheme(self.uri.scheme().unwrap().clone())
            .authority(self.uri.authority().unwrap().clone());
        if let Some(path) = request_uri.path_and_query() {
            builder = builder.path_and_query(path.clone());
        }
        (*request.uri_mut()) = builder.build().unwrap();
        request
            .headers_mut()
            .insert(header::HOST, self.host_header.clone());

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
    pub fn send(
        &mut self,
        request: Request<Body>,
    ) -> impl Future<Output = Result<Response<Body>, hyper::Error>> {
        self.conn.send_request(request)
    }
}

impl Drop for HttpStream {
    fn drop(&mut self) {
        self.waiter.abort();
    }
}