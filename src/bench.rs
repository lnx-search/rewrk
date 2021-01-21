use std::time::Duration;
use tokio::time::Instant;

use crate::runtime;
use crate::http;

#[derive(Clone)]
pub struct BenchmarkSettings {
    pub threads: usize,
    pub connections: usize,
    pub host: String,
    pub http2: bool,
    pub duration: Duration,
}

pub fn start_benchmark(settings: BenchmarkSettings) {
    let rt = runtime::get_rt(settings.threads);
    rt.block_on(run(settings))
}


async fn run(settings: BenchmarkSettings) {
    let (emitter, handles) = http::create_pool(
        settings.connections,
        settings.host.clone(),
        settings.http2,
    ).await;

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

    let mode = (max - min) * modified;
    let max = max * modified;
    let min = min * modified;


    println!(
        "Benchmarking {} connections @ {} for {}s",
        settings.connections,
        settings.host,
        settings.duration.as_secs(),
    );
    println!(
        "Latencies:\n  {:.2}ms min, {:.2}ms max, {:.2}ms mode",
        min,
        max,
        mode,
    );
    println!(
        "Requests:\n  {} requests in {:.2}s, {:.2} req/sec",
        total_reqs,
        time_taken,
        (total_reqs as f64 / time_taken),
    );
}
