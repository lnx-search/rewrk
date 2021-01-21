extern crate clap;
use clap::{Arg, App, ArgMatches};
use tokio::time::Duration;

mod http;
mod runtime;
mod bench;


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

    let http2: bool = match args
        .value_of("protocol")
        .unwrap_or("1")
        .parse::<u8>() {
        Ok(v) => v != 1,
        Err(_) => {
            println!("Invalid parameter for 'h2' given, input type must be a boolean.");
            return;
        }
    };

    let duration: u64 = match args
        .value_of("duration")
        .unwrap_or("1")
        .parse() {
        Ok(v) => v,
        Err(_) => {
            println!("Invalid parameter for 'duration' given, input type must be a integer.");
            return;
        }
    };

    let settings = bench::BenchmarkSettings {
        threads,
        connections: conns,
        host: host.to_string(),
        http2,
        duration: Duration::from_secs(duration)
    };

    bench::start_benchmark(settings);
}


fn parse_args() -> ArgMatches<'static> {
    App::new("Real World Wrk")
        .version("0.0.1")
        .author("Harrison Burt <hburt2003@gmail.com>")
        .about("Benched frameworks")
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
            Arg::with_name("protocol")
                .short("p")
                .long("protocol")
                .help("Set the client to use http2 only. (default is http/1) e.g. '-protocol 1'")
                .takes_value(true)
                .default_value("1")
        ).arg(
            Arg::with_name("duration")
                .short("d")
                .long("duration")
                .help("Set the duration of the benchmark in seconds.")
                .takes_value(true)
                .required(true)
        ).get_matches()
}