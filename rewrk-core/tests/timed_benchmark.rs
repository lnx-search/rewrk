use std::time::{Duration, Instant};

use axum::routing::get;
use axum::Router;
use http::{Method, Uri};
use rewrk_core::{
    Batch,
    Bytes,
    HttpProtocol,
    Producer,
    ReWrkBenchmark,
    Request,
    RequestBatch,
    Sample,
    SampleCollector,
};

static ADDR: &str = "127.0.0.1:19999";

#[tokio::test]
async fn test_basic_benchmark() {
    let _ = tracing_subscriber::fmt::try_init();

    tokio::spawn(run_server());

    let uri = Uri::builder()
        .scheme("http")
        .authority(ADDR)
        .path_and_query("/")
        .build()
        .expect("Create URI");

    let mut benchmarker = ReWrkBenchmark::create(
        uri,
        1,
        HttpProtocol::HTTP1,
        TimedProducer::default(),
        BasicCollector::default(),
    )
    .await
    .expect("Create benchmark");
    benchmarker.set_sample_window(Duration::from_secs(30));
    benchmarker.set_num_workers(1);

    let start = Instant::now();
    benchmarker.run().await;
    assert!(start.elapsed() >= Duration::from_secs(10));

    let mut collector = benchmarker.consume_collector().await;
    let sample = collector.samples.remove(0);
    assert_eq!(sample.tag(), 0);
    dbg!(
        sample.latency().len(),
        sample.latency_min(),
        sample.latency_max(),
        sample.latency_stdev(),
    );
}

async fn run_server() {
    // build our application with a single route
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    axum::Server::bind(&ADDR.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Clone)]
pub struct TimedProducer {
    start: Instant,
}

impl Default for TimedProducer {
    fn default() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

#[rewrk_core::async_trait]
impl Producer for TimedProducer {
    fn for_worker(&mut self, _worker_id: usize) -> Self {
        self.clone()
    }

    fn ready(&mut self) {
        self.start = Instant::now();
    }

    async fn create_batch(&mut self) -> anyhow::Result<RequestBatch> {
        if self.start.elapsed() >= Duration::from_secs(10) {
            return Ok(RequestBatch::End);
        }

        let uri = Uri::builder().path_and_query("/").build()?;
        let requests = (0..500)
            .map(|_| {
                let req = http::Request::builder()
                    .method(Method::GET)
                    .uri(uri.clone())
                    .body(Bytes::new())?;
                Ok::<_, http::Error>(Request::new(0, req))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(RequestBatch::Batch(Batch { tag: 0, requests }))
    }
}

#[derive(Default)]
pub struct BasicCollector {
    samples: Vec<Sample>,
}

#[rewrk_core::async_trait]
impl SampleCollector for BasicCollector {
    async fn process_sample(&mut self, sample: Sample) -> anyhow::Result<()> {
        self.samples.push(sample);
        Ok(())
    }
}
