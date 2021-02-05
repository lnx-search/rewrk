use async_channel::Sender;

use tokio::task::JoinHandle;

use crate::proto::{h1, h2};
use crate::results::WorkerResult;

pub type Handle = JoinHandle<Result<WorkerResult, String>>;
pub type Handles = Vec<Handle>;


/// The type of bench that is being ran.
#[derive(Clone, Copy, Debug)]
pub enum BenchType {
    /// Sets the http protocol to be used as h1
    HTTP1,

    /// Sets the http protocol to be used as h2
    HTTP2,

}


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
    bench_type: BenchType,
    predicted_size: usize,
) -> (Sender<()>, Handles) {
    let (tx, rx) = async_channel::bounded::<()>(connections * 2);


    let mut handles: Handles = Vec::with_capacity(connections);
    for _ in 0..connections {
        match bench_type {
            BenchType::HTTP1 => {
                let handle: Handle = tokio::spawn(h1::client(
                    rx.clone(),
                    host.clone(),
                    predicted_size,
                ));
                handles.push(handle);
            },
            BenchType::HTTP2 => {
                let handle: Handle = tokio::spawn(h2::client(
                    rx.clone(),
                    host.clone(),
                    predicted_size,
                ));
                handles.push(handle);
            },
        };
    }

    (tx, handles)
}

