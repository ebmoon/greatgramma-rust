# Benchmark protocol v1

Benchmarks are not runnable during bootstrap. Before performance claims, freeze
small, medium, and 32K/50K/128K-token corpora with source hashes; record
hardware, OS, Rust and dependency pins, warm-up count, repetitions, p50/p95/p99
latencies, peak RSS, retained-bytes/RSS slope, and derived-table size.

The first correct implementation establishes the baseline. Any regression above
10% in time or memory requires an ADR amendment with a reproduced comparison.
No benchmark result from this repository may compare a Rust matcher with
llguidance as though the algorithms shared semantics; it is informational only.
