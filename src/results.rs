use tokio::time::Duration;

pub struct WorkerResult {
    pub total_times: Vec<Duration>,
    pub request_times: Vec<Duration>,
}

impl WorkerResult {
    fn combine(mut self, other: Self) -> self {
        self.request_times.extend(other.total_times);
        self.total_times.extend(other.request_times);

        self
    }
}