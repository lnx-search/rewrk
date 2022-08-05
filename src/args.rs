use std::path::PathBuf;
use std::time::Duration;
use clap::Parser;
use url::Url;


#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
/// A more modern http framework benchmarker supporting HTTP/1 and HTTP/2 benchmarks.
pub struct Args {
    #[clap(short, long, env, default_value_t = 1)]
    /// The number of runtime threads to use during the benchmark.
    pub threads: usize,

    #[clap(short, long, env, default_value_t = 64)]
    /// The number of concurrent connections during the benchmark.
    pub concurrency: usize,

    #[clap(short, long, env, value_parser = check_address)]
    /// The server host address.
    pub address: Url,

    #[clap(long, env)]
    /// Set the benchmarker client to use HTTP2 only.
    pub http2_only: bool,

    #[clap(short, long, env, value_parser = parse_duration)]
    /// The duration to run the benchmark for, this can be
    /// passed in the format `12h 5min 2ns` etc...
    pub duration: Duration,

    #[clap(long, env, value_parser = parse_duration, default_value = "0s")]
    /// The duration to run a warmup benchmark which is ran before the sample are taken,
    /// this can be passed in the format `12h 5min 2ns` etc...
    pub warmup: Duration,

    #[clap(long, env, default_value_t = 1)]
    /// The number of times to repeat the benchmark and sample taking.
    pub rounds: usize,

    #[clap(long, env, default_value_t = http::Method::GET)]
    /// The HTTP method to use during requests.
    pub method: http::Method,

    #[clap(long, env)]
    /// The body of the HTTP requests. This can either be the raw body, or a path to a file.
    pub body: Option<String>,

    #[clap(long, env, value_parser = parse_duration)]
    /// An optional setup script to run which is executed at the start of each round.
    ///
    /// This must be a `rhai` script. See `scripting.md` for more.
    pub setup: Option<PathBuf>,
}

fn check_address(s: &str) -> Result<Url, String> {
    Url::parse(s).map_err(|e| e.to_string())
}

fn parse_duration(s: &str) -> Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| e.to_string())
}

fn parse_file_path(s: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(s);

    if !path.exists() {
        return Err(format!(
            "Cannot find setup script file @ {:?}",
            path.canonicalize().unwrap_or_else(|_| path),
        ))
    }

    Ok(path)
}