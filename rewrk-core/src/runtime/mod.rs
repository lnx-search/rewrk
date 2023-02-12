mod worker;

use std::future::Future;
use std::io::ErrorKind;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::Duration;
use std::{cmp, io};

use http::{HeaderValue, Uri};
use tokio_native_tls::TlsConnector;

pub(crate) use self::worker::{spawn_workers, ShutdownHandle, WorkerConfig};
use crate::http::ReWrkConnector;
use crate::producer::Producer;
use crate::recording::CollectorActor;
use crate::{HttpProtocol, ResponseValidator, SampleCollector, Scheme};

pub const DEFAULT_WAIT_WARNING_THRESHOLD: f32 = 5.0;
pub const DEFAULT_WINDOW_DURATION: Duration = Duration::from_secs(10);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("The provided base URI is missing the required scheme (http, https)")]
    /// The base URI is missing the HTTP scheme.
    MissingScheme,
    #[error("The provided base URI with an invalid scheme expected 'http' or 'https' got {0:?}")]
    /// The base URI has a scheme which is not supported.
    InvalidScheme(String),
    #[error("The provided base URI is missing the required host")]
    /// The base URI is missing the server host.
    MissingHost,
    #[error("An error occurred while building the TLS config: {0}")]
    /// An error occurred while building the TLS config.
    TlsError(native_tls::Error),
    #[error("Failed to resolve the host socket address: {0}")]
    /// The system failed to resolve the socket address.
    AddressLookup(io::Error),
}

#[derive(Clone)]
pub struct ReWrkBenchmark<P>
where
    P: Producer + Clone,
{
    shutdown: ShutdownHandle,
    num_workers: usize,
    concurrency: usize,
    worker_config: WorkerConfig<P>,
}

impl<P> ReWrkBenchmark<P>
where
    P: Producer + Clone,
{
    /// Creates a new [ReWrkRuntime].
    ///
    /// This sets up the connector and collector actor.
    ///
    /// Once created benchmarks can be started by calling the `run` method.
    pub async fn create(
        base_uri: Uri,
        concurrency: usize,
        protocol: HttpProtocol,
        producer: P,
        validator: impl ResponseValidator,
        collector: impl SampleCollector,
    ) -> Result<Self, Error> {
        let connector = create_connector(base_uri, protocol)?;
        let collector = CollectorActor::spawn(collector).await;
        let shutdown = ShutdownHandle::default();
        let worker_config = WorkerConfig {
            connector,
            validator: Arc::new(validator),
            collector,
            producer,
            sample_window: DEFAULT_WINDOW_DURATION,
            producer_wait_warning_threshold: DEFAULT_WAIT_WARNING_THRESHOLD,
        };

        let num_workers = cmp::max(num_cpus::get() - 1, 1);

        Ok(Self {
            shutdown,
            num_workers,
            concurrency,
            worker_config,
        })
    }

    /// Run a benchmark.
    ///
    /// This returns a future which will complete once all
    /// workers for the benchmark have completed.
    pub fn run(&self) -> impl Future<Output = ()> {
        let waiter = spawn_workers(
            self.shutdown.clone(),
            self.num_workers,
            self.concurrency,
            self.worker_config.clone(),
        );

        async move {
            let _ = waiter.recv_async().await;
        }
    }

    /// Sets the shutdown flag for the running benchmark.
    pub fn shutdown(&self) {
        self.shutdown.set_abort();
    }

    /// Set the number of workers to spawn.
    pub fn set_num_workers(&mut self, n: usize) {
        self.num_workers = n;
    }

    /// Set the duration which should elapse before a sample
    /// is submitted to be processed in the collector.
    pub fn set_sample_window(&mut self, dur: Duration) {
        self.worker_config.sample_window = dur;
    }

    /// Set the percentage threshold that the system must be
    /// waiting on the producer in order for a warning to be raised.
    ///
    /// This is useful in situations where you know the producer will
    /// take more time than normal and want to silence the warning.
    pub fn set_producer_wait_warning_threshold(&mut self, pct: f32) {
        self.worker_config.producer_wait_warning_threshold = pct;
    }
}

/// Creates a new [ReWrkConnector] using a provided protocol and URI.
fn create_connector(uri: Uri, protocol: HttpProtocol) -> Result<ReWrkConnector, Error> {
    let scheme = uri.scheme_str().ok_or(Error::MissingScheme)?;
    let scheme = match scheme {
        "http" => Scheme::Http,
        "https" => {
            let mut builder = native_tls::TlsConnector::builder();

            builder
                .danger_accept_invalid_certs(true)
                .danger_accept_invalid_hostnames(true);

            match protocol {
                HttpProtocol::HTTP1 => builder.request_alpns(&["http/1.1"]),
                HttpProtocol::HTTP2 => builder.request_alpns(&["h2"]),
            };

            let cfg = builder.build().map_err(Error::TlsError)?;
            Scheme::Https(TlsConnector::from(cfg))
        },
        _ => return Err(Error::InvalidScheme(scheme.to_string())),
    };

    let authority = uri.authority().ok_or(Error::MissingHost)?;
    let host = authority.host();
    let port = authority
        .port_u16()
        .unwrap_or_else(|| scheme.default_port());

    // Prefer ipv4.
    let addr_iter = (host, port)
        .to_socket_addrs()
        .map_err(Error::AddressLookup)?;
    let mut last_addr = None;
    for addr in addr_iter {
        last_addr = Some(addr);
        if addr.is_ipv4() {
            break;
        }
    }
    let addr = last_addr.ok_or_else(|| {
        Error::AddressLookup(io::Error::new(
            ErrorKind::Other,
            "Failed to lookup hostname",
        ))
    })?;
    let host_header = HeaderValue::from_str(host).map_err(|_| Error::MissingHost)?;
    let host = host.to_string();

    let connector = ReWrkConnector::new(uri, host_header, addr, protocol, scheme, host);

    Ok(connector)
}
