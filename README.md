# rewrk
A more modern http framework benchmark utility.

```
F:\rewrk> rewrk -h http://127.0.0.1:5000 -t 12 -c 60 -d 5s

Benchmarking 60 connections @ http://127.0.0.1:5000 for 5 seconds
  Latencies:
    Avg      Stdev    Min      Max    
    3.27ms   0.40ms   1.95ms   9.39ms
  Requests:
    Total:  91281  Req/Sec: 18227.81
  Transfer:
    Total: 1.13 MB Transfer Rate: 231.41 KB/Sec
```

*With optional --pct flag*
```
+ --------------- + --------------- +
|   Percentile    |   Avg Latency   |
+ --------------- + --------------- +
|      99.9%      |     6.88ms      |
|       99%       |     5.62ms      |
|       95%       |     4.62ms      |
|       90%       |     4.24ms      |
|       75%       |     3.78ms      |
|       50%       |     3.49ms      |
+ --------------- + --------------- +
```

# Motivation
The motivation behind this project extends from developers tunnel visioning on benchmarks like [techempower](https://www.techempower.com/benchmarks/) that use the benchmarking tool called [wrk](https://github.com/wg/wrk).

The issue is that wrk only handle *some* of the HTTP spec and is entirely biased towards frameworks and servers that can make heavy use of HTTP/1 Pipelining which is no longer enabled in most modern browsers or clients, this can give a very unfare and unreasonable set of stats when comparing frameworks as those at the top are simply
better at using a process which is now not used greatly.

This is where rewrk comes in, this benchmarker is built on top of [hyper's client api](https://github.com/hyperium/hyper) and brings with it many advantages and more realistic methods of benchmarking.

### Current features
- Supports **both** HTTP/1 and HTTP/2.
- Pipelining is disabled giving a more realistic idea on actual perfromance.
- Multi-Platform support, developed on Windows but will run on Mac and Linux aswell.

### To do list
- Add a random artificial delay benchmark to simulate random latency with clients.
- Arithmetic benchmark to simulate diffrent loads across clients.
- State checking, making the frameworks and servers use all of their api rather than a minimised set.
- JSON deserialization and validation benchmarks and checking.
- Truly concurrent HTTP/2 benchmark.

# Usage
Usage is relatively simple, if you have a compiled binary simply run using the CLI.

## Example
Here's an example to produce the following benchmark:
- 256 connections (`-c 256`)
- HTTP/2 only (`--http2`)
- 12 threads (`-t 12`)
- 15 seconds (`-d 15s`)
- with percentile table (`--pct`)
- on host `http://127.0.0.1:5000` (`-h http://127.0.0.1:5000`)<br>

**CLI command:**<br>
`rewrk -c 256 -t 12 -d 15s -h http://127.0.0.1:5000 --http2 --pct`


## CLI Help
To bring up the help menu simply run `rewrk --help` to produce this:

```
USAGE:
    rewrk.exe [FLAGS] [OPTIONS] --duration <duration> --host <host>

FLAGS:
        --help       Prints help information
        --http2      Set the client to use http2 only. (default is http/1) e.g. '--http2'
        --pct        Displays the percentile table after benchmarking.
    -V, --version    Prints version information

OPTIONS:
    -c, --connections <connections>    Set the amount of concurrent e.g. '-c 512' [default: 1]
    -d, --duration <duration>          Set the duration of the benchmark.
    -h, --host <host>                  Set the host to bench e.g. '-h http://127.0.0.1:5050'
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
