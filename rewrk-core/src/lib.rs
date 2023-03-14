//! # ReWrk Core
//!
//! HTTP benchmarking as a library made simple.
//!
//! ReWrk Core is a easily configurable and extendable framework for benchmarking
//! HTTP servers providing things like response validation, custom result collectors and
//! custom request producers.
//!
//! It measures some of the key metrics like latency, write IO and read IO and provides you
//! with a way of grouping results together with the concept of `tags`.
//!
//! ```
//! use axum::routing::get;
//! use axum::Router;
//! use anyhow::Result;
//! use http::{Method, Request, Uri};
//! use hyper::Body;
//! use rewrk_core::{
//!     Batch,
//!     HttpProtocol,
//!     Producer,
//!     ReWrkBenchmark,
//!     RequestBatch,
//!     Sample,
//!     SampleCollector,
//! };
//!
//! static ADDR: &str = "127.0.0.1:8080";
//!
//! #[tokio::test]
//! async fn test_basic_benchmark() -> Result<()> {
//!     tracing_subscriber::fmt::try_init()?;
//!
//!     tokio::spawn(run_server());
//!
//!     let uri = Uri::builder()
//!         .scheme("http")
//!         .authority(ADDR)
//!         .path_and_query("/")
//!         .build()?;
//!
//!     let mut benchmarker = ReWrkBenchmark::create(
//!         uri,
//!         1,
//!         HttpProtocol::HTTP1,
//!         BasicProducer::default(),
//!         BasicCollector::default(),
//!     )
//!     .await?;
//!     benchmarker.set_num_workers(1);
//!     benchmarker.run().await;
//!
//!     let mut collector = benchmarker.consume_collector().await;
//!     let sample = collector.samples.remove(0);
//!     assert_eq!(sample.tag(), 0);
//!     assert_eq!(sample.latency().len(), 1);
//!     assert_eq!(sample.read_transfer().len(), 1);
//!     assert_eq!(sample.write_transfer().len(), 1);
//! }
//!
//! async fn run_server() {
//!     // build our application with a single route
//!     let app = Router::new().route("/", get(|| async { "Hello, World!" }));
//!
//!     axum::Server::bind(&ADDR.parse().unwrap())
//!         .serve(app.into_make_service())
//!         .await
//!         .unwrap();
//! }
//!
//! #[derive(Default, Clone)]
//! pub struct BasicProducer {
//!     count: usize,
//! }
//!
//! #[rewrk_core::async_trait]
//! impl Producer for BasicProducer {
//!     fn ready(&mut self) {
//!         self.count = 1;
//!     }
//!
//!     async fn create_batch(&mut self) -> Result<RequestBatch> {
//!         if self.count > 0 {
//!             self.count -= 1;
//!
//!             let uri = Uri::builder().path_and_query("/").build()?;
//!             let request = Request::builder()
//!                 .method(Method::GET)
//!                 .uri(uri)
//!                 .body(Body::empty())?;
//!             Ok(RequestBatch::Batch(Batch {
//!                 tag: 0,
//!                 requests: vec![rewrk_core::Request::new(0, request)],
//!             }))
//!         } else {
//!             Ok(RequestBatch::End)
//!         }
//!     }
//! }
//!
//! #[derive(Default)]
//! pub struct BasicCollector {
//!     samples: Vec<Sample>,
//! }
//!
//! #[rewrk_core::async_trait]
//! impl SampleCollector for BasicCollector {
//!     async fn process_sample(&mut self, sample: Sample) -> Result<()> {
//!         self.samples.push(sample);
//!         Ok(())
//!     }
//! }
//! ```
//!

#[macro_use]
extern crate tracing;

mod connection;
mod producer;
mod recording;
mod runtime;
mod utils;
mod validator;

use std::fmt::{Debug, Formatter};

pub use async_trait::async_trait;
pub use http;

pub use self::connection::{HttpProtocol, Scheme};
pub use self::producer::{Batch, Producer, ProducerBatches, Request, RequestBatch};
pub use self::recording::{Sample, SampleCollector};
pub use self::runtime::{
    Error,
    ReWrkBenchmark,
    DEFAULT_WAIT_WARNING_THRESHOLD,
    DEFAULT_WINDOW_DURATION,
};
pub use self::utils::SampleMerger;
pub use self::validator::{DefaultValidator, ResponseValidator, ValidationError};

#[derive(Copy, Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
/// A unique ID for each request which is executed.
///
/// This is provided to allow the user to deterministically work out what
/// request produced by a producer is what when validating.
///
/// The system is deterministic, on a per-worker basis.
pub struct RequestKey(usize, usize);

impl Debug for RequestKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "RequestKey(worker_id={}, request_id={})", self.0, self.1)
    }
}

impl RequestKey {
    pub fn new(worker_id: usize, request_id: usize) -> Self {
        Self(worker_id, request_id)
    }

    #[inline]
    pub fn worker_id(&self) -> usize {
        self.0
    }

    #[inline]
    pub fn request_id(&self) -> usize {
        self.1
    }
}
