use self::{
    usage::Usage,
    user_input::{Scheme, UserInput},
};
use crate::results::WorkerResult;
use futures_util::TryFutureExt;
use http::{
    header::{self, HeaderMap},
    Request,
};
use hyper::{
    client::conn::{self, SendRequest},
    Body,
};
use std::{net::SocketAddr, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
    task::JoinHandle,
    time::{error::Elapsed, sleep, timeout_at, Instant},
};
use tower::{util::ServiceExt, Service};

mod usage;
mod user_input;

pub type Handle = JoinHandle<anyhow::Result<WorkerResult>>;

/// The type of bench that is being ran.
#[derive(Clone, Copy, Debug)]
pub enum BenchType {
    /// Sets the http protocol to be used as h1
    HTTP1,

    /// Sets the http protocol to be used as h2
    HTTP2,
}

impl BenchType {
    fn is_http1(&self) -> bool {
        matches!(self, Self::HTTP1)
    }

    fn is_http2(&self) -> bool {
        matches!(self, Self::HTTP2)
    }
}

pub async fn start_tasks(
    time_for: Duration,
    connections: usize,
    uri_string: String,
    bench_type: BenchType,
    _predicted_size: usize,
) -> anyhow::Result<Vec<Handle>> {
    let deadline = Instant::now() + time_for;
    let user_input = UserInput::new(bench_type, uri_string).await?;

    let mut handles: Vec<Handle> = Vec::with_capacity(connections);

    for _ in 0..connections {
        let handle = tokio::spawn(benchmark(deadline, bench_type, user_input.clone()));

        handles.push(handle);
    }

    Ok(handles)
}

// Futures must not be awaited without timeout.
async fn benchmark(
    deadline: Instant,
    bench_type: BenchType,
    user_input: UserInput,
) -> anyhow::Result<WorkerResult> {
    let benchmark_start = Instant::now();
    let connector = RewrkConnector::new(
        deadline,
        bench_type,
        user_input.addr,
        user_input.scheme,
        user_input.host,
    );

    let mut send_request = match timeout_at(deadline, connector.connect()).await {
        Ok(result) => result?,
        Err(_elapsed) => return Ok(WorkerResult::default()),
    };

    let mut request_headers = HeaderMap::new();

    // Set "host" header for HTTP/1.
    if bench_type.is_http1() {
        request_headers.insert(header::HOST, user_input.host_header);
    }

    let mut request_times = Vec::new();

    // Benchmark loop.
    // Futures must not be awaited without timeout.
    loop {
        // Create request from **parsed** data.
        let mut request = Request::new(Body::empty());
        *request.uri_mut() = user_input.uri.clone();
        *request.headers_mut() = request_headers.clone();

        let future = send_request
            // Call poll_ready first.
            .ready()
            // Call the service.
            .and_then(|sr| sr.call(request))
            // Read response body completely.
            .and_then(|response| hyper::body::to_bytes(response.into_body()));

        let request_start = Instant::now();

        // Try to resolve future before benchmark deadline is elapsed.
        if let Ok(result) = timeout_at(deadline, future).await {
            // TODO: Log errors.
            if result.is_err() {
                send_request = match connector.try_connect_until().await {
                    Ok(v) => v,
                    Err(_elapsed) => break,
                };
            }
        } else {
            // Benchmark deadline is elapsed. Break the loop.
            break;
        }

        request_times.push(request_start.elapsed());
    }

    Ok(WorkerResult {
        total_times: vec![benchmark_start.elapsed()],
        request_times,
        buffer_sizes: vec![connector.get_received_bytes()],
    })
}

struct RewrkConnector {
    deadline: Instant,
    bench_type: BenchType,
    addr: SocketAddr,
    scheme: Scheme,
    host: String,
    usage: Usage,
}

impl RewrkConnector {
    fn new(
        deadline: Instant,
        bench_type: BenchType,
        addr: SocketAddr,
        scheme: Scheme,
        host: String,
    ) -> Self {
        let usage = Usage::new();

        Self {
            deadline,
            bench_type,
            addr,
            scheme,
            host,
            usage,
        }
    }

    async fn try_connect_until(&self) -> Result<SendRequest<Body>, Elapsed> {
        let future = async {
            loop {
                if let Ok(v) = self.connect().await {
                    return v;
                }

                sleep(Duration::from_millis(25)).await;
            }
        };

        timeout_at(self.deadline, future).await
    }

    async fn connect(&self) -> anyhow::Result<SendRequest<Body>> {
        let mut conn_builder = conn::Builder::new();

        if self.bench_type.is_http2() {
            conn_builder.http2_only(true);
        }

        let stream = TcpStream::connect(self.addr).await?;
        let stream = self.usage.wrap_stream(stream);

        let send_request = match self.scheme {
            Scheme::Http => handshake(conn_builder, stream).await?,
            Scheme::Https(ref tls_connector) => {
                let stream = tls_connector.connect(&self.host, stream).await?;
                handshake(conn_builder, stream).await?
            }
        };

        Ok(send_request)
    }

    fn get_received_bytes(&self) -> usize {
        self.usage.get_received_bytes()
    }
}

async fn handshake<S>(conn_builder: conn::Builder, stream: S) -> anyhow::Result<SendRequest<Body>>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (send_request, connection) = conn_builder.handshake(stream).await?;
    tokio::spawn(connection);
    Ok(send_request)
}
