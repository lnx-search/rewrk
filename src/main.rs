extern crate clap;
use clap::{Arg, App, ArgMatches};
use tokio::time::Duration;
use regex::Regex;

mod http;
mod runtime;
mod bench;
mod proto;
mod results;


/// Matches a string like '12d 24h 5m 45s' to a regex capture.
static DURATION_MATCH: &str = "(?P<days>[0-9]*)d|(?P<hours>[0-9]*)h|(?P<minutes>[0-9]*)m|(?P<seconds>[0-9]*)s";


/// ReWrk
///
/// Captures CLI arguments and build benchmarking settings and runtime to
/// suite the arguments and options.
fn main() {
    let args = parse_args();

    let threads: usize = match args
        .value_of("threads")
        .unwrap_or("1")
        .parse() {
        Ok(v) => v,
        Err(_) => {
            println!("Invalid parameter for 'threads' given, input type must be a integer.");
            return;
        }
    };

    let conns: usize = match args
        .value_of("connections")
        .unwrap_or("1")
        .parse() {
        Ok(v) => v,
        Err(_) => {
            println!("Invalid parameter for 'connections' given, input type must be a integer.");
            return;
        }
    };

    let host: &str = match args.value_of("host") {
        Some(v) => v,
        None => {
            println!("Missing 'host' parameter.");
            return;
        }
    };

    let http2: bool = args.is_present("http2");

    let duration: &str = args.value_of("duration").unwrap_or("1s");
    let duration = match parse_duration(duration) {
        Ok(dur) => dur,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    let pct: bool = args.is_present("pct");

    let settings = bench::BenchmarkSettings {
        threads,
        connections: conns,
        host: host.to_string(),
        http2,
        duration,
        display_percentile: pct,
    };

    bench::start_benchmark(settings);
}


/// Parses a duration string from the CLI to a Duration.
/// '11d 3h 32m 4s' -> Duration
///
/// If no matches are found for the string or a invalid match
/// is captured a error message returned and displayed.
fn parse_duration(duration: &str) -> Result<Duration, String> {
    let mut dur = Duration::default();

    let re = Regex::new(DURATION_MATCH).unwrap();
    for cap in re.captures_iter(duration) {
        let add_to = if let Some(days) = cap.name("days") {
            let days = days
                .as_str()
                .parse::<u64>()
                .unwrap();

            let seconds = days * 24 * 60 * 60;
            Duration::from_secs(seconds)

        } else if let Some(hours) = cap.name("hours") {
            let hours = hours
                .as_str()
                .parse::<u64>()
                .unwrap();

            let seconds = hours * 60 * 60;
            Duration::from_secs(seconds)

        } else if let Some(minutes) = cap.name("minutes") {
            let minutes = minutes
                .as_str()
                .parse::<u64>()
                .unwrap();

            let seconds = minutes * 60;
            Duration::from_secs(seconds)

        } else if let Some(seconds) = cap.name("seconds") {
            let seconds = seconds
                .as_str()
                .parse::<u64>()
                .unwrap();

            Duration::from_secs(seconds)

        } else {
            return Err(format!("Invalid match: {:?}", cap))
        };

        dur += add_to
    }


    if dur.as_secs() <= 0 {
        return Err(format!(
            "Failed to extract any valid duration from {}",
            duration
        ))
    }

    Ok(dur)
}


/// Contains Clap's app setup.
///
/// Duration:
///     Benchmark HTTP/1 and HTTP/2 frameworks without pipelining bias.
///
/// Options:
///     threads (t):
///         Set the amount of threads to use e.g. '-t 12'
///     connections (c):
///         Set the amount of concurrent e.g. '-c 512'
///     host (h):
///         Set the host to bench e.g. '-h http://127.0.0.1:5050'
///     protocol (p):
///         Set the client to use http2 only. (default is http/1)
///         e.g. '-protocol 1'
///     duration (d):
///         Set the duration of the benchmark.
fn parse_args() -> ArgMatches<'static> {
    App::new("ReWrk")
        .version("0.1.2")
        .author("Harrison Burt <hburt2003@gmail.com>")
        .about("Benchmark HTTP/1 and HTTP/2 frameworks without pipelining bias.")
        .arg(
            Arg::with_name("threads")
                .short("t")
                .long("threads")
                .help("Set the amount of threads to use e.g. '-t 12'")
                .takes_value(true)
                .default_value("1")
        ).arg(
            Arg::with_name("connections")
                .short("c")
                .long("connections")
                .help("Set the amount of concurrent e.g. '-c 512'")
                .takes_value(true)
                .default_value("1")
        ).arg(
            Arg::with_name("host")
                .short("h")
                .long("host")
                .help("Set the host to bench e.g. '-h http://127.0.0.1:5050'")
                .takes_value(true)
                .required(true)
        ).arg(
            Arg::with_name("http2")
                .long("http2")
                .help("Set the client to use http2 only. (default is http/1) e.g. '--http2'")
                .required(false)
                .takes_value(false)
    ).arg(
            Arg::with_name("duration")
                .short("d")
                .long("duration")
                .help("Set the duration of the benchmark.")
                .takes_value(true)
                .required(true)
        ).arg(
            Arg::with_name("pct")
                .long("pct")
                .help("Displays the percentile table after benchmarking.")
                .takes_value(false)
                .required(false)

        ).get_matches()
}