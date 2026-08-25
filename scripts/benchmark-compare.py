#!/usr/bin/env python3
"""Compare benchmark NDJSON without manufacturing an upstream claim.

The baseline must identify the frozen upstream GreatGramma revision. Both files
must use the committed synthetic corpus protocol and retain their raw samples.
An absent, incompatible, or incomplete input is an error (exit 2); a measured
threshold miss is a comparison failure (exit 1).
"""

from __future__ import annotations

import sys

if not sys.flags.isolated:
    raise SystemExit("benchmark comparator requires scripts/benchmark-compare.sh")

import argparse
import hashlib
import json
import math
import os
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable, TextIO, cast


RESULT_SCHEMA = "greatgramma.python-benchmark.v1"
PROTOCOL_SCHEMA = "greatgramma.synthetic-benchmark-protocol.v1"
CORPUS_SCHEMA = "greatgramma.synthetic-exact-byte-corpus.v1"
THRESHOLD_SCHEMA = "greatgramma.python-benchmark-thresholds.v1"
COMPARISON_SCHEMA = "greatgramma.python-benchmark-comparison.v1"
UPSTREAM_PIN = "4c21981386fc6d457efa381d1eb1863623d50fa1"
DEFAULT_PROTOCOL = (
    Path(__file__).resolve().parents[1]
    / "tests"
    / "fixtures"
    / "benchmark"
    / "protocol-v1.json"
)


class InputError(ValueError):
    """The comparison inputs are absent or not mutually compatible."""


def canonical_bytes(value: object) -> bytes:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).encode("ascii")


def object_hash(value: object) -> str:
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


def file_hash(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as error:
        raise InputError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def benchmark_environment_policy(protocol: dict[str, Any]) -> dict[str, Any]:
    raw_value = protocol.get("benchmark_environment")
    if not isinstance(raw_value, dict):
        raise InputError("protocol has no exact benchmark environment policy")
    raw = cast(dict[str, object], raw_value)
    if set(raw) != {
        "allowed_names",
        "required_names",
        "required_values",
    }:
        raise InputError("protocol has no exact benchmark environment policy")
    allowed_value = raw.get("allowed_names")
    required_value = raw.get("required_names")
    values_value = raw.get("required_values")
    if (
        not isinstance(allowed_value, list)
        or not allowed_value
        or any(
            not isinstance(name, str) or not name
            for name in cast(list[object], allowed_value)
        )
        or not isinstance(required_value, list)
        or not required_value
        or any(
            not isinstance(name, str) or not name
            for name in cast(list[object], required_value)
        )
        or not isinstance(values_value, dict)
        or any(
            not isinstance(name, str)
            or not isinstance(value, str)
            for name, value in cast(dict[object, object], values_value).items()
        )
    ):
        raise InputError("protocol has no exact benchmark environment policy")
    allowed = cast(list[str], allowed_value)
    required = cast(list[str], required_value)
    values = cast(dict[str, str], values_value)
    if (
        len(set(allowed)) != len(allowed)
        or len(set(required)) != len(required)
        or not set(required).issubset(allowed)
        or not set(values).issubset(required)
        or values.get("GREATGRAMMA_BENCHMARK_SANITIZED") != "1"
    ):
        raise InputError("protocol has no exact benchmark environment policy")
    return {
        "allowed_names": list(allowed),
        "required_names": list(required),
        "required_values": dict(values),
    }


def release_sampling(protocol: dict[str, Any]) -> dict[str, int]:
    expected = {
        "compile_samples": 7,
        "operation_samples": 1000,
        "warmup_count": 10,
    }
    if protocol.get("release_sampling") != expected:
        raise InputError("protocol does not select the release sample counts")
    return expected


def environment_policy_violations(
    policy: dict[str, Any], environment: dict[str, str]
) -> list[str]:
    allowed = set(cast(list[str], policy["allowed_names"]))
    required = set(cast(list[str], policy["required_names"]))
    values = cast(dict[str, str], policy["required_values"])
    present = set(environment)
    violations = [f"unexpected variable {name}" for name in sorted(present - allowed)]
    violations.extend(f"missing variable {name}" for name in sorted(required - present))
    violations.extend(
        f"variable {name} must equal {expected!r}"
        for name, expected in sorted(values.items())
        if environment.get(name) != expected
    )
    return violations


def enforce_environment_policy(protocol: dict[str, Any]) -> dict[str, Any]:
    policy = benchmark_environment_policy(protocol)
    violations = environment_policy_violations(policy, dict(os.environ))
    if violations:
        raise InputError(
            "benchmark must use scripts/benchmark-compare.sh: "
            + "; ".join(violations)
        )
    return policy


def load_object(path: Path, label: str) -> dict[str, Any]:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except OSError as error:
        raise InputError(f"cannot read {label} {path}: {error}") from error
    except json.JSONDecodeError as error:
        raise InputError(f"invalid JSON in {label} {path}: {error}") from error
    if not isinstance(raw, dict):
        raise InputError(f"{label} must be one JSON object")
    return cast(dict[str, Any], raw)


def load_protocol(path: Path) -> tuple[dict[str, Any], str]:
    protocol = load_object(path, "protocol")
    if protocol.get("schema") != PROTOCOL_SCHEMA:
        raise InputError(f"unsupported benchmark protocol schema in {path}")
    if protocol.get("result_schema") != RESULT_SCHEMA:
        raise InputError("protocol selects an unsupported result schema")
    if protocol.get("corpus_schema") != CORPUS_SCHEMA:
        raise InputError("protocol selects an unsupported corpus schema")
    if protocol.get("python_isolated_mode_required") is not True:
        raise InputError("protocol does not require isolated Python mode")
    if protocol.get("candidate_rust_toolchain") != "1.98.0":
        raise InputError("protocol does not select the pinned Rust toolchain")
    release_sampling(protocol)
    if (
        not isinstance(protocol.get("grammar"), dict)
        or not isinstance(protocol.get("upstream_grammar"), dict)
        or not isinstance(protocol.get("language"), str)
    ):
        raise InputError("protocol has no shared grammar semantics")
    upstream = protocol.get("upstream")
    if not isinstance(upstream, dict) or cast(dict[str, Any], upstream).get(
        "source_pin"
    ) != UPSTREAM_PIN:
        raise InputError("protocol does not select the frozen upstream revision")
    dependencies = cast(dict[str, Any], upstream).get("python_dependencies")
    if (
        not isinstance(dependencies, dict)
        or not dependencies
        or any(
            not isinstance(name, str)
            or not name
            or not isinstance(version, str)
            or not version
            for name, version in cast(dict[object, object], dependencies).items()
        )
    ):
        raise InputError("protocol has no exact upstream dependency set")
    benchmark_environment_policy(protocol)
    operations = protocol.get("operations")
    if not isinstance(operations, dict) or not operations:
        raise InputError("protocol has no operation registry")
    return protocol, object_hash(protocol)


def generated_token(token_id: int) -> bytes:
    if token_id < 256:
        return bytes((token_id,))
    return b"\xa5" + token_id.to_bytes(4, "little")


def expected_corpus(
    protocol: dict[str, Any], protocol_hash: str, vocab_size: int
) -> dict[str, Any]:
    if not 257 <= vocab_size <= 1_000_000:
        raise InputError(f"unsupported synthetic vocabulary size {vocab_size}")
    stream = hashlib.sha256()
    for token_id in range(vocab_size - 1):
        token = generated_token(token_id)
        stream.update(token_id.to_bytes(4, "little"))
        stream.update(len(token).to_bytes(4, "little"))
        stream.update(token)
    eos = vocab_size - 1
    stream.update(eos.to_bytes(4, "little"))
    stream.update((0).to_bytes(4, "little"))
    descriptor: dict[str, Any] = {
        "corpus_schema": CORPUS_SCHEMA,
        "corpus_id": f"synthetic-exact-byte-v1-vocab-{vocab_size}",
        "vocab_size": vocab_size,
        "eos_token_ids": [eos],
        "token_stream_sha256": stream.hexdigest(),
        "grammar_sha256": object_hash(
            {
                "language": protocol["language"],
                "candidate": protocol["grammar"],
                "upstream": protocol["upstream_grammar"],
            }
        ),
        "protocol_sha256": protocol_hash,
    }
    descriptor["corpus_sha256"] = object_hash(descriptor)
    return descriptor


def percentile(samples: list[int], percentage: int) -> int:
    ordered = sorted(samples)
    rank = (len(ordered) * percentage + 99) // 100
    return ordered[max(0, rank - 1)]


def load_ndjson(path: Path, label: str) -> list[dict[str, Any]]:
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise InputError(f"cannot read {label} {path}: {error}") from error
    records: list[dict[str, Any]] = []
    for number, line in enumerate(lines, 1):
        if not line.strip():
            continue
        try:
            raw = json.loads(line)
        except json.JSONDecodeError as error:
            raise InputError(f"invalid {label} NDJSON line {number}: {error}") from error
        if not isinstance(raw, dict):
            raise InputError(f"{label} line {number} is not a JSON object")
        records.append(cast(dict[str, Any], raw))
    if not records:
        raise InputError(f"{label} result is empty")
    return records


@dataclass(frozen=True)
class ResultSet:
    run: dict[str, Any]
    corpora: dict[str, dict[str, Any]]
    measurements: dict[tuple[str, str], dict[str, Any]]


def require_nonnegative_integer(value: object, label: str) -> int:
    if type(value) is not int or value < 0:
        raise InputError(f"{label} must be a nonnegative integer")
    return value


def validate_tool_descriptor(value: object, label: str) -> dict[str, str]:
    if not isinstance(value, dict):
        raise InputError(f"{label} has invalid tool provenance")
    descriptor = cast(dict[str, object], value)
    if set(descriptor) != {"path", "sha256", "version"}:
        raise InputError(f"{label} has invalid tool provenance")
    path_value = descriptor.get("path")
    digest = descriptor.get("sha256")
    version = descriptor.get("version")
    if (
        not isinstance(path_value, str)
        or not Path(path_value).is_absolute()
        or not isinstance(digest, str)
        or len(digest) != 64
        or any(character not in "0123456789abcdef" for character in digest)
        or not isinstance(version, str)
        or not version
    ):
        raise InputError(f"{label} has invalid tool provenance")
    path = Path(path_value)
    if not path.is_file() or file_hash(path) != digest:
        raise InputError(f"{label} tool executable changed or is absent")
    return {"path": path_value, "sha256": digest, "version": version}


def validate_rust_toolchain(
    environment: dict[str, Any], protocol: dict[str, Any], label: str
) -> None:
    value = environment.get("rust_toolchain")
    if not isinstance(value, dict):
        raise InputError(f"{label} has no pinned Rust toolchain provenance")
    toolchain = cast(dict[str, object], value)
    if set(toolchain) != {
        "channel",
        "rustup",
        "cargo",
        "rustc",
    }:
        raise InputError(f"{label} has no pinned Rust toolchain provenance")
    channel = protocol["candidate_rust_toolchain"]
    if toolchain.get("channel") != channel:
        raise InputError(f"{label} used the wrong Rust toolchain")
    validate_tool_descriptor(toolchain.get("rustup"), f"{label} rustup")
    cargo = validate_tool_descriptor(toolchain.get("cargo"), f"{label} Cargo")
    rustc = validate_tool_descriptor(toolchain.get("rustc"), f"{label} rustc")
    if not cargo["version"].startswith(
        f"cargo {channel} "
    ) or not rustc["version"].startswith(f"rustc {channel} "):
        raise InputError(f"{label} used the wrong Rust toolchain")


def validate_result(
    records: list[dict[str, Any]],
    *,
    label: str,
    implementation: str,
    protocol: dict[str, Any],
    protocol_hash: str,
    required_pin: str | None,
) -> ResultSet:
    for record in records:
        if record.get("schema") != RESULT_SCHEMA:
            raise InputError(f"{label} contains an incompatible result schema")
        if record.get("protocol_sha256") != protocol_hash:
            raise InputError(f"{label} contains an incompatible protocol hash")

    run_records = [record for record in records if record.get("kind") == "run"]
    if len(run_records) != 1:
        raise InputError(f"{label} must contain exactly one run record")
    run = run_records[0]
    if run.get("implementation") != implementation:
        raise InputError(f"{label} implementation is not {implementation!r}")
    source_pin = run.get("source_pin")
    if (
        not isinstance(source_pin, str)
        or len(source_pin) != 40
        or any(character not in "0123456789abcdef" for character in source_pin)
    ):
        raise InputError(f"{label} has no source pin")
    if required_pin is not None and source_pin != required_pin:
        raise InputError(f"{label} is not pinned to {required_pin}")
    dirty = run.get("dirty")
    if type(dirty) is not bool:
        raise InputError(f"{label} has no Git dirty-state metadata")
    if dirty:
        raise InputError(f"{label} has modifications beyond its source pin")
    runner_hash = run.get("runner_sha256")
    if (
        not isinstance(runner_hash, str)
        or len(runner_hash) != 64
        or any(character not in "0123456789abcdef" for character in runner_hash)
    ):
        raise InputError(f"{label} has no valid benchmark-runner hash")
    expected_runner_hash = file_hash(Path(__file__).with_name("benchmark-python.py"))
    if runner_hash != expected_runner_hash:
        raise InputError(f"{label} was not produced by the current benchmark runner")
    if not isinstance(run.get("environment"), dict):
        raise InputError(f"{label} has no environment metadata")
    expected_policy = benchmark_environment_policy(protocol)
    if run.get("environment_policy") != expected_policy:
        raise InputError(f"{label} did not bind the exact environment policy")
    environment = cast(dict[str, Any], run["environment"])
    if environment.get("python_isolated_mode") is not True:
        raise InputError(f"{label} did not use isolated Python mode")
    sanitized = environment.get("sanitized_environment")
    if not isinstance(sanitized, dict) or any(
        not isinstance(name, str) or not isinstance(value, str)
        for name, value in cast(dict[object, object], sanitized).items()
    ):
        raise InputError(f"{label} has no sanitized benchmark environment")
    sanitized_values = cast(dict[str, str], sanitized)
    violations = environment_policy_violations(expected_policy, sanitized_values)
    if violations:
        raise InputError(
            f"{label} used an invalid benchmark environment: " + "; ".join(violations)
        )
    if environment.get("sanitized_environment_sha256") != object_hash(
        sanitized_values
    ):
        raise InputError(f"{label} has an invalid benchmark environment hash")
    if implementation == "greatgramma-upstream-python":
        expected_versions = cast(
            dict[str, str],
            cast(dict[str, Any], protocol["upstream"])["python_dependencies"],
        )
        if environment.get("python_distribution_versions") != expected_versions:
            raise InputError(f"{label} did not use the exact upstream dependencies")
    else:
        validate_rust_toolchain(environment, protocol, label)
    if not isinstance(run.get("parameters"), dict):
        raise InputError(f"{label} has no benchmark parameters")
    parameters = cast(dict[str, Any], run["parameters"])
    sampling = release_sampling(protocol)
    if any(parameters.get(name) != value for name, value in sampling.items()):
        raise InputError(f"{label} did not use the release sample counts")

    corpora: dict[str, dict[str, Any]] = {}
    measurements: dict[tuple[str, str], dict[str, Any]] = {}
    known_operations = protocol["operations"]
    for record in records:
        kind = record.get("kind")
        if kind == "run":
            continue
        if kind == "corpus":
            corpus_id = record.get("corpus_id")
            vocab_size = record.get("vocab_size")
            if not isinstance(corpus_id, str) or corpus_id in corpora:
                raise InputError(f"{label} has an invalid or duplicate corpus")
            if type(vocab_size) is not int:
                raise InputError(f"{label} corpus {corpus_id!r} has no vocabulary size")
            expected = expected_corpus(protocol, protocol_hash, vocab_size)
            if any(record.get(key) != value for key, value in expected.items()):
                raise InputError(f"{label} corpus {corpus_id!r} fails hash validation")
            corpora[corpus_id] = record
            continue
        if kind == "timing":
            corpus_id = record.get("corpus_id")
            operation = record.get("operation")
            if not isinstance(corpus_id, str) or not isinstance(operation, str):
                raise InputError(f"{label} has a timing without an operation/corpus")
            if operation not in known_operations:
                raise InputError(f"{label} has unregistered operation {operation!r}")
            key = (corpus_id, operation)
            if key in measurements:
                raise InputError(f"{label} has duplicate timing {key!r}")
            raw_samples = record.get("samples_ns")
            if not isinstance(raw_samples, list) or not raw_samples:
                raise InputError(f"{label} timing {key!r} has invalid raw samples")
            sample_values = cast(list[object], raw_samples)
            if any(type(sample) is not int or sample < 0 for sample in sample_values):
                raise InputError(f"{label} timing {key!r} has invalid raw samples")
            samples = cast(list[int], sample_values)
            expected_samples = (
                sampling["compile_samples"]
                if operation == "compile_prepare"
                else sampling["operation_samples"]
            )
            expected_warmup = (
                min(1, sampling["warmup_count"])
                if operation == "compile_prepare"
                else sampling["warmup_count"]
            )
            if (
                record.get("sample_count") != len(samples)
                or len(samples) != expected_samples
            ):
                raise InputError(f"{label} timing {key!r} has a bad sample count")
            if record.get("warmup_count") != expected_warmup:
                raise InputError(f"{label} timing {key!r} has a bad warm-up count")
            for percentage in (50, 95, 99):
                field = f"p{percentage}_ns"
                if record.get(field) != percentile(samples, percentage):
                    raise InputError(f"{label} timing {key!r} has an invalid {field}")
            measurements[key] = record
            continue
        if kind == "memory":
            corpus_id = record.get("corpus_id")
            operation = record.get("operation")
            if not isinstance(corpus_id, str) or not isinstance(operation, str):
                raise InputError(f"{label} has a memory record without an operation/corpus")
            if operation not in known_operations:
                raise InputError(f"{label} has unregistered operation {operation!r}")
            key = (corpus_id, operation)
            if key in measurements:
                raise InputError(f"{label} has duplicate measurement {key!r}")
            initial = require_nonnegative_integer(
                record.get("initial_peak_rss_bytes"), "initial_peak_rss_bytes"
            )
            peak = require_nonnegative_integer(record.get("peak_rss_bytes"), "peak_rss_bytes")
            incremental = require_nonnegative_integer(
                record.get("incremental_peak_rss_bytes"), "incremental_peak_rss_bytes"
            )
            if peak < initial or incremental != peak - initial:
                raise InputError(f"{label} memory measurement {key!r} is inconsistent")
            measurements[key] = record
            continue
        raise InputError(f"{label} has unsupported record kind {kind!r}")
    if not corpora or not measurements:
        raise InputError(f"{label} is missing corpus or measurement records")
    if any(corpus_id not in corpora for corpus_id, _ in measurements):
        raise InputError(f"{label} has a measurement for an unknown corpus")
    return ResultSet(run=run, corpora=corpora, measurements=measurements)


def load_thresholds(path: Path, protocol_hash: str) -> list[dict[str, Any]]:
    value = load_object(path, "threshold configuration")
    if value.get("schema") != THRESHOLD_SCHEMA:
        raise InputError("unsupported threshold schema")
    if value.get("baseline_source_pin") != UPSTREAM_PIN:
        raise InputError("thresholds do not select the frozen upstream revision")
    if value.get("protocol_sha256") != protocol_hash:
        raise InputError("thresholds do not select the exact corpus protocol")
    raw_comparisons = value.get("comparisons")
    if not isinstance(raw_comparisons, list) or not raw_comparisons:
        raise InputError("threshold configuration contains no comparisons")
    raw_rules = cast(list[object], raw_comparisons)
    if any(not isinstance(rule, dict) for rule in raw_rules):
        raise InputError("threshold configuration contains a non-object comparison")
    return cast(list[dict[str, Any]], raw_rules)


def compare(
    candidate: ResultSet,
    baseline: ResultSet,
    comparisons: list[dict[str, Any]],
    protocol_hash: str,
) -> tuple[list[dict[str, Any]], bool]:
    if candidate.run["runner_sha256"] != baseline.run["runner_sha256"]:
        raise InputError("candidate and baseline used different benchmark adapters")
    candidate_environment = candidate.run["environment"]
    baseline_environment = baseline.run["environment"]
    for field in (
        "python_version",
        "python_implementation",
        "python_executable",
        "platform",
        "system",
        "release",
        "machine",
        "processor",
        "logical_cpu_count",
        "total_memory_bytes",
        "timer",
        "garbage_collector_during_measurement",
        "sanitized_environment",
        "sanitized_environment_sha256",
    ):
        if candidate_environment.get(field) is None or candidate_environment.get(
            field
        ) != baseline_environment.get(field):
            raise InputError(f"candidate and baseline environments differ at {field!r}")
    if candidate.corpora.keys() != baseline.corpora.keys():
        raise InputError("candidate and baseline corpus sets differ")
    for corpus_id in candidate.corpora:
        candidate_hash = candidate.corpora[corpus_id].get("corpus_sha256")
        baseline_hash = baseline.corpora[corpus_id].get("corpus_sha256")
        if candidate_hash != baseline_hash:
            raise InputError(f"candidate and baseline corpus {corpus_id!r} differ")

    output: list[dict[str, Any]] = []
    seen: set[tuple[str, str, str]] = set()
    all_passed = True
    for index, rule in enumerate(comparisons):
        corpus_id = rule.get("corpus_id")
        operation = rule.get("operation")
        metric = rule.get("metric")
        maximum = rule.get("max_candidate_over_baseline")
        if (
            not isinstance(corpus_id, str)
            or not isinstance(operation, str)
            or metric
            not in (
                "p50_ns",
                "p95_ns",
                "p99_ns",
                "peak_rss_bytes",
                "incremental_peak_rss_bytes",
            )
            or type(maximum) not in (int, float)
        ):
            raise InputError(f"comparison {index} is invalid")
        maximum_ratio = float(cast(int | float, maximum))
        if not math.isfinite(maximum_ratio) or maximum_ratio <= 0.0:
            raise InputError(f"comparison {index} is invalid")
        metric_name = cast(str, metric)
        comparison_key = (corpus_id, operation, metric_name)
        if comparison_key in seen:
            raise InputError(f"duplicate comparison {comparison_key!r}")
        seen.add(comparison_key)
        timing_key = (corpus_id, operation)
        if (
            timing_key not in candidate.measurements
            or timing_key not in baseline.measurements
        ):
            raise InputError(f"comparison measurement {timing_key!r} is absent")
        candidate_record = candidate.measurements[timing_key]
        baseline_record = baseline.measurements[timing_key]
        if candidate_record.get("kind") != baseline_record.get("kind"):
            raise InputError(f"comparison measurement kinds for {timing_key!r} differ")
        if candidate_record.get("kind") == "timing" and any(
            candidate_record.get(field) != baseline_record.get(field)
            for field in ("sample_count", "warmup_count")
        ):
            raise InputError(f"comparison sampling parameters for {timing_key!r} differ")
        candidate_value = candidate_record.get(metric_name)
        baseline_value = baseline_record.get(metric_name)
        if (
            type(candidate_value) is not int
            or type(baseline_value) is not int
            or baseline_value <= 0
        ):
            raise InputError(f"comparison metric {comparison_key!r} is invalid")
        ratio = candidate_value / baseline_value
        passed = ratio <= maximum_ratio
        all_passed = all_passed and passed
        output.append(
            {
                "schema": COMPARISON_SCHEMA,
                "kind": "comparison",
                "protocol_sha256": protocol_hash,
                "baseline_source_pin": UPSTREAM_PIN,
                "candidate_source_pin": candidate.run["source_pin"],
                "candidate_dirty": candidate.run["dirty"],
                "corpus_id": corpus_id,
                "operation": operation,
                "metric": metric_name,
                "candidate_value": candidate_value,
                "baseline_value": baseline_value,
                "candidate_over_baseline": ratio,
                "maximum_candidate_over_baseline": maximum_ratio,
                "passed": passed,
            }
        )
    output.append(
        {
            "schema": COMPARISON_SCHEMA,
            "kind": "summary",
            "protocol_sha256": protocol_hash,
            "baseline_source_pin": UPSTREAM_PIN,
            "candidate_source_pin": candidate.run["source_pin"],
            "candidate_dirty": candidate.run["dirty"],
            "comparison_count": len(comparisons),
            "passed": all_passed,
        }
    )
    return output, all_passed


def emit(records: Iterable[dict[str, Any]], output: TextIO) -> None:
    for record in records:
        print(json.dumps(record, sort_keys=True, separators=(",", ":")), file=output)


def self_test(protocol_path: Path) -> None:
    protocol, protocol_hash = load_protocol(protocol_path)
    policy = enforce_environment_policy(protocol)
    corpus = expected_corpus(protocol, protocol_hash, 257)
    sanitized_environment = {
        "GREATGRAMMA_BENCHMARK_SANITIZED": "1",
        "HOME": "/home/benchmark-self-test",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
        "TMPDIR": "/tmp",
    }
    current_runner_hash = file_hash(Path(__file__).with_name("benchmark-python.py"))
    tool_path = str(Path(sys.executable).resolve())
    tool_sha256 = file_hash(Path(tool_path))

    def result(implementation: str, pin: str, samples: list[int]) -> list[dict[str, Any]]:
        environment: dict[str, Any] = {
            "python_version": "3.11.0",
            "python_implementation": "CPython",
            "python_executable": "/benchmark/python",
            "python_isolated_mode": True,
            "platform": "SelfTest-1-test",
            "system": "SelfTest",
            "release": "1",
            "machine": "test",
            "processor": "test",
            "logical_cpu_count": 1,
            "total_memory_bytes": 1,
            "timer": "time.perf_counter_ns",
            "garbage_collector_during_measurement": "disabled",
            "sanitized_environment": dict(sanitized_environment),
            "sanitized_environment_sha256": object_hash(sanitized_environment),
        }
        if implementation == "greatgramma-upstream-python":
            environment["python_distribution_versions"] = cast(
                dict[str, Any], protocol["upstream"]
            )["python_dependencies"]
        else:
            environment["rust_toolchain"] = {
                "channel": "1.98.0",
                "rustup": {
                    "path": tool_path,
                    "sha256": tool_sha256,
                    "version": "rustup self-test",
                },
                "cargo": {
                    "path": tool_path,
                    "sha256": tool_sha256,
                    "version": "cargo 1.98.0 (self-test)",
                },
                "rustc": {
                    "path": tool_path,
                    "sha256": tool_sha256,
                    "version": "rustc 1.98.0 (self-test)",
                },
            }
        timing = {
            "schema": RESULT_SCHEMA,
            "kind": "timing",
            "protocol_sha256": protocol_hash,
            "corpus_id": corpus["corpus_id"],
            "operation": "processor_advance_and_mask",
            "sample_count": len(samples),
            "warmup_count": release_sampling(protocol)["warmup_count"],
            "samples_ns": samples,
            "p50_ns": percentile(samples, 50),
            "p95_ns": percentile(samples, 95),
            "p99_ns": percentile(samples, 99),
        }
        memory = {
            "schema": RESULT_SCHEMA,
            "kind": "memory",
            "protocol_sha256": protocol_hash,
            "corpus_id": corpus["corpus_id"],
            "operation": "compile_prepare_peak_rss",
            "initial_peak_rss_bytes": 10,
            "peak_rss_bytes": 50 if implementation == "greatgramma-rust" else 100,
            "incremental_peak_rss_bytes": 40
            if implementation == "greatgramma-rust"
            else 90,
        }
        return [
            {
                "schema": RESULT_SCHEMA,
                "kind": "run",
                "protocol_sha256": protocol_hash,
                "implementation": implementation,
                "source_pin": pin,
                "dirty": False,
                "runner_sha256": current_runner_hash,
                "environment_policy": policy,
                "environment": environment,
                "parameters": {
                    "self_test": True,
                    **release_sampling(protocol),
                },
            },
            {"schema": RESULT_SCHEMA, "kind": "corpus", **corpus},
            timing,
            memory,
        ]

    candidate_records = result(
        "greatgramma-rust", "0" * 40, [9] * release_sampling(protocol)["operation_samples"]
    )
    baseline_records = result(
        "greatgramma-upstream-python",
        UPSTREAM_PIN,
        [10] * release_sampling(protocol)["operation_samples"],
    )
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        candidate_path = root / "candidate.ndjson"
        baseline_path = root / "baseline.ndjson"
        threshold_path = root / "thresholds.json"
        candidate_path.write_text(
            "\n".join(json.dumps(record) for record in candidate_records) + "\n",
            encoding="utf-8",
        )
        baseline_path.write_text(
            "\n".join(json.dumps(record) for record in baseline_records) + "\n",
            encoding="utf-8",
        )
        threshold_path.write_text(
            json.dumps(
                {
                    "schema": THRESHOLD_SCHEMA,
                    "baseline_source_pin": UPSTREAM_PIN,
                    "protocol_sha256": protocol_hash,
                    "comparisons": [
                        {
                            "corpus_id": corpus["corpus_id"],
                            "operation": "processor_advance_and_mask",
                            "metric": "p95_ns",
                            "max_candidate_over_baseline": 0.9,
                        },
                        {
                            "corpus_id": corpus["corpus_id"],
                            "operation": "compile_prepare_peak_rss",
                            "metric": "peak_rss_bytes",
                            "max_candidate_over_baseline": 0.9,
                        }
                    ],
                }
            ),
            encoding="utf-8",
        )
        candidate = validate_result(
            load_ndjson(candidate_path, "candidate"),
            label="candidate",
            implementation="greatgramma-rust",
            protocol=protocol,
            protocol_hash=protocol_hash,
            required_pin=None,
        )
        baseline = validate_result(
            load_ndjson(baseline_path, "baseline"),
            label="baseline",
            implementation="greatgramma-upstream-python",
            protocol=protocol,
            protocol_hash=protocol_hash,
            required_pin=UPSTREAM_PIN,
        )
        rules = load_thresholds(threshold_path, protocol_hash)
        _, passed = compare(candidate, baseline, rules, protocol_hash)
        if not passed:
            raise AssertionError("passing self-test comparison failed")
        rules[0]["max_candidate_over_baseline"] = 0.8
        _, passed = compare(candidate, baseline, rules, protocol_hash)
        if passed:
            raise AssertionError("threshold miss passed the self-test comparison")
        baseline_records[0]["runner_sha256"] = "1" * 64
        baseline_path.write_text(
            "\n".join(json.dumps(record) for record in baseline_records) + "\n",
            encoding="utf-8",
        )
        try:
            validate_result(
                load_ndjson(baseline_path, "baseline"),
                label="baseline",
                implementation="greatgramma-upstream-python",
                protocol=protocol,
                protocol_hash=protocol_hash,
                required_pin=UPSTREAM_PIN,
            )
        except InputError:
            pass
        else:
            raise AssertionError("baseline from a stale runner was accepted")
        baseline_records[0]["runner_sha256"] = current_runner_hash
        baseline_parameters = cast(dict[str, Any], baseline_records[0]["parameters"])
        baseline_parameters["operation_samples"] = 1
        baseline_path.write_text(
            "\n".join(json.dumps(record) for record in baseline_records) + "\n",
            encoding="utf-8",
        )
        try:
            validate_result(
                load_ndjson(baseline_path, "baseline"),
                label="baseline",
                implementation="greatgramma-upstream-python",
                protocol=protocol,
                protocol_hash=protocol_hash,
                required_pin=UPSTREAM_PIN,
            )
        except InputError:
            pass
        else:
            raise AssertionError("baseline with reduced sampling was accepted")
        baseline_parameters["operation_samples"] = release_sampling(protocol)[
            "operation_samples"
        ]
        candidate_environment = cast(dict[str, Any], candidate_records[0]["environment"])
        candidate_toolchain = cast(
            dict[str, Any], candidate_environment["rust_toolchain"]
        )
        candidate_cargo = cast(dict[str, str], candidate_toolchain["cargo"])
        candidate_cargo_version = candidate_cargo["version"]
        candidate_cargo["version"] = "cargo 0.0.0 (wrong)"
        candidate_path.write_text(
            "\n".join(json.dumps(record) for record in candidate_records) + "\n",
            encoding="utf-8",
        )
        try:
            validate_result(
                load_ndjson(candidate_path, "candidate"),
                label="candidate",
                implementation="greatgramma-rust",
                protocol=protocol,
                protocol_hash=protocol_hash,
                required_pin=None,
            )
        except InputError:
            pass
        else:
            raise AssertionError("candidate with the wrong Rust toolchain was accepted")
        candidate_cargo["version"] = candidate_cargo_version
        baseline_environment = cast(dict[str, Any], baseline_records[0]["environment"])
        hostile_environment = cast(
            dict[str, str], baseline_environment["sanitized_environment"]
        )
        hostile_environment["CPATH"] = "/hostile/include"
        baseline_environment["sanitized_environment_sha256"] = object_hash(
            hostile_environment
        )
        baseline_path.write_text(
            "\n".join(json.dumps(record) for record in baseline_records) + "\n",
            encoding="utf-8",
        )
        try:
            validate_result(
                load_ndjson(baseline_path, "baseline"),
                label="baseline",
                implementation="greatgramma-upstream-python",
                protocol=protocol,
                protocol_hash=protocol_hash,
                required_pin=UPSTREAM_PIN,
            )
        except InputError:
            pass
        else:
            raise AssertionError("baseline with environment overrides was accepted")
        hostile_environment.pop("CPATH")
        baseline_environment["sanitized_environment_sha256"] = object_hash(
            hostile_environment
        )
        baseline_records[0]["environment_policy"] = {
            "allowed_names": [],
            "required_names": [],
            "required_values": {},
        }
        baseline_path.write_text(
            "\n".join(json.dumps(record) for record in baseline_records) + "\n",
            encoding="utf-8",
        )
        try:
            validate_result(
                load_ndjson(baseline_path, "baseline"),
                label="baseline",
                implementation="greatgramma-upstream-python",
                protocol=protocol,
                protocol_hash=protocol_hash,
                required_pin=UPSTREAM_PIN,
            )
        except InputError:
            pass
        else:
            raise AssertionError("baseline with a forged environment policy was accepted")
        baseline_records[0]["environment_policy"] = policy
        baseline_environment["python_distribution_versions"] = {"Cython": "wrong"}
        baseline_path.write_text(
            "\n".join(json.dumps(record) for record in baseline_records) + "\n",
            encoding="utf-8",
        )
        try:
            validate_result(
                load_ndjson(baseline_path, "baseline"),
                label="baseline",
                implementation="greatgramma-upstream-python",
                protocol=protocol,
                protocol_hash=protocol_hash,
                required_pin=UPSTREAM_PIN,
            )
        except InputError:
            pass
        else:
            raise AssertionError("baseline with dependency drift was accepted")
        baseline_environment["python_distribution_versions"] = cast(
            dict[str, Any], protocol["upstream"]
        )["python_dependencies"]
        baseline_records[0]["source_pin"] = "unpinned"
        baseline_path.write_text(
            "\n".join(json.dumps(record) for record in baseline_records) + "\n",
            encoding="utf-8",
        )
        try:
            validate_result(
                load_ndjson(baseline_path, "baseline"),
                label="baseline",
                implementation="greatgramma-upstream-python",
                protocol=protocol,
                protocol_hash=protocol_hash,
                required_pin=UPSTREAM_PIN,
            )
        except InputError:
            pass
        else:
            raise AssertionError("unpinned baseline was accepted")
    print("benchmark comparison self-test: ok")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--thresholds", type=Path)
    parser.add_argument("--protocol", type=Path, default=DEFAULT_PROTOCOL)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    if not args.self_test:
        missing = [
            flag
            for flag, value in (
                ("--candidate", args.candidate),
                ("--baseline", args.baseline),
                ("--thresholds", args.thresholds),
            )
            if value is None
        ]
        if missing:
            parser.error(f"required arguments: {', '.join(missing)}")
    return args


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        if args.self_test:
            self_test(args.protocol)
            return 0
        protocol, protocol_hash = load_protocol(args.protocol)
        enforce_environment_policy(protocol)
        candidate = validate_result(
            load_ndjson(args.candidate, "candidate"),
            label="candidate",
            implementation="greatgramma-rust",
            protocol=protocol,
            protocol_hash=protocol_hash,
            required_pin=None,
        )
        baseline = validate_result(
            load_ndjson(args.baseline, "baseline"),
            label="baseline",
            implementation="greatgramma-upstream-python",
            protocol=protocol,
            protocol_hash=protocol_hash,
            required_pin=UPSTREAM_PIN,
        )
        comparisons = load_thresholds(args.thresholds, protocol_hash)
        records, passed = compare(candidate, baseline, comparisons, protocol_hash)
        if args.output is None:
            emit(records, sys.stdout)
        else:
            with args.output.open("w", encoding="utf-8", newline="\n") as output:
                emit(records, output)
        return 0 if passed else 1
    except (InputError, OSError) as error:
        print(f"benchmark comparison refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
