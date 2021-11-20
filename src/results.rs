#![allow(unused)]

use std::collections::HashMap;

use colored::Colorize;
use serde_json::json;
use tokio::time::Duration;

use crate::utils::format_data;

fn get_percentile(request_times: &[Duration], pct: f64) -> Duration {
    let mut len = request_times.len() as f64 * pct;
    if len < 1.0 {
        len = 1.0;
    }

    let e = format!("failed to calculate P{} avg latency", (1.0 - pct) * 100f64);
    let pct = request_times.chunks(len as usize).next().expect(&e);

    let total: f64 = pct.iter().map(|dur| dur.as_secs_f64()).sum();

    let avg = total / pct.len() as f64;

    Duration::from_secs_f64(avg)
}

/// Contains and handles results from the workers
#[derive(Default)]
pub struct WorkerResult {
    /// The total time taken for each worker.
    pub total_times: Vec<Duration>,

    /// The vec of latencies per request stored.
    pub request_times: Vec<Duration>,

    /// The amount of data read from each worker.
    pub buffer_sizes: Vec<usize>,

    /// Error counting map.
    pub error_map: HashMap<String, usize>,
}

impl WorkerResult {
    /// Creates a empty result, useful for merging results into one
    /// consumer.
    pub fn default() -> Self {
        Self {
            total_times: vec![],
            request_times: vec![],
            buffer_sizes: vec![],
            error_map: HashMap::new(),
        }
    }

    /// Consumes both self and other producing a combined result.
    pub fn combine(mut self, other: Self) -> Self {
        self.request_times.extend(other.request_times);
        self.total_times.extend(other.total_times);
        self.buffer_sizes.extend(other.buffer_sizes);

        // Insert/add new errors to current error map.
        for (message, count) in other.error_map {
            match self.error_map.get_mut(&message) {
                Some(c) => *c += count,
                None => {
                    self.error_map.insert(message, count);
                },
            }
        }

        self
    }

    /// Simple helper returning the amount of requests overall.
    pub fn total_requests(&self) -> usize {
        self.request_times.len()
    }

    /// Calculates the total transfer in bytes.
    pub fn total_transfer(&self) -> usize {
        self.buffer_sizes.iter().sum()
    }

    /// Calculates the total transfer in bytes.
    pub fn avg_transfer(&self) -> f64 {
        self.total_transfer() as f64 / self.avg_total_time().as_secs_f64()
    }

    /// Calculates the requests per second average.
    pub fn avg_request_per_sec(&self) -> f64 {
        let amount = self.request_times.len() as f64;
        let avg_time = self.avg_total_time();

        amount / avg_time.as_secs_f64()
    }

    /// Calculates the average time per worker overall as a `Duration`
    ///
    /// Basic Logic:
    /// Sum(worker totals) / length = avg duration
    pub fn avg_total_time(&self) -> Duration {
        let avg: f64 = self.total_times.iter().map(|dur| dur.as_secs_f64()).sum();

        let len = self.total_times.len() as f64;
        Duration::from_secs_f64(avg / len)
    }

    /// Calculates the average latency overall from all requests..
    pub fn avg_request_latency(&self) -> Duration {
        let avg: f64 = self.request_times.iter().map(|dur| dur.as_secs_f64()).sum();

        let len = self.total_requests() as f64;
        Duration::from_secs_f64(avg / len)
    }

    /// Calculates the max latency overall from all requests.
    pub fn max_request_latency(&self) -> Duration {
        self.request_times.iter().max().copied().unwrap_or_default()
    }

    /// Calculates the min latency overall from all requests.
    pub fn min_request_latency(&self) -> Duration {
        self.request_times.iter().min().copied().unwrap_or_default()
    }

    /// Calculates the variance between all requests
    pub fn variance(&self) -> f64 {
        let mean = self.avg_request_latency().as_secs_f64();
        let sum_delta: f64 = self
            .request_times
            .iter()
            .map(|dur| {
                let time = dur.as_secs_f64();
                let delta = time - mean;

                delta.powi(2)
            })
            .sum();

        sum_delta / self.total_requests() as f64
    }

    /// Calculates the standard deviation of request latency.
    pub fn std_deviation_request_latency(&self) -> f64 {
        let diff = self.variance();
        diff.powf(0.5)
    }

    /// Sorts the list of times.
    ///
    /// this is needed before calculating the Pn percentiles, this must be
    /// manually ran to same some compute time.
    pub fn sort_request_times(&mut self) {
        self.request_times.sort_by(|a, b| b.partial_cmp(a).unwrap());
    }

    /// Works out the average latency of the 99.9 percentile.
    pub fn p999_avg_latency(&self) -> Duration {
        get_percentile(&self.request_times, 0.001)
    }

    /// Works out the average latency of the 99 percentile.
    pub fn p99_avg_latency(&self) -> Duration {
        get_percentile(&self.request_times, 0.01)
    }

    /// Works out the average latency of the 95 percentile.
    pub fn p95_avg_latency(&self) -> Duration {
        get_percentile(&self.request_times, 0.05)
    }

    /// Works out the average latency of the 90 percentile.
    pub fn p90_avg_latency(&self) -> Duration {
        get_percentile(&self.request_times, 0.1)
    }

    /// Works out the average latency of the 75 percentile.
    pub fn p75_avg_latency(&mut self) -> Duration {
        get_percentile(&self.request_times, 0.25)
    }

    /// Works out the average latency of the 50 percentile.
    pub fn p50_avg_latency(&mut self) -> Duration {
        get_percentile(&self.request_times, 0.5)
    }

    pub fn display_latencies(&mut self) {
        let modified = 1000_f64;
        let avg = self.avg_request_latency().as_secs_f64() * modified;
        let max = self.max_request_latency().as_secs_f64() * modified;
        let min = self.min_request_latency().as_secs_f64() * modified;
        let std_deviation = self.std_deviation_request_latency() * modified;

        println!("  Latencies:");
        println!(
            "    {:<7}  {:<7}  {:<7}  {:<7}  ",
            "Avg".bright_yellow(),
            "Stdev".bright_magenta(),
            "Min".bright_green(),
            "Max".bright_red(),
        );
        println!(
            "    {:<7}  {:<7}  {:<7}  {:<7}  ",
            format!("{:.2}ms", avg),
            format!("{:.2}ms", std_deviation),
            format!("{:.2}ms", min),
            format!("{:.2}ms", max),
        );
    }

    pub fn display_requests(&mut self) {
        let total = self.total_requests();
        let avg = self.avg_request_per_sec();

        println!("  Requests:");
        println!(
            "    Total: {:^7} Req/Sec: {:^7}",
            format!("{}", total).as_str().bright_cyan(),
            format!("{:.2}", avg).as_str().bright_cyan()
        )
    }

    pub fn display_transfer(&mut self) {
        let total = self.total_transfer() as f64;
        let rate = self.avg_transfer();

        let display_total = format_data(total as f64);
        let display_rate = format_data(rate);

        println!("  Transfer:");
        println!(
            "    Total: {:^7} Transfer Rate: {:^7}",
            display_total.as_str().bright_cyan(),
            format!("{}/Sec", display_rate).as_str().bright_cyan()
        )
    }

    pub fn display_percentile_table(&mut self) {
        self.sort_request_times();

        println!("+ {:-^15} + {:-^15} +", "", "",);

        println!(
            "| {:^15} | {:^15} |",
            "Percentile".bright_cyan(),
            "Avg Latency".bright_yellow(),
        );

        println!("+ {:-^15} + {:-^15} +", "", "",);

        let modifier = 1000_f64;
        println!(
            "| {:^15} | {:^15} |",
            "99.9%",
            format!("{:.2}ms", self.p999_avg_latency().as_secs_f64() * modifier)
        );
        println!(
            "| {:^15} | {:^15} |",
            "99%",
            format!("{:.2}ms", self.p99_avg_latency().as_secs_f64() * modifier)
        );
        println!(
            "| {:^15} | {:^15} |",
            "95%",
            format!("{:.2}ms", self.p95_avg_latency().as_secs_f64() * modifier)
        );
        println!(
            "| {:^15} | {:^15} |",
            "90%",
            format!("{:.2}ms", self.p90_avg_latency().as_secs_f64() * modifier)
        );
        println!(
            "| {:^15} | {:^15} |",
            "75%",
            format!("{:.2}ms", self.p75_avg_latency().as_secs_f64() * modifier)
        );
        println!(
            "| {:^15} | {:^15} |",
            "50%",
            format!("{:.2}ms", self.p50_avg_latency().as_secs_f64() * modifier)
        );

        println!("+ {:-^15} + {:-^15} +", "", "",);
    }

    pub fn display_errors(&self) {
        if !self.error_map.is_empty() {
            println!();

            for (message, count) in &self.error_map {
                println!("{} Errors: {}", count, message);
            }
        }
    }

    pub fn display_json(&self) {
        // prevent div-by-zero panics
        if self.total_requests() == 0 {
            let null = None::<()>;

            let out = json!({
                "latency_avg": null,
                "latency_max": null,
                "latency_min": null,
                "latency_std_deviation": null,

                "transfer_total": null,
                "transfer_rate": null,

                "requests_total": 0,
                "requests_avg": null,
            });

            println!("{}", out.to_string());
            return;
        }

        let modified = 1000_f64;
        let avg = self.avg_request_latency().as_secs_f64() * modified;
        let max = self.max_request_latency().as_secs_f64() * modified;
        let min = self.min_request_latency().as_secs_f64() * modified;
        let std_deviation = self.std_deviation_request_latency() * modified;

        let total = self.total_transfer() as f64;
        let rate = self.avg_transfer();

        let total_requests = self.total_requests();
        let avg_request_per_sec = self.avg_request_per_sec();

        let out = json!({
            "latency_avg": avg,
            "latency_max": max,
            "latency_min": min,
            "latency_std_deviation": std_deviation,

            "transfer_total": total,
            "transfer_rate": rate,

            "requests_total": total_requests,
            "requests_avg": avg_request_per_sec,
        });

        println!("{}", out.to_string())
    }
}
