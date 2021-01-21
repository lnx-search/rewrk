# rww
A more modern http framework benchmarker.

# Motivation
The motivation behind this project extends from developers tunnel visioning on benchmarks like [techempower](https://www.techempower.com/benchmarks/) that use the benchmarking tool called [wrk](https://github.com/wg/wrk).

The issue is that wrk only handle *some* of the HTTP spec and is entirely biased towards frameworks and servers that can make heavy use of HTTP/1 Pipelining which is no longer enabled in most modern browsers or clients, this can give a very unfare and unreasonable set of stats when comparing frameworks as those at the top are simply
better at using a process which is now not used greatly.

This is where rww or real world wrk comes in, this benchmarker is built on top of [hyper's client api](https://github.com/hyperium/hyper) and brings with it many advantages and more realistic methods of benchmarking.

### Current features
- Supports **both** HTTP/1 and HTTP/2.
- Pipelining is disabled giving a more realistic idea on actual perfromance.
- Multi-Platform support, developed on Windows but will run on Mac and Linux aswell.

### To do list
- Add a random artificial delay benchmark to simulate random latency with clients.
- Arithmetic benchmark to simulate diffrent loads across clients.
- State checking, making the frameworks and servers use all of their api rather than a minimised set.
- JSON deserialization and validation benchmarks and checking.

# Usage
Usage is relatively simple, if you have a compiled binary simply run using the CLI.

## Example
Here's an example to produce the following benchmark:
- 256 connections (`-c 256`)
- HTTP/1 only (`-p 1`)
- 12 threads (`-t 12`)
- 15 seconds (`-d 15`)
- on host `http://127.0.0.1:5000` (`-h http://127.0.0.1:5000`)<br>

**CLI command:**<br>
`rww -c 256 -t 12 -p 1 -d 15 -h http://127.0.0.1:5000`


## CLI Help
To brind up the help menue simply run `rww --help` to produce this:

```
USAGE:
    rww [OPTIONS] --duration <duration> --host <host>

FLAGS:
        --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -c, --connections <connections>    Set the amount of concurrent e.g. '-c 512' [default: 1]
    -d, --duration <duration>          Set the duration of the benchmark in seconds.
    -h, --host <host>                  Set the host to bench e.g. '-h http://127.0.0.1:5050'
    -p, --protocol <protocol>          Set the client to use http2 only. (default is http/1) e.g. '-protocol 1'
                                       [default: 1]
    -t, --threads <threads>            Set the amount of threads to use e.g. '-t 12' [default: 1]
```

# Building from source

Building from source is incredibly simple, just make sure you have a stable version of Rust installed before you start.

**With Cargo Run**
1) - Clone the repo source code
2) - Run `cargo run --release -- <enter flags here>`

**With Cargo Build**
1) - Clone the repo source code
2) - Run `cargo build --release`
3) - Extract the binary from the release folder
4) - Binary ready to use.
