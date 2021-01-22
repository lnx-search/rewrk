use async_channel::Receiver;

use std::time::Instant;

use tokio::time::Duration;

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
pub async fn client(
    waiter: Receiver<()>,
    host: String,
    predicted_size: usize,
) -> Result<WorkerResult, String> {
    let session = Client::builder()
        .http2_only(false)
        .build_http();

    let mut times: Vec<Duration> = Vec::with_capacity(predicted_size);

    let start = Instant::now();
    while let Ok(_) = waiter.recv().await {
        let req = get_request(&host);

        let ts = Instant::now();
        let re = session.request(req).await;
        let took = ts.elapsed();

        if let Err(e) = &re {
            println!("{:?}", e);
        } else if let Ok(r) = re {
            assert_eq!(r.status(), StatusCode::OK);
        }

        times.push(took);

    }
    let time_taken = start.elapsed();


    let result = WorkerResult{
        total_time: time_taken,
        request_times: times,
    };

    Ok(result)
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