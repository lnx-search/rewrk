use anyhow::{anyhow, Result};
use colored::*;
use futures_util::StreamExt;
use std::fmt::Display;
use std::time::Duration;

use crate::http;
use crate::results::WorkerResult;
use crate::runtime;
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

    /// Display the percentile table.
    pub display_percentile: bool,

    /// Display the result data as a json.
    pub display_json: bool,

    /// The number of rounds to repeat.
    pub rounds: usize,
}

/// Builds the runtime with the given settings and blocks on the main future.
pub fn start_benchmark(settings: BenchmarkSettings) {
    let rt = runtime::get_rt(settings.threads);
    let rounds = settings.rounds;
    let is_json = settings.display_json;
    for i in 0..rounds {
        if !is_json {
            println!("Beginning round {}...", i + 1);
        }

        if let Err(e) = rt.block_on(run(settings.clone())) {
            eprintln!("failed to run benchmark round due to error: {:?}", e);
            return;
        }

        // Adds a line separator between rounds unless it's formatting
        // as a json, for readability.
        if !is_json {
            println!();
        };
    }
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
async fn run(settings: BenchmarkSettings) -> Result<()> {
    let predict_size = settings.duration.as_secs() * 10_000;

    let handles = http::start_tasks(
        settings.duration,
        settings.connections,
        settings.host.clone(),
        settings.bench_type,
        predict_size as usize,
    )
    .await;

    let mut handles = match handles {
        Ok(v) => v,
        Err(e) => return Err(anyhow!("error parsing uri: {}", e)),
    };

    if !settings.display_json {
        println!(
            "Benchmarking {} connections @ {} for {}",
            string(settings.connections).cyan(),
            settings.host,
            humanize(settings.duration),
        );
    }

    let mut combiner = WorkerResult::default();
    while let Some(result) = handles.next().await {
        match result.unwrap() {
            Ok(stats) => combiner = combiner.combine(stats),
            Err(e) => return Err(anyhow!("error combining results: {}", e)),
        }
    }

    if settings.display_json {
        combiner.display_json();
        return Ok(());
    }

    // prevent div-by-zero panics
    if combiner.total_requests() == 0 {
        println!("No requests completed successfully");
        return Ok(());
    }

    combiner.display_latencies();
    combiner.display_requests();
    combiner.display_transfer();

    if settings.display_percentile {
        combiner.display_percentile_table();
    }

    Ok(())
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

    let mut human = Vec::new();

    if days != 0 {
        human.push(format!("{} day(s)", days));
    };

    if hours != 0 {
        human.push(format!("{} hour(s)", hours));
    };

    if minutes != 0 {
        human.push(format!("{} minute(s)", minutes));
    };

    if seconds != 0 {
        human.push(format!("{} second(s)", seconds));
    };

    human.join(", ")
}
