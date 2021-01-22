use async_channel::{Receiver, Sender};

use std::time::Instant;

use tokio::time::Duration;
use tokio::task::JoinHandle;

use hyper::{Body, Request, StatusCode};
use hyper::Client;

use crate::proto::{h1, h2};
use crate::results::WorkerResult;

pub type Handle = JoinHandle<Result<WorkerResult, String>>;
pub type Handles = Vec<Handle>;


/// Creates n amount of workers that all listen and work steal off the same
/// async channel where n is the amount of concurrent connections wanted, all
/// handles and then the signal sender are returned.
///
/// Connections:
///     The amount of concurrent connections to spawn also known as the
///     worker pool size.
/// Host:
///     The host string / url for each worker to connect to when it gets a
///     signal to send a request.
/// Http2:
///     A bool to signal if the worker should use only HTTP/2 or HTTP/1.
pub async fn create_pool(
    connections: usize,
    host: String,
    http2: bool
) -> (Sender<()>, Handles) {

    let (tx, rx) = async_channel::bounded::<()>(connections * 2);

    let handles = if http2 {
        start_h2(connections, host, rx).await
    } else {
        start_h1(connections, host, rx).await
    };

    (tx, handles)
}


async fn start_h1(
    connections: usize,
    host: String,
    poller: Receiver<()>,
) -> Handles {
    let mut handles: Handles = Vec::with_capacity(connections);
    for _ in 0..connections {
        let handle: Handle = tokio::spawn(h1::client(
            poller.clone(),
            host.clone(),
        ));
        handles.push(handle);
    }

    handles
}


async fn start_h2(
    connections: usize,
    host: String,
    poller: Receiver<()>,
) -> Handles {
    let mut handles: Handles = Vec::with_capacity(connections);
    for _ in 0..connections {
        let handle: Handle = tokio::spawn(h2::client(
            poller.clone(),
            host.clone(),
        ));
        handles.push(handle);
    }

    handles
}

