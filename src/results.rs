#![allow(unused)]

use tokio::time::Duration;


/// Contains and handles results from the workers
pub struct WorkerResult {
    /// The total time taken for each worker.
    pub total_times: Vec<Duration>,

    /// The vec of latencies per request stored.
    pub request_times: Vec<Duration>,
}

impl WorkerResult {
    /// Creates a empty result, useful for merging results into one
    /// consumer.
    pub fn default() -> Self {
        Self {
            total_times: vec![],
            request_times: vec![]
        }
    }

    /// Consumes both self and other producing a combined result.
    pub fn combine(mut self, other: Self) -> Self {
        self.request_times.extend(other.total_times);
        self.total_times.extend(other.request_times);

        self
    }

    /// Simple helper returning the amount of requests overall.
    pub fn total_requests(&self) -> usize {
        self.request_times.len()
    }

    /// Calculates the average time per worker overall as a `Duration`
    ///
    /// Basic Logic:
    /// Sum(worker totals) / length = avg duration
    pub fn avg_total_time(&self) -> Duration {
        let avg: f64 = self.total_times
            .iter()
            .map(|dur| dur.as_secs_f64())
            .sum();

        let len = self.total_times.len() as f64;
        Duration::from_secs_f64(avg / len)
    }

    /// Calculates the average latency overall from all requests..
    pub fn avg_request_latency(&self) -> Duration {
       let avg: f64 = self.request_times
            .iter()
            .map(|dur| dur.as_secs_f64())
            .sum();

        let len = self.total_requests() as f64;
        Duration::from_secs_f64(avg / len)
    }

    /// Calculates the max latency overall from all requests.
    pub fn max_request_latency(&self) -> Duration {
       let max = self.request_times
           .iter()
           .map(|dur| dur)
           .max()
           .map(|res| *res)
           .unwrap_or(Duration::default());

        max
    }

    /// Calculates the min latency overall from all requests.
    pub fn min_request_latency(&self) -> Duration {
       let min = self.request_times
           .iter()
           .map(|dur| dur)
           .max()
           .map(|res| *res)
           .unwrap_or(Duration::default());

        min
    }

    /// Sorts the list of times.
    ///
    /// this is needed before calculating the Pn percentiles, this must be
    /// manually ran to same some compute time.
    pub fn sort_request_times(&mut self) {
        self.request_times.sort();
    }

    /// Works out the average latency of the 99 percentile.
    pub fn p99_avg_latency(&self) -> Duration {
        let len = self.request_times.len() as f64 * 0.01;
        let p99 = self.request_times
            .chunks(len as usize)
            .next()
            .expect("Failed to calculate P50 avg latency");

        let avg: f64 = p99.iter()
            .map(|dur| dur.as_secs_f64())
            .sum();

        Duration::from_secs_f64(avg)
    }

    /// Works out the average latency of the 95 percentile.
    pub fn p95_avg_latency(&self) -> Duration {
        let len = self.request_times.len() as f64 * 0.05;
        let p95 = self.request_times
            .chunks(len as usize)
            .next()
            .expect("Failed to calculate P50 avg latency");

        let avg: f64 = p95.iter()
            .map(|dur| dur.as_secs_f64())
            .sum();

        Duration::from_secs_f64(avg)
    }

    /// Works out the average latency of the 90 percentile.
    pub fn p90_avg_latency(&self) -> Duration {
        let len = self.request_times.len() as f64 * 0.10;
        let p90 = self.request_times
            .chunks(len as usize)
            .next()
            .expect("Failed to calculate P50 avg latency");

        let avg: f64 = p90.iter()
            .map(|dur| dur.as_secs_f64())
            .sum();

        Duration::from_secs_f64(avg)
    }

    /// Works out the average latency of the 75 percentile.
    pub fn p75_avg_latency(&mut self) -> Duration {
        let len = self.request_times.len() as f64 * 0.25;
        let p75 = self.request_times
            .chunks(len as usize)
            .next()
            .expect("Failed to calculate P50 avg latency");

        let avg: f64 = p75.iter()
            .map(|dur| dur.as_secs_f64())
            .sum();

        Duration::from_secs_f64(avg)
    }

    /// Works out the average latency of the 50 percentile.
    pub fn p50_avg_latency(&mut self) -> Duration {
        let len = self.request_times.len() / 2;
        let p50 = self.request_times
            .chunks(len)
            .next()
            .expect("Failed to calculate P50 avg latency");

        let avg: f64 = p50.iter()
            .map(|dur| dur.as_secs_f64())
            .sum();

        Duration::from_secs_f64(avg)
    }
}