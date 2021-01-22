use async_channel::{Receiver, Sender};

use std::time::Instant;

use tokio::time::Duration;
use tokio::task::JoinHandle;

use hyper::{Body, Request, StatusCode};
use hyper::Client;

use crate::results::WorkerResult;


/// A single http/1 connection worker
///
/// Builds a new http client with the http2_only option set either to false.
///
/// It then waits for the signaller to start sending pings to queue requests,
/// a client can take a request from the queue and then send the request,
/// these times are then measured and compared against previous latencies
/// to work out the min, max, total time and total requests of the given
/// worker which can then be sent back to the controller when the handle
/// is awaited.
///
/// todo Make concurrent handling for h2 tests
pub async fn client(
    waiter: Receiver<()>,
    host: String,
) -> Result<WorkerResult, String> {
    let session = Client::builder()
        .http2_only(true)
        .build_http();

    let mut max_latency: Duration = Duration::default();  // in seconds
    let mut min_latency: Duration = Duration::default();  // in seconds
    let mut has_been_set: bool = false;

    let mut total_requests: usize = 0;

    let start = Instant::now();
    while let Ok(_) = waiter.recv().await {
        let req = get_request(&host);

        let ts = Instant::now();

        let re = session.request(req).await;
        if let Err(e) = &re {
            println!("{:?}", e);
        };

        if let Ok(r) = re {
            assert_eq!(r.status(), StatusCode::OK);
        }

        let took = ts.elapsed();

        max_latency = max_latency.max(took);

        min_latency = if has_been_set {
            min_latency.min(took)
        } else {
            has_been_set = true;
            took
        };

        total_requests += 1;
    }
    let time_taken = start.elapsed();

    // Ok((max_latency, min_latency, total_requests, time_taken))

    Ok(WorkerResult{})
}


/// Constructs a new Request of a given host.
fn get_request(host: &str) -> Request<Body> {
    Request::builder()
        .uri(host)
        .header("Host", host)
        .method("GET")
        .body(Body::from(""))
        .expect("Failed to build request")
}