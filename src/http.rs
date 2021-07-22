use tokio::task::JoinHandle;

use crate::proto;
use crate::error::AnyError;
use crate::results::WorkerResult;
use tokio::time::Duration;

pub type Handle = JoinHandle<Result<WorkerResult, AnyError>>;

/// The type of bench that is being ran.
#[derive(Clone, Copy, Debug)]
pub enum BenchType {
    /// Sets the http protocol to be used as h1
    HTTP1,

    /// Sets the http protocol to be used as h2
    HTTP2,
}

pub async fn create_pool(
    time_for: Duration,
    connections: usize,
    uri_string: String,
    bench_type: BenchType,
    predicted_size: usize,
) -> Vec<Handle> {
    let client = proto::parse::get_client(
        time_for,
        uri_string,
        bench_type,
        predicted_size
    ).expect("bad uri");

    let mut handles: Vec<Handle> = Vec::with_capacity(connections);

    for _ in 0..connections {
        let handle: Handle = tokio::spawn(client.clone().start_instance());

        handles.push(handle);
    }

    handles
}
