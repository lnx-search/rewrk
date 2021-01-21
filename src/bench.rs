use std::time::Duration;
use std::fmt::Display;
use tokio::time::Instant;
use colored::*;

use crate::runtime;
use crate::http;


/// The customisable settings that build the benchmark's behaviour.
#[derive(Clone)]
pub struct BenchmarkSettings {
    /// The number of worker threads given to Tokio's runtime.
    pub threads: usize,

    /// The amount of concurrent connections when connecting to the
    /// framework.
    pub connections: usize,

    /// The host connection / url.
    pub host: String,

    /// HTTP/2 only (on/off).
    pub http2: bool,

    /// The duration of the benchmark.
    pub duration: Duration,
}


/// Builds the runtime with the given settings and blocks on the main future.
pub fn start_benchmark(settings: BenchmarkSettings) {
    let rt = runtime::get_rt(settings.threads);
    rt.block_on(run(settings))
}


/// Controls the benchmark itself.
///
/// A pool is created with a set of options that then wait for the
/// channels to be filled with requests.
///
/// Once the duration has elapsed the handles are awaited and the results
/// extracted from the handle.
///
/// The results are then merged into a single set of averages across workers.
async fn run(settings: BenchmarkSettings) {
    let (emitter, handles) = http::create_pool(
        settings.connections,
        settings.host.clone(),
        settings.http2,
    ).await;

    println!(
        "Benchmarking {} connections @ {} for {}",
        string(settings.connections).cyan(),
        settings.host,
        humanize(settings.duration),
    );

    let start = Instant::now();
    while start.elapsed() < settings.duration {
        let _ = emitter.send(()).await;
    }
    drop(emitter);


    let mut total_request = Vec::new();
    let mut total_max = Vec::new();
    let mut total_min = Vec::new();
    let mut total_time = Vec::new();

    for handle in handles {
        let result = match handle.await {
            Ok(r) => r,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };

        if let Ok((max, min, total, time)) = result {
            total_max.push(max);
            total_min.push(min);
            total_request.push(total);
            total_time.push(time);
        }
    }

    let total_reqs: usize = total_request.iter().sum();
    let max: f64 = total_max.iter().map(|v| v.as_secs_f64()).sum::<f64>() / settings.connections as f64;
    let min: f64 = total_min.iter().map(|v| v.as_secs_f64()).sum::<f64>() / settings.connections as f64;
    let time_taken: f64 = total_time.iter().map(|v| v.as_secs_f64()).sum::<f64>() / settings.connections as f64;


    let modified: f64 = 1000.0;

    let median = ((max - min) / 2.0) * modified;
    let max = max * modified;
    let min = min * modified;


    println!(
        "  Latencies:\n    \
        min    - {}ms\n    \
        max    - {}ms\n    \
        median - {}ms",
        string(min).green(),
        string(max).red(),
        string(median).yellow(),
    );
    println!(
        "  Requests:\n    \
        Total Requests - {}\n    \
        Requests/Sec   - {} ",
        string(total_reqs).cyan(),
        string(total_reqs as f64 / time_taken).cyan(),
    );
}


/// Uber lazy way of just stringing everything and limiting it to 2 d.p
fn string<T: Display>(value: T) -> String {
    format!("{:.2}", value)
}

/// Turns a fairly un-readable float in seconds / Duration into a human
/// friendly string.
///
/// E.g.
/// 10,000 seconds -> '2 hours, 46 minutes, 40 seconds'
fn humanize(time: Duration) -> String {
    let seconds = time.as_secs();

    let (minutes, seconds) = div_mod(seconds, 60);
    let (hours, minutes) = div_mod(minutes, 60);
    let (days, hours) = div_mod(hours, 24);

    let mut human = String::new();

    if days != 0 {
        human = format!("{} days, ", days);
    };

    if hours != 0 {
        human = format!("{}{} hours, ", human, hours);
    };

    if minutes != 0 {
        human = format!("{}{} minutes, ", human, minutes);
    };

    if seconds != 0 {
        human = format!("{}{} seconds", human, seconds);
    };

    human
}

/// Dirt simple div mod function.
fn div_mod(main: u64, divider: u64) -> (u64, u64) {
    let whole = main / divider;
    let rem = main % divider;

    (whole, rem)
}