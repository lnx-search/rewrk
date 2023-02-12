use axum::routing::get;
use axum::Router;
use http::{Method, Request, Uri};
use hyper::Body;
use rewrk_core::{
    Batch,
    HttpProtocol,
    Producer,
    ReWrkBenchmark,
    RequestBatch,
    Sample,
    SampleCollector,
};

static ADDR: &str = "127.0.0.1:200000";

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
        HttpProtocol::HTTP2,
        BasicProducer::default(),
        BasicCollector::default(),
    )
    .await
    .expect("Create benchmark");
    benchmarker.set_num_workers(1);
    benchmarker.run().await;

    let mut collector = benchmarker.consume_collector().await;
    let sample = collector.samples.remove(0);
    assert_eq!(sample.tag(), 0);
    assert_eq!(sample.latency().len(), 1);
    assert_eq!(sample.read_transfer().len(), 1);
    assert_eq!(sample.write_transfer().len(), 1);
}

async fn run_server() {
    // build our application with a single route
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    axum::Server::bind(&ADDR.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Default, Clone)]
pub struct BasicProducer {
    count: usize,
}

#[rewrk_core::async_trait]
impl Producer for BasicProducer {
    fn ready(&mut self) {
        self.count = 1;
    }

    async fn create_batch(&mut self) -> anyhow::Result<RequestBatch> {
        if self.count > 0 {
            self.count -= 1;

            let uri = Uri::builder().path_and_query("/").build()?;
            let request = Request::builder()
                .method(Method::GET)
                .uri(uri)
                .body(Body::empty())?;
            Ok(RequestBatch::Batch(Batch {
                tag: 0,
                requests: vec![request],
            }))
        } else {
            Ok(RequestBatch::End)
        }
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
