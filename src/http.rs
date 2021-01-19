use async_channel::{Receiver, Sender};

use std::time::Instant;

use tokio::time::Duration;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

use hyper::{Body, Request, StatusCode};
use hyper::client::conn::{Builder, SendRequest};
use hyper::{Client, Uri};

pub type ClientResult = Result<(Duration, Duration, usize, Duration), String>;
pub type Handle = JoinHandle<ClientResult>;
pub type Handles = Vec<Handle>;


pub async fn create_pool(
    connections: usize,
    host: String,
    http2: bool
) -> (Sender<()>, Handles) {

    let (tx, rx) = async_channel::bounded::<()>(connections * 2);

    let mut handles: Handles = Vec::with_capacity(connections);
    for _ in 0..connections {
        let handle: Handle = tokio::spawn(client(
            rx.clone(),
            host.clone(),
            http2
        ));
        handles.push(handle);
    }

    (tx, handles)
}

async fn client(
    waiter: Receiver<()>,
    host: String,
    http2: bool,
) -> ClientResult {
    let mut session = Client::builder()
        .http2_only(http2)
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

    Ok((max_latency, min_latency, total_requests, time_taken))
}


fn get_request(host: &str) -> Request<Body> {
    Request::builder()
        .uri(host)
        .header("Host", host)
        .method("GET")
        .body(Body::from(""))
        .expect("Failed to build request")
}