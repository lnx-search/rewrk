"""
This is an example benchmarking test using python to benchmark Pyre one of
my pet projects.

This measures the latency and throughput and displays them with matplotlib.
"""

import matplotlib.pyplot as plt
import sys
import os

from subprocess import Popen, PIPE
from json import loads


def start_benchmark(
    host: str,
    connections: int,
    time: str = "10s",
    rounds: int = 3,
) -> list:
    command = f"cargo run --release -- " \
              f"-h {host} " \
              f"-c {connections} " \
              f"-d {time} " \
              f"-t 12 " \
              f"--rounds {rounds} " \
              f"--json"
    process = Popen(command, shell=True, stdout=PIPE, stderr=PIPE)

    out, err = process.communicate()
    print(err)
    out = out.decode(sys.stdin.encoding)
    return [loads(o) for o in out.splitlines()]


def get_avg_without_enom(inputs: list) -> float:
    print(inputs)
    _, *good, _ = sorted(inputs)
    return sum(good) / len(good)


def make_runs():
    host = "http://127.0.0.1:8080"
    connections_start = 60
    connections_end = 100
    connections_step = 5

    x_index = []
    latencies = []
    req_secs = []
    for conns in range(connections_start, connections_end, connections_step):
        results = start_benchmark(host, conns, time="10s", rounds=5)
        avg_latency = get_avg_without_enom([o['latency_avg'] for o in results])
        avg_req_sec = get_avg_without_enom([o['requests_avg'] for o in results])

        x_index.append(conns)
        latencies.append(avg_latency)
        req_secs.append(avg_req_sec)

    plt.figure()
    plt.xlabel("Connection Concurrency")
    plt.ylabel("Latency / ms")
    plt.title("Benchmark Results")
    plt.plot(x_index, latencies)
    plt.savefig("./latencies.png")
    plt.close()

    plt.figure()
    plt.xlabel("Connection Concurrency")
    plt.ylabel("Request Per Second")
    plt.title("Benchmark Results")
    plt.plot(x_index, req_secs)
    plt.savefig("./requests.png")


if __name__ == '__main__':
    make_runs()