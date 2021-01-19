use tokio::runtime::{Builder, Runtime};


/// Creates and starts the runtime.
pub fn get_rt(workers: usize) -> Runtime {
    Builder::new_multi_thread()
        .enable_all()
        .worker_threads(workers)
        .build()
        .expect("Failed to build the runtime.")
}
