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
        settings.host,
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

    let total_req: usize = total_request.iter().sum();
    let max: f64 = total_max.iter().map(|v| v.as_secs_f64()).sum::<f64>() / settings.connections as f64;
    let min: f64 = total_min.iter().map(|v| v.as_secs_f64()).sum::<f64>() / settings.connections as f64;
    let avg_time: f64 = total_time.iter().map(|v| v.as_secs_f64()).sum::<f64>() / settings.connections as f64;
    println!(
        "{} total req, {:.4}ms max, {:.4}ms min, {:.4} avg time, {} Req/sec",
        total_req,
        max * 1000 as f64,
        min * 1000 as f64,
        avg_time,
        total_req as f64 / avg_time,
    )
}
