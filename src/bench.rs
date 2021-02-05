use std::time::Duration;
use std::fmt::Display;
use tokio::time::Instant;
use colored::*;

use crate::runtime;
use crate::http;
use crate::results::WorkerResult;
use crate::utils::div_mod;


/// The customisable settings that build the benchmark's behaviour.
#[derive(Clone, Debug)]
pub struct BenchmarkSettings {
    /// The number of worker threads given to Tokio's runtime.
    pub threads: usize,

    /// The amount of concurrent connections when connecting to the
    /// framework.
    pub connections: usize,

    /// The host connection / url.
    pub host: String,

    /// The bench mark type e.g. http1 only.
    pub bench_type: http::BenchType,

    /// The duration of the benchmark.
    pub duration: Duration,

    pub display_percentile: bool,
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
    let predict_size = settings.duration.as_secs() * 10_000;

    let (emitter, handles) = http::create_pool(
        settings.connections,
        settings.host.clone(),
        settings.bench_type,
        predict_size as usize,
    ).await;

    println!(
        "Benchmarking {} connections @ {} for {}",
        string(settings.connections).cyan(),
        settings.host,
        humanize(settings.duration),
    );

    let start = Instant::now();
    while start.elapsed() < settings.duration {
        if let Err(_) = emitter.send(()).await {
            break;
        };
    }
    drop(emitter);


    let mut combiner = WorkerResult::default();
    for handle in handles {
        let result = match handle.await {
            Ok(r) => r,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };

        if let Ok(stats) = result {
            combiner = combiner.combine(stats);
        } else if let Err(e) = result {
            eprintln!("{}", e);
            return;
        }
    }

    combiner.display_latencies();
    combiner.display_requests();
    combiner.display_transfer();

    if settings.display_percentile {
        combiner.display_percentile_table();
    }
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


