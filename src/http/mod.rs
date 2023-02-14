use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::anyhow;
use futures_util::stream::FuturesUnordered;
use futures_util::TryFutureExt;
use http::header::{self, HeaderMap};
use http::{Method, Request};
use hyper::body::Bytes;
use hyper::client::conn::{self, SendRequest};
use hyper::Body;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio::time::error::Elapsed;
use tokio::time::{sleep, timeout_at, Instant};
use tower::util::ServiceExt;
use tower::Service;

use self::usage::Usage;
use self::user_input::{Scheme, UserInput};
use crate::results::WorkerResult;
use crate::runtime::BenchmarkRuntime;

mod usage;
mod user_input;

type ReturnFuture = JoinHandle<anyhow::Result<WorkerResult>>;

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
    rt: &mut BenchmarkRuntime,
    time_for: Duration,
    connections: usize,
    uri_string: String,
    bench_type: BenchType,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
    _predicted_size: usize,
) -> anyhow::Result<FuturesUnordered<ReturnFuture>> {
    let deadline = Instant::now() + time_for;
    let user_input =
        UserInput::new(bench_type, uri_string, method, headers, body).await?;

    let handles = FuturesUnordered::new();

    for _ in 0..connections {
        let handle = rt.spawn(benchmark(deadline, bench_type, user_input.clone()));

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

    let (mut send_request, mut connection_task) =
        match timeout_at(deadline, connector.connect()).await {
            Ok(result) => result?,
            Err(_elapsed) => return Ok(WorkerResult::default()),
        };

    let mut request_headers = HeaderMap::new();

    // Set "host" header for HTTP/1.
    if bench_type.is_http1() {
        request_headers.insert(header::HOST, user_input.host_header);
    }

    request_headers.extend(user_input.headers);

    let mut request_times = Vec::new();
    let mut error_map = HashMap::new();

    // Benchmark loop.
    // Futures must not be awaited without timeout.
    loop {
        // Create request from **parsed** data.
        let mut request = Request::new(Body::from(user_input.body.clone()));
        *request.method_mut() = user_input.method.clone();
        *request.uri_mut() = user_input.uri.clone();
        *request.headers_mut() = request_headers.clone();

        let future = send_request
            // Call poll_ready first.
            .ready()
            // Call the service.
            .and_then(|sr| sr.call(request))
            // Read response body completely.
            .and_then(|response| hyper::body::to_bytes(response.into_body()));

        // ResponseFuture of send_request might return channel closed error instead of real error
        // in the case of connection_task being finished. This future will check if connection_task
        // is finished first.
        let future = async {
            tokio::select! {
                biased;
                result = (&mut connection_task) => {
                    match result.unwrap() {
                        Ok(()) => Err::<_, anyhow::Error>(anyhow!("connection closed")),
                        Err(e) => Err::<_, anyhow::Error>(anyhow::Error::new(e)),
                    }
                },
                result = future => result.map(|_| ()).map_err(Into::into),
            }
        };

        let request_start = Instant::now();

        // Try to resolve future before benchmark deadline is elapsed.
        if let Ok(result) = timeout_at(deadline, future).await {
            if let Err(e) = result {
                let error = e.to_string();

                // Insert/add error string to error log.
                match error_map.get_mut(&error) {
                    Some(count) => *count += 1,
                    None => {
                        error_map.insert(error, 1);
                    },
                }

                // Try reconnecting.
                match connector.try_connect_until().await {
                    Ok((sr, task)) => {
                        send_request = sr;
                        connection_task = task;
                    },
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
        error_map,
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

    async fn try_connect_until(
        &self,
    ) -> Result<(SendRequest<Body>, JoinHandle<hyper::Result<()>>), Elapsed> {
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

    async fn connect(
        &self,
    ) -> anyhow::Result<(SendRequest<Body>, JoinHandle<hyper::Result<()>>)> {
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
            },
        };

        Ok(send_request)
    }

    fn get_received_bytes(&self) -> usize {
        self.usage.get_received_bytes()
    }
}

async fn handshake<S>(
    conn_builder: conn::Builder,
    stream: S,
) -> anyhow::Result<(SendRequest<Body>, JoinHandle<hyper::Result<()>>)>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (send_request, connection) = conn_builder.handshake(stream).await?;
    let connection_task = tokio::spawn(connection);
    Ok((send_request, connection_task))
}
