use std::future::Future;
use std::sync::{mpsc, Arc};

use tokio::runtime::{self, Builder, Runtime};
use tokio::sync::Notify;
use tokio::task::JoinHandle;

/// Creates a default tokio runtime.
pub fn get_rt() -> Runtime {
    Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build the runtime.")
}

/// Create requested amount of multiple single threaded runtimes.
pub fn get_bench_rt(workers: usize) -> BenchmarkRuntime {
    BenchmarkRuntime::new(workers)
}

pub struct BenchmarkRuntime {
    handles: Vec<runtime::Handle>,
    shutdown: Arc<Notify>,
    cursor: usize,
}

impl BenchmarkRuntime {
    fn new(workers: usize) -> Self {
        let shutdown = Arc::new(Notify::new());
        let mut receivers = Vec::new();

        for _ in 0..workers {
            let shutdown = shutdown.clone();
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let runtime = Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build the runtime.");

                let handle = runtime.handle().clone();

                tx.send(handle).unwrap();

                runtime.block_on(shutdown.notified());
            });
            receivers.push(rx);
        }

        let mut handles = Vec::new();

        for receiver in receivers {
            let handle = receiver.recv().unwrap();

            handles.push(handle);
        }

        let cursor = 0;

        Self {
            handles,
            shutdown,
            cursor,
        }
    }

    pub fn spawn<F>(&mut self, future: F) -> JoinHandle<F::Output>
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let task = self.handles[self.cursor].spawn(future);

        self.cursor += 1;

        if self.cursor == self.handles.len() {
            self.cursor = 0;
        }

        task
    }
}

impl Drop for BenchmarkRuntime {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
    }
}
