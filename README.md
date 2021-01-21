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

