use tokio::time::Duration;

pub struct WorkerResult {
    pub total_time: Duration,
    pub request_times: Vec<Duration>,
}