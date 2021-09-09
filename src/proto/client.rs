use crate::error::AnyError;
use crate::proto::tcp_stream::CustomTcpStream;
use crate::proto::uri::ParsedUri;
use crate::proto::{Connect, Connection, HttpProtocol};
use crate::results::WorkerResult;
use crate::utils::BoxedFuture;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::sleep;

use hyper::client::conn;
use hyper::{Body, StatusCode};

use tower::{Service, ServiceExt};

pub trait Client {
    fn start_instance(self: Arc<Self>) -> BoxedFuture<'static, Result<WorkerResult, AnyError>>;
}

pub struct BenchmarkClient<C, P> {
    connector: C,
    protocol: P,
    time_for: Duration,
    predicted_size: usize,
    parsed_uri: ParsedUri,
}

impl<C, P> Client for BenchmarkClient<C, P>
where
    C: Connect + Send + Sync + 'static,
    P: HttpProtocol + Copy + Send + Sync + 'static,
{
    fn start_instance(self: Arc<Self>) -> BoxedFuture<'static, Result<WorkerResult, AnyError>> {
        Box::pin(self.start_ins())
    }
}

impl<C, P> BenchmarkClient<C, P>
where
    C: Connect + Send + Sync + 'static,
    P: HttpProtocol + Copy + Send + Sync + 'static,
{
    pub fn new(
        connector: C,
        protocol: P,
        time_for: Duration,
        predicted_size: usize,
        parsed_uri: ParsedUri,
    ) -> Self {
        Self {
            connector,
            protocol,
            time_for,
            predicted_size,
            parsed_uri,
        }
    }

    pub async fn start_ins(self: Arc<Self>) -> Result<WorkerResult, AnyError> {
        let start = Instant::now();
        let counter = Arc::new(AtomicUsize::new(0));

        let mut connection = match self.connect_retry(start, self.time_for, &counter).await {
            Ok(conn) => conn,
            Err(_) => {
                return Ok(WorkerResult::default());
            }
        };

        let mut times: Vec<Duration> = Vec::with_capacity(self.predicted_size);

        while self.time_for > start.elapsed() {
            tokio::select! {
                val = self.bench_request(&mut connection.send_request, &mut times) => {
                    if let Err(_e) = val {
                        // Errors are ignored currently.
                    }
                },
                _ = (&mut connection.handle) => {
                    match self.connect_retry(start, self.time_for, &counter).await {
                        Ok(conn) => connection = conn,
                        // Errors are ignored currently.
                        Err(_) => break,
                    }
                }
            };
        }

        let time_taken = start.elapsed();

        let result = WorkerResult {
            total_times: vec![time_taken],
            request_times: times,
            buffer_sizes: vec![counter.load(Ordering::Acquire)],
        };

        Ok(result)
    }

    // NOTE: Currently ignoring errors.
    async fn bench_request(
        &self,
        send_request: &mut conn::SendRequest<Body>,
        times: &mut Vec<Duration>,
    ) -> Result<(), AnyError> {
        let req = self.protocol.get_request(&self.parsed_uri.uri);

        let ts = Instant::now();

        if let Err(_) = send_request.ready().await {
            return Ok(());
        }

        let resp = match send_request.call(req).await {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let took = ts.elapsed();

        let status = resp.status();
        assert_eq!(status, StatusCode::OK);

        let _buff = match hyper::body::to_bytes(resp).await {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        times.push(took);

        Ok(())
    }

    async fn connect_retry(
        &self,
        start: Instant,
        time_for: Duration,
        counter: &Arc<AtomicUsize>,
    ) -> Result<Connection, AnyError> {
        while start.elapsed() < time_for {
            let res = self.connect(counter).await;

            match res {
                Ok(val) => return Ok(val),
                Err(_) => (),
            }

            sleep(Duration::from_millis(200)).await;
        }

        Err("connection closed".into())
    }

    async fn connect(&self, counter: &Arc<AtomicUsize>) -> Result<Connection, AnyError> {
        let stream = TcpStream::connect(&self.parsed_uri.addr).await?;

        let stream = CustomTcpStream::new(stream, counter.clone());

        let connection = self.connector.handshake(stream, self.protocol).await?;

        Ok(connection)
    }
}
