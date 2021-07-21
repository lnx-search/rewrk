use crate::error::AnyError;
use crate::proto::tls;
use crate::proto::tcp_stream::CustomTcpStream;
use crate::results::WorkerResult;
use crate::utils::{get_http2_request, Scheme};

use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::task::JoinHandle;
use tokio::net::TcpStream;
use tokio::time::Duration;

use tokio_rustls::TlsConnector;
use tokio_rustls::webpki::{DNSName, DNSNameRef};

use hyper::client::conn;
use hyper::{Body, StatusCode, Uri};

use tower::{Service, ServiceExt};

enum SecurityProtocol {
    Tls(TlsConnector),
    Plain
}

struct ParsedUri {
    uri: Uri,
    host: String,
    host_dns: DNSName,
    port: u16
}

struct BenchmarkClient {
    protocol: SecurityProtocol,
    time_for: Duration,
    predicted_size: usize,
    parsed_uri: ParsedUri,
    counter: Arc<AtomicUsize>
}

impl BenchmarkClient {
    fn new(
        time_for: Duration,
        uri_string: String,
        predicted_size: usize
    ) -> Result<Self, AnyError> {
        let uri = Uri::from_str(&uri_string)?;

        let scheme = Scheme::from(uri.scheme_str());

        let protocol = match scheme {
            Scheme::HTTP => SecurityProtocol::Plain,
            Scheme::HTTPS => SecurityProtocol::Tls(
                tls::http2_alpn_connector()?
            )
        };

        let host = uri.host().ok_or("cant find host")?.to_owned();
        let host_dns = DNSNameRef::try_from_ascii_str(&host)?.to_owned();

        let port = match uri.port_u16() {
            Some(port) => port,
            None => scheme.default_port()
        };

        let parsed_uri = ParsedUri {
            uri,
            host,
            host_dns,
            port
        };

        let counter = Arc::new(AtomicUsize::new(0));

        Ok(Self {
            protocol,
            time_for,
            predicted_size,
            parsed_uri,
            counter
        })
    }

    async fn run(&self) -> Result<WorkerResult, AnyError> {
        let start = Instant::now();

        let (mut send_request, mut conn_handle) = self.connect_retry(start, self.time_for).await?;

        let mut times: Vec<Duration> = Vec::with_capacity(self.predicted_size);

        while self.time_for > start.elapsed() {
            tokio::select! {
                val = self.bench_request(&mut send_request, &mut times) => {
                    if let Err(_e) = val {
                        // Errors are ignored currently.
                    }
                },
                _ = (&mut conn_handle) => {
                    let (sr, handle) = self.connect_retry(start, self.time_for).await?;

                    send_request = sr;
                    conn_handle = handle;
                }
            };
        }

        let time_taken = start.elapsed();

        let result = WorkerResult {
            total_times: vec![time_taken],
            request_times: times,
            buffer_sizes: vec![self.counter.load(Ordering::Acquire)]
        };

        Ok(result)
    }

    // NOTE: Currently ignoring errors.
    async fn bench_request(
        &self,
        send_request: &mut conn::SendRequest<Body>,
        times: &mut Vec<Duration>
    ) -> Result<(), AnyError> {
        let req = get_http2_request(&self.parsed_uri.uri);

        let ts = Instant::now();

        if let Err(_) = send_request.ready().await {
            return Ok(());
        }

        let resp = match send_request.call(req).await {
            Ok(v) => v,
            Err(_) => return Ok(())
        };

        let took = ts.elapsed();

        let status = resp.status();
        assert_eq!(status, StatusCode::OK);

        let _buff = match hyper::body::to_bytes(resp).await {
            Ok(v) => v,
            Err(_) => return Ok(())
        };

        times.push(took);

        Ok(())
    }

    async fn connect_retry(
        &self,
        start: Instant,
        time_for: Duration
    ) -> Result<(conn::SendRequest<Body>, JoinHandle<()>), AnyError> {
        while start.elapsed() < time_for {
            let res = self.connect().await;

            match res {
                Ok(val) => return Ok(val),
                Err(_) => ()
            }
        }

        Err("connection closed".into())
    }

    async fn connect(&self) -> Result<(conn::SendRequest<Body>, JoinHandle<()>), AnyError> {
        let host_port = format!("{}:{}", self.parsed_uri.host, self.parsed_uri.port);

        let stream = TcpStream::connect(&host_port).await?;
        let stream = CustomTcpStream::new(stream, self.counter.clone());

        match &self.protocol {
            SecurityProtocol::Plain => {
                let (send_request, connection) = conn::Builder::new()
                    .http2_only(true)
                    .handshake(stream).await?;

                let handle = tokio::spawn(async move {
                    if let Err(_) = connection.await {}

                    // Connection died
                    // Should reconnect and log
                });

                Ok((send_request, handle))
            },
            SecurityProtocol::Tls(tls_connector) => {
                let stream = tls_connector.connect(self.parsed_uri.host_dns.as_ref(), stream).await?;

                let (send_request, connection) = conn::Builder::new()
                    .http2_only(true)
                    .handshake(stream).await?;

                let handle = tokio::spawn(async move {
                    if let Err(_) = connection.await {}

                    // Connection died
                    // Should reconnect and log
                });

                Ok((send_request, handle))
            }
        }
    }
}

pub async fn client(
    time_for: Duration,
    uri_string: String,
    predicted_size: usize,
) -> Result<WorkerResult, AnyError> {
    let benchmark_client = BenchmarkClient::new(time_for, uri_string, predicted_size)?;

    let result = benchmark_client.run().await?;

    Ok(result)
}
