#!/usr/bin/env python3
"""Run deterministic candidate or frozen-upstream benchmarks as NDJSON.

An upstream result is produced only when an explicit clean checkout matches the
frozen revision. Comparative claims still require benchmark-compare.py, which
validates both complete result sets and a committed threshold policy.
"""

from __future__ import annotations

import sys

if not sys.flags.isolated:
    raise SystemExit("benchmark runner requires scripts/benchmark-python.sh")

import argparse
import contextlib
import gc
import hashlib
import importlib
import importlib.metadata
import io
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tempfile
import time
from collections.abc import Generator, Mapping
from pathlib import Path
from pathlib import PurePosixPath
from typing import Any, Callable, Iterable, TextIO, cast


RESULT_SCHEMA = "greatgramma.python-benchmark.v1"
PROTOCOL_SCHEMA = "greatgramma.synthetic-benchmark-protocol.v1"
CORPUS_SCHEMA = "greatgramma.synthetic-exact-byte-corpus.v1"
UPSTREAM_PIN = "4c21981386fc6d457efa381d1eb1863623d50fa1"
DEFAULT_PROTOCOL = (
    Path(__file__).resolve().parents[1]
    / "tests"
    / "fixtures"
    / "benchmark"
    / "protocol-v1.json"
)


class BenchmarkError(RuntimeError):
    """The benchmark could not produce a complete, comparable result."""


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
        raise BenchmarkError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def benchmark_environment_policy(protocol: dict[str, Any]) -> dict[str, Any]:
    raw_value = protocol.get("benchmark_environment")
    if not isinstance(raw_value, dict):
        raise BenchmarkError("protocol has no exact benchmark environment policy")
    raw = cast(dict[str, object], raw_value)
    if set(raw) != {
        "allowed_names",
        "required_names",
        "required_values",
    }:
        raise BenchmarkError("protocol has no exact benchmark environment policy")
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
        raise BenchmarkError("protocol has no exact benchmark environment policy")
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
        raise BenchmarkError("protocol has no exact benchmark environment policy")
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
        raise BenchmarkError("protocol does not select the release sample counts")
    return expected


def environment_policy_violations(
    policy: dict[str, Any], environment: Mapping[str, str]
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
    violations = environment_policy_violations(policy, os.environ)
    if violations:
        raise BenchmarkError(
            "benchmark must use scripts/benchmark-python.sh: "
            + "; ".join(violations)
        )
    return policy


def child_process_environment(*, virtual_environment: bool = False) -> dict[str, str]:
    environment = dict(os.environ)
    if virtual_environment:
        environment["VIRTUAL_ENV"] = str(Path(sys.executable).parent.parent)
    return environment


def path_exists(path: Path) -> bool:
    return path.exists() or path.is_symlink()


@contextlib.contextmanager
def isolated_build_environment(
    *,
    rust_workspace: Path | None = None,
    cargo: Path | None = None,
    rustc: Path | None = None,
) -> Generator[dict[str, str], None, None]:
    """Hide user build configuration while retaining explicit Rust caches."""

    with tempfile.TemporaryDirectory(prefix="greatgramma-benchmark-home-") as directory:
        environment = child_process_environment(
            virtual_environment=rust_workspace is not None
        )
        environment["HOME"] = directory
        if rust_workspace is not None:
            if cargo is None or rustc is None:
                raise BenchmarkError("candidate build has no pinned Rust tools")
            original_home = Path(os.environ["HOME"]).resolve()
            cargo_home = original_home / ".cargo"
            if not cargo_home.is_dir():
                raise BenchmarkError(
                    "candidate benchmark requires a populated $HOME/.cargo cache"
                )
            user_configs = (cargo_home / "config", cargo_home / "config.toml")
            if any(path_exists(path) for path in user_configs):
                raise BenchmarkError(
                    "candidate benchmark refuses user Cargo configuration"
                )
            for parent in rust_workspace.resolve().parents:
                parent_configs = (
                    parent / ".cargo" / "config",
                    parent / ".cargo" / "config.toml",
                )
                if any(path_exists(path) for path in parent_configs):
                    raise BenchmarkError(
                        "candidate benchmark refuses Cargo configuration outside source"
                    )
            environment["CARGO_HOME"] = str(cargo_home)
            environment["CARGO_NET_OFFLINE"] = "true"
            environment["CARGO"] = str(cargo)
            environment["RUSTC"] = str(rustc)
            rustup_home = original_home / ".rustup"
            if rustup_home.is_dir():
                environment["RUSTUP_HOME"] = str(rustup_home)
            try:
                yield environment
            finally:
                if any(path_exists(path) for path in user_configs):
                    raise BenchmarkError("user Cargo configuration changed during build")
        else:
            yield environment


def source_state(repo_root: Path) -> tuple[str, str]:
    commit = command_output(["git", "rev-parse", "HEAD"], repo_root)
    status = command_output(
        ["git", "status", "--porcelain", "--untracked-files=all"], repo_root
    )
    if (
        commit is None
        or len(commit) != 40
        or any(character not in "0123456789abcdef" for character in commit)
        or status is None
    ):
        raise BenchmarkError(f"cannot resolve source state for {repo_root}")
    return commit, status


@contextlib.contextmanager
def materialized_commit(
    repo_root: Path, *, expected_pin: str | None = None
) -> Generator[tuple[Path, str], None, None]:
    """Yield a fresh tracked-files-only tree for one clean source commit."""

    repo_root = repo_root.resolve()
    commit, status = source_state(repo_root)
    if expected_pin is not None and commit != expected_pin:
        raise BenchmarkError(f"source checkout is not pinned to {expected_pin}")
    if status:
        raise BenchmarkError("benchmark source checkout must have no tracked or untracked changes")
    archive = subprocess.run(
        ["git", "archive", "--format=tar", commit],
        cwd=repo_root,
        env=child_process_environment(),
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if archive.returncode != 0:
        detail = archive.stderr.decode("utf-8", errors="replace").strip()
        raise BenchmarkError(f"cannot materialize source commit: {detail}")
    with tempfile.TemporaryDirectory(prefix="greatgramma-benchmark-source-") as directory:
        destination = Path(directory)
        try:
            with tarfile.open(fileobj=io.BytesIO(archive.stdout), mode="r:") as source:
                for member in source:
                    relative = PurePosixPath(member.name)
                    if (
                        relative.is_absolute()
                        or not relative.parts
                        or ".." in relative.parts
                    ):
                        raise BenchmarkError("Git archive contains an unsafe path")
                    target = destination.joinpath(*relative.parts)
                    if member.isdir():
                        target.mkdir(parents=True, exist_ok=True)
                    elif member.isfile():
                        target.parent.mkdir(parents=True, exist_ok=True)
                        contents = source.extractfile(member)
                        if contents is None:
                            raise BenchmarkError("Git archive contains an unreadable file")
                        with target.open("wb") as output:
                            block = contents.read(1024 * 1024)
                            while block:
                                output.write(block)
                                block = contents.read(1024 * 1024)
                        target.chmod(member.mode & 0o777)
                    else:
                        raise BenchmarkError("Git archive contains an unsupported entry")
        except (OSError, tarfile.TarError) as error:
            raise BenchmarkError(f"cannot extract source commit: {error}") from error
        try:
            yield destination, commit
        finally:
            final_commit, final_status = source_state(repo_root)
            if final_commit != commit or final_status != status:
                raise BenchmarkError("benchmark source checkout changed during the run")


def load_protocol(path: Path) -> tuple[dict[str, Any], str]:
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise BenchmarkError(f"cannot load benchmark protocol {path}: {error}") from error
    if not isinstance(raw, dict):
        raise BenchmarkError("benchmark protocol must be one JSON object")
    value = cast(dict[str, Any], raw)
    if value.get("schema") != PROTOCOL_SCHEMA:
        raise BenchmarkError("unsupported benchmark protocol schema")
    if value.get("result_schema") != RESULT_SCHEMA:
        raise BenchmarkError("protocol selects an unsupported result schema")
    if value.get("corpus_schema") != CORPUS_SCHEMA:
        raise BenchmarkError("protocol selects an unsupported corpus schema")
    if value.get("python_isolated_mode_required") is not True:
        raise BenchmarkError("protocol does not require isolated Python mode")
    if value.get("candidate_rust_toolchain") != "1.98.0":
        raise BenchmarkError("protocol does not select the pinned Rust toolchain")
    release_sampling(value)
    if not isinstance(value.get("grammar"), dict):
        raise BenchmarkError("protocol has no grammar definition")
    if not isinstance(value.get("upstream_grammar"), dict) or not isinstance(
        value.get("language"), str
    ):
        raise BenchmarkError("protocol has no shared upstream grammar semantics")
    if not isinstance(value.get("operations"), dict):
        raise BenchmarkError("protocol has no operation registry")
    upstream = value.get("upstream")
    if not isinstance(upstream, dict) or cast(dict[str, Any], upstream).get(
        "source_pin"
    ) != UPSTREAM_PIN:
        raise BenchmarkError("protocol does not select the frozen upstream revision")
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
        raise BenchmarkError("protocol has no exact upstream dependency set")
    benchmark_environment_policy(value)
    return value, object_hash(value)


def generated_token(token_id: int) -> bytes:
    if token_id < 256:
        return bytes((token_id,))
    return b"\xa5" + token_id.to_bytes(4, "little")


def token_bytes(vocab_size: int) -> tuple[bytes, ...]:
    if not 257 <= vocab_size <= 1_000_000:
        raise BenchmarkError(f"vocabulary size {vocab_size} is outside [257, 1000000]")
    return tuple(generated_token(token_id) for token_id in range(vocab_size - 1)) + (
        b"",
    )


def corpus_descriptor(
    protocol: dict[str, Any], protocol_hash: str, vocab_size: int
) -> dict[str, Any]:
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


def consume(value: object) -> int:
    if isinstance(value, bytes):
        if not value:
            return 0
        return len(value) + value[0] + value[-1]
    if type(value) is int:
        return int(value)
    if isinstance(value, dict):
        mapping = cast(dict[object, object], value)
        first = next(iter(mapping), 0)
        return len(mapping) + (first if type(first) is int else 0)
    return id(value) & 0xFFFF


def validate_packed_mask(value: object, vocab_size: int, allowed_count: int) -> None:
    if not isinstance(value, bytes) or not 0 <= allowed_count <= vocab_size:
        raise BenchmarkError("candidate returned a malformed packed mask")
    width = (vocab_size + 7) // 8
    full_bytes, tail_bits = divmod(allowed_count, 8)
    expected = b"\xff" * full_bytes
    if tail_bits:
        expected += bytes(((1 << tail_bits) - 1,))
    expected += b"\x00" * (width - len(expected))
    if value != expected:
        raise BenchmarkError("candidate acceptance mask disagrees with the byte corpus")


def validate_upstream_acceptance(value: object, allowed_count: int) -> None:
    if not isinstance(value, dict):
        raise BenchmarkError("upstream acceptance disagrees with the byte corpus")
    acceptance = cast(dict[object, object], value)
    if len(acceptance) != allowed_count:
        raise BenchmarkError("upstream acceptance disagrees with the byte corpus")
    if any(token not in acceptance for token in range(allowed_count)):
        raise BenchmarkError("upstream acceptance disagrees with the byte corpus")


def measure(
    operation: str,
    corpus: dict[str, Any],
    *,
    warmup_count: int,
    sample_count: int,
    callback: Callable[[], object],
    buffer_bytes: int | None = None,
    validator: Callable[[object], None] | None = None,
) -> dict[str, Any]:
    checksum = 0
    for _ in range(warmup_count):
        value = callback()
        if validator is not None:
            validator(value)
        checksum = (checksum + consume(value)) & 0xFFFF_FFFF_FFFF_FFFF
    samples: list[int] = []
    for _ in range(sample_count):
        started = time.perf_counter_ns()
        value = callback()
        samples.append(time.perf_counter_ns() - started)
        if validator is not None:
            validator(value)
        checksum = (checksum + consume(value)) & 0xFFFF_FFFF_FFFF_FFFF
    record: dict[str, Any] = {
        "schema": RESULT_SCHEMA,
        "kind": "timing",
        "protocol_sha256": corpus["protocol_sha256"],
        "corpus_id": corpus["corpus_id"],
        "operation": operation,
        "sample_count": sample_count,
        "warmup_count": warmup_count,
        "samples_ns": samples,
        "p50_ns": percentile(samples, 50),
        "p95_ns": percentile(samples, 95),
        "p99_ns": percentile(samples, 99),
        "elapsed_ns": sum(samples),
        "checksum": checksum,
    }
    if buffer_bytes is not None:
        record["packed_buffer_bytes"] = buffer_bytes
    return record


def candidate_compile_inputs(
    api: Any,
    grammar: dict[str, Any],
    manifest_bytes: tuple[bytes, ...],
) -> tuple[list[Any], Any]:
    terminals = [
        api.Terminal(spec["name"], spec["pattern"], spec["priority"])
        for spec in grammar["terminals"]
    ]
    manifest = api.TokenizerManifest(
        manifest_bytes,
        frozenset((len(manifest_bytes) - 1,)),
    )
    return terminals, manifest


def compile_corpus(
    api: Any,
    grammar: dict[str, Any],
    terminals: list[Any],
    manifest: Any,
) -> Any:
    return api.compile(
        grammar["source"],
        start_rule=grammar["start_rule"],
        terminals=terminals,
        tokenizer=manifest,
        ignored_terminals=grammar["ignored_terminals"],
    )


def upstream_vocabulary(vocab_size: int) -> dict[int, str]:
    vocabulary = {
        token_id: generated_token(token_id).decode("latin-1")
        for token_id in range(vocab_size - 1)
    }
    vocabulary[vocab_size - 1] = ""
    return vocabulary


def native_artifacts(directory: Path, module_name: str) -> list[Path]:
    return sorted(
        path.resolve()
        for suffix in (".so", ".pyd", ".dylib")
        for path in directory.glob(f"{module_name}*{suffix}")
        if path.is_file()
    )


def load_upstream(checkout: Path, *, rebuild: bool = True) -> Any:
    checkout = checkout.resolve()
    grammar_directory = checkout / "alignment" / "monitor" / "grammar"
    module_names = ("partial_lexer", "partial_parser")
    if rebuild:
        if any(native_artifacts(grammar_directory, name) for name in module_names):
            raise BenchmarkError("materialized upstream tree contains native artifacts")
        with isolated_build_environment() as build_environment:
            for module_name in module_names:
                relative_path = f"alignment/monitor/grammar/{module_name}.pyx"
                result = subprocess.run(
                    [
                        sys.executable,
                        "-I",
                        "-m",
                        "Cython.Build.Cythonize",
                        "-i",
                        "-f",
                        relative_path,
                    ],
                    cwd=checkout,
                    env=build_environment,
                    check=False,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                )
                if result.returncode != 0:
                    detail = (
                        result.stderr.strip()
                        or f"cythonize exited {result.returncode}"
                    )
                    raise BenchmarkError(
                        f"fresh upstream extension build failed for {relative_path}: {detail}"
                    )
    expected_native_paths: dict[str, Path] = {}
    for module_name in module_names:
        artifacts = native_artifacts(grammar_directory, module_name)
        if len(artifacts) != 1:
            raise BenchmarkError(
                f"upstream build produced {len(artifacts)} {module_name} artifacts"
            )
        expected_native_paths[module_name] = artifacts[0]
    sys.path.insert(0, str(checkout))
    try:
        module = importlib.import_module("alignment.monitor.grammar.cfg_monitor")
    except ImportError as error:
        raise BenchmarkError(
            "frozen upstream CFGMonitor is unavailable; install its pinned dependencies "
            "and build partial_lexer.pyx plus partial_parser.pyx in that checkout"
        ) from error
    module_file = module.__file__
    if not isinstance(module_file, str):
        raise BenchmarkError("frozen upstream CFGMonitor has no source path")
    module_path = Path(module_file).resolve()
    if checkout not in module_path.parents:
        raise BenchmarkError("CFGMonitor was imported from outside the frozen checkout")
    for module_name, expected_path in expected_native_paths.items():
        native_module = importlib.import_module(
            f"alignment.monitor.grammar.{module_name}"
        )
        native_file = native_module.__file__
        if not isinstance(native_file, str) or Path(native_file).resolve() != expected_path:
            raise BenchmarkError(
                f"upstream imported an unexpected {module_name} extension"
            )
    return module.CFGMonitor


def compile_upstream(
    monitor_type: Any,
    protocol: dict[str, Any],
    vocabulary: dict[int, str],
    eos_token_id: int,
) -> Any:
    return monitor_type(
        protocol["upstream_grammar"]["source"],
        vocabulary,
        eos_token_id,
        1,
    )


class Scores:
    def __init__(self, vocab_size: int) -> None:
        self.shape = (1, vocab_size)


class StepInput:
    def __init__(self, width: int, token_id: int) -> None:
        self.shape = (1, width)
        self.token_id = token_id

    def __getitem__(self, key: object) -> tuple[int]:
        if key != (slice(None), -1):
            raise IndexError(key)
        return (self.token_id,)


def next_token(step: int, ordinary_tokens: int) -> int:
    return (step * 2_654_435_761) % ordinary_tokens


def benchmark_corpus(
    api: Any,
    protocol: dict[str, Any],
    protocol_hash: str,
    vocab_size: int,
    *,
    compile_samples: int,
    operation_samples: int,
    warmup_count: int,
) -> list[dict[str, Any]]:
    corpus = corpus_descriptor(protocol, protocol_hash, vocab_size)
    corpus_record = {"schema": RESULT_SCHEMA, "kind": "corpus", **corpus}
    manifest_bytes = token_bytes(vocab_size)
    grammar = protocol["grammar"]
    terminals, manifest = candidate_compile_inputs(api, grammar, manifest_bytes)
    print(f"benchmarking Python/PyO3 layers at vocab={vocab_size}", file=sys.stderr)

    compile_record = measure(
        "compile_prepare",
        corpus,
        warmup_count=min(1, warmup_count),
        sample_count=compile_samples,
        callback=lambda: compile_corpus(api, grammar, terminals, manifest).vocab_size,
    )

    native_initial_compiled = compile_corpus(api, grammar, terminals, manifest)
    native_initial = native_initial_compiled._new_batch(1, None)
    initial_record = measure(
        "engine_initial_mask",
        corpus,
        warmup_count=warmup_count,
        sample_count=operation_samples,
        callback=native_initial.initial_masks,
        buffer_bytes=native_initial.mask_bytes,
        validator=lambda value: validate_packed_mask(value, vocab_size, vocab_size - 1),
    )

    native_advance_compiled = compile_corpus(api, grammar, terminals, manifest)
    native_advance = native_advance_compiled._new_batch(1, None)
    native_step = 0

    def advance_native() -> bytes:
        nonlocal native_step
        token_id = next_token(native_step, vocab_size - 1)
        native_step += 1
        return native_advance.advance_and_masks([token_id])

    native_record = measure(
        "engine_advance_and_mask",
        corpus,
        warmup_count=warmup_count,
        sample_count=operation_samples,
        callback=advance_native,
        buffer_bytes=native_advance.mask_bytes,
        validator=lambda value: validate_packed_mask(value, vocab_size, vocab_size),
    )

    processor_compiled = compile_corpus(api, grammar, terminals, manifest)
    scores = Scores(vocab_size)
    observed_checksum = 0
    processor_started = False

    def masker(value: object, packed: bytes, rows: int, size: int) -> object:
        nonlocal observed_checksum
        if rows != 1 or size != vocab_size or len(packed) != (vocab_size + 7) // 8:
            raise BenchmarkError("processor returned a malformed packed mask")
        validate_packed_mask(
            packed,
            vocab_size,
            vocab_size if processor_started else vocab_size - 1,
        )
        observed_checksum = (observed_checksum + consume(packed)) & 0xFFFF_FFFF_FFFF_FFFF
        return value

    processor = processor_compiled.logits_processor(
        ((0,),),
        _masker=masker,
        _trusted_fixed_append=True,
    )
    processor(((0,),), scores)
    processor_started = True
    processor_step = 0

    def advance_processor() -> int:
        nonlocal processor_step
        token_id = next_token(processor_step, vocab_size - 1)
        processor_step += 1
        processor(StepInput(processor_step + 1, token_id), scores)
        return observed_checksum

    processor_record = measure(
        "processor_advance_and_mask",
        corpus,
        warmup_count=warmup_count,
        sample_count=operation_samples,
        callback=advance_processor,
        buffer_bytes=(vocab_size + 7) // 8,
    )
    return [corpus_record, compile_record, initial_record, native_record, processor_record]


def benchmark_upstream_corpus(
    monitor_type: Any,
    protocol: dict[str, Any],
    protocol_hash: str,
    vocab_size: int,
    *,
    compile_samples: int,
    operation_samples: int,
    warmup_count: int,
) -> list[dict[str, Any]]:
    corpus = corpus_descriptor(protocol, protocol_hash, vocab_size)
    corpus_record = {"schema": RESULT_SCHEMA, "kind": "corpus", **corpus}
    vocabulary = upstream_vocabulary(vocab_size)
    eos_token_id = vocab_size - 1
    print(f"benchmarking frozen upstream at vocab={vocab_size}", file=sys.stderr)

    with open(os.devnull, "w", encoding="utf-8") as sink, contextlib.redirect_stdout(sink):
        compile_record = measure(
            "compile_prepare",
            corpus,
            warmup_count=min(1, warmup_count),
            sample_count=compile_samples,
            callback=lambda: len(
                compile_upstream(
                    monitor_type, protocol, vocabulary, eos_token_id
                ).state
            ),
        )

        initial_monitor = compile_upstream(
            monitor_type, protocol, vocabulary, eos_token_id
        )
        initial_acceptance = initial_monitor.initial_state.acceptance
        if len(initial_acceptance) != vocab_size - 1 or eos_token_id in initial_acceptance:
            raise BenchmarkError("upstream initial mask disagrees with the byte corpus")
        initial_record = measure(
            "engine_initial_mask",
            corpus,
            warmup_count=warmup_count,
            sample_count=operation_samples,
            callback=lambda: initial_monitor.initial_state.acceptance,
            validator=lambda value: validate_upstream_acceptance(
                value, vocab_size - 1
            ),
        )

        advance_monitor = compile_upstream(
            monitor_type, protocol, vocabulary, eos_token_id
        )
        upstream_state = advance_monitor.initial_state
        upstream_step = 0

        def advance_upstream() -> dict[int, Any]:
            nonlocal upstream_state, upstream_step
            token_id = next_token(upstream_step, vocab_size - 1)
            upstream_step += 1
            upstream_state = upstream_state.feed_token(token_id)
            acceptance = upstream_state.acceptance
            if eos_token_id not in acceptance:
                raise BenchmarkError("upstream lost EOS after a nonempty byte prefix")
            return acceptance

        advance_record = measure(
            "engine_advance_and_mask",
            corpus,
            warmup_count=warmup_count,
            sample_count=operation_samples,
            callback=advance_upstream,
            validator=lambda value: validate_upstream_acceptance(value, vocab_size),
        )
    return [corpus_record, compile_record, initial_record, advance_record]


def peak_rss_bytes() -> int:
    try:
        import resource
    except ImportError as error:
        raise BenchmarkError("peak RSS measurement is unavailable on this platform") from error
    value = int(resource.getrusage(resource.RUSAGE_SELF).ru_maxrss)
    return value if sys.platform == "darwin" else value * 1024


def rss_worker(
    implementation: str,
    protocol_path: Path,
    vocab_size: int,
    upstream_checkout: Path | None,
    materialized_checkout: Path | None,
) -> int:
    protocol, _ = load_protocol(protocol_path)
    enforce_environment_policy(protocol)
    if materialized_checkout is None:
        raise BenchmarkError("RSS worker has no materialized source checkout")
    source_root = materialized_checkout.resolve()
    gc.disable()
    if implementation == "candidate":
        sys.path.insert(0, str(source_root / "python"))
        api = importlib.import_module("greatgramma")
        manifest_bytes = token_bytes(vocab_size)
        grammar = protocol["grammar"]
        terminals, manifest = candidate_compile_inputs(api, grammar, manifest_bytes)
        initial = peak_rss_bytes()
        prepared = compile_corpus(api, grammar, terminals, manifest)
        if prepared.vocab_size != vocab_size:
            raise BenchmarkError("candidate RSS worker compiled the wrong vocabulary")
    elif implementation == "upstream" and upstream_checkout is not None:
        monitor_type = load_upstream(source_root, rebuild=False)
        vocabulary = upstream_vocabulary(vocab_size)
        initial = peak_rss_bytes()
        with open(os.devnull, "w", encoding="utf-8") as sink, contextlib.redirect_stdout(sink):
            prepared = compile_upstream(
                monitor_type, protocol, vocabulary, vocab_size - 1
            )
        if len(prepared.state) != 1:
            raise BenchmarkError("upstream RSS worker created the wrong row count")
    else:
        raise BenchmarkError("invalid RSS worker implementation")
    peak = peak_rss_bytes()
    print(
        json.dumps(
            {
                "initial_peak_rss_bytes": initial,
                "peak_rss_bytes": peak,
                "incremental_peak_rss_bytes": max(0, peak - initial),
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


def measure_preparation_rss(
    implementation: str,
    protocol_path: Path,
    corpus: dict[str, Any],
    upstream_checkout: Path | None,
    materialized_checkout: Path,
) -> dict[str, Any]:
    command = [
        sys.executable,
        "-I",
        str(Path(__file__).resolve()),
        "--_rss-worker",
        implementation,
        "--rss-vocab-size",
        str(corpus["vocab_size"]),
        "--protocol",
        str(protocol_path.resolve()),
        "--_materialized-checkout",
        str(materialized_checkout.resolve()),
    ]
    if upstream_checkout is not None:
        command.extend(("--upstream-checkout", str(upstream_checkout.resolve())))
    result = subprocess.run(
        command,
        env=child_process_environment(),
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or f"worker exited {result.returncode}"
        raise BenchmarkError(f"preparation peak RSS worker failed: {detail}")
    try:
        values = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise BenchmarkError("preparation peak RSS worker emitted invalid JSON") from error
    if not isinstance(values, dict):
        raise BenchmarkError("preparation peak RSS worker emitted a non-object")
    record = {
        "schema": RESULT_SCHEMA,
        "kind": "memory",
        "protocol_sha256": corpus["protocol_sha256"],
        "corpus_id": corpus["corpus_id"],
        "operation": "compile_prepare_peak_rss",
    }
    record.update(cast(dict[str, Any], values))
    return record


def command_output(arguments: list[str], cwd: Path) -> str | None:
    try:
        result = subprocess.run(
            arguments,
            cwd=cwd,
            env=child_process_environment(),
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except OSError:
        return None
    if result.returncode != 0:
        return None
    return result.stdout.strip()


def tool_descriptor(path: Path, version_arguments: list[str]) -> dict[str, str]:
    version = command_output([str(path), *version_arguments], Path.cwd())
    if version is None:
        raise BenchmarkError(f"cannot identify benchmark tool {path}")
    return {
        "path": str(path),
        "sha256": file_hash(path),
        "version": version,
    }


def pinned_rust_tools(repo_root: Path, protocol: dict[str, Any]) -> dict[str, Any]:
    channel = protocol.get("candidate_rust_toolchain")
    if channel != "1.98.0":
        raise BenchmarkError("candidate Rust toolchain is not pinned to 1.98.0")
    rustup_name = shutil.which("rustup", path=os.environ["PATH"])
    if rustup_name is None:
        raise BenchmarkError("candidate benchmark requires rustup on PATH")
    rustup = Path(rustup_name).resolve()
    cargo_name = command_output(
        [str(rustup), "which", "--toolchain", channel, "cargo"], repo_root
    )
    rustc_name = command_output(
        [str(rustup), "which", "--toolchain", channel, "rustc"], repo_root
    )
    if cargo_name is None or rustc_name is None:
        raise BenchmarkError(f"Rust toolchain {channel} is not installed")
    cargo = Path(cargo_name).resolve()
    rustc = Path(rustc_name).resolve()
    if not cargo.is_file() or not rustc.is_file():
        raise BenchmarkError("rustup returned a missing Cargo or rustc executable")
    provenance: dict[str, Any] = {
        "channel": channel,
        "rustup": tool_descriptor(rustup, ["--version"]),
        "cargo": tool_descriptor(cargo, ["--version", "--verbose"]),
        "rustc": tool_descriptor(rustc, ["--version", "--verbose"]),
    }
    cargo_version = cast(dict[str, str], provenance["cargo"])["version"]
    rustc_version = cast(dict[str, str], provenance["rustc"])["version"]
    if not cargo_version.startswith(f"cargo {channel} ") or not rustc_version.startswith(
        f"rustc {channel} "
    ):
        raise BenchmarkError(f"rustup did not resolve Rust toolchain {channel}")
    return provenance


def build_candidate(
    repo_root: Path, protocol: dict[str, Any]
) -> tuple[Path, dict[str, Any]]:
    package_directory = repo_root / "python" / "greatgramma"
    if native_artifacts(package_directory, "_native"):
        raise BenchmarkError("materialized candidate tree contains native artifacts")
    tools = pinned_rust_tools(repo_root, protocol)
    cargo = Path(cast(dict[str, str], tools["cargo"])["path"])
    rustc = Path(cast(dict[str, str], tools["rustc"])["path"])
    with isolated_build_environment(
        rust_workspace=repo_root, cargo=cargo, rustc=rustc
    ) as build_environment:
        result = subprocess.run(
            [
                sys.executable,
                "-I",
                "-m",
                "maturin",
                "develop",
                "--release",
                "--locked",
                "--offline",
                "--skip-install",
                "--manifest-path",
                str(repo_root / "crates" / "greatgramma-python" / "Cargo.toml"),
            ],
            cwd=repo_root,
            env=build_environment,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    if result.returncode != 0:
        detail = result.stderr.strip() or f"maturin exited {result.returncode}"
        raise BenchmarkError(f"fresh candidate release build failed: {detail}")
    artifacts = native_artifacts(package_directory, "_native")
    if len(artifacts) != 1:
        raise BenchmarkError(
            f"candidate build produced {len(artifacts)} native extension artifacts"
        )
    if pinned_rust_tools(repo_root, protocol) != tools:
        raise BenchmarkError("pinned Rust toolchain changed during candidate build")
    return artifacts[0], tools


def total_memory_bytes() -> int | None:
    try:
        pages = os.sysconf("SC_PHYS_PAGES")
        page_size = os.sysconf("SC_PAGE_SIZE")
    except (AttributeError, OSError, ValueError):
        return None
    if type(pages) is int and type(page_size) is int:
        return pages * page_size
    return None


def distribution_versions(names: Iterable[str]) -> dict[str, str]:
    versions: dict[str, str] = {}
    for name in names:
        try:
            versions[name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            versions[name] = "not-installed"
    return versions


def exact_upstream_versions(protocol: dict[str, Any]) -> dict[str, str]:
    upstream = cast(dict[str, Any], protocol["upstream"])
    expected = cast(dict[str, str], upstream["python_dependencies"])
    actual = distribution_versions(expected)
    if actual != expected:
        raise BenchmarkError(
            f"upstream dependencies are {actual!r}; expected {expected!r}"
        )
    return actual


def sanitized_environment_metadata() -> dict[str, Any]:
    values = dict(sorted(os.environ.items()))
    return {
        "sanitized_environment": values,
        "sanitized_environment_sha256": object_hash(values),
    }


def verify_sanitized_environment(metadata: dict[str, Any]) -> None:
    current = sanitized_environment_metadata()
    if any(metadata.get(name) != value for name, value in current.items()):
        raise BenchmarkError("benchmark environment changed during the run")


def environment(
    repo_root: Path,
    native: Any,
    expected_native_path: Path,
    rust_tools: dict[str, Any],
) -> tuple[str, bool, dict[str, Any]]:
    commit, status = source_state(repo_root)
    extension_path = Path(native.__file__).resolve()
    if extension_path != expected_native_path:
        raise BenchmarkError("candidate imported an unexpected native extension")
    try:
        package_version = importlib.metadata.version("greatgramma")
    except importlib.metadata.PackageNotFoundError:
        package_version = "uninstalled-worktree"
    hashes: dict[str, str] = {}
    for name in ("Cargo.lock", "pyproject.toml", "uv.lock"):
        path = repo_root / name
        if path.is_file():
            hashes[name] = file_hash(path)
    metadata: dict[str, Any] = {
        "python_version": platform.python_version(),
        "python_implementation": platform.python_implementation(),
        "python_executable": sys.executable,
        "python_isolated_mode": bool(sys.flags.isolated),
        "package_version": package_version,
        "native_extension": str(extension_path),
        "native_extension_sha256": file_hash(extension_path),
        "platform": platform.platform(),
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "logical_cpu_count": os.cpu_count(),
        "total_memory_bytes": total_memory_bytes(),
        "rust_toolchain": rust_tools,
        "python_distribution_versions": distribution_versions(("greatgramma",)),
        "timer": "time.perf_counter_ns",
        "garbage_collector_during_measurement": "disabled",
        "input_hashes": hashes,
        **sanitized_environment_metadata(),
    }
    return commit, bool(status), metadata


def upstream_environment(
    source_checkout: Path,
    materialized_checkout: Path,
    dependency_versions: dict[str, str],
) -> tuple[str, bool, dict[str, Any]]:
    commit, status = source_state(source_checkout)
    if commit != UPSTREAM_PIN or status:
        raise BenchmarkError(
            f"upstream checkout must be clean and pinned to {UPSTREAM_PIN}"
        )
    native_hashes: dict[str, str] = {}
    for module_name in (
        "alignment.monitor.grammar.partial_lexer",
        "alignment.monitor.grammar.partial_parser",
    ):
        module = importlib.import_module(module_name)
        module_file = module.__file__
        if not isinstance(module_file, str):
            raise BenchmarkError(f"{module_name} has no extension path")
        path = Path(module_file).resolve()
        if materialized_checkout.resolve() not in path.parents:
            raise BenchmarkError(f"{module_name} was imported outside the frozen checkout")
        native_hashes[module_name] = file_hash(path)
    input_hashes = {
        name: file_hash(materialized_checkout / name)
        for name in ("pyproject.toml", "requirements.txt")
        if (materialized_checkout / name).is_file()
    }
    metadata: dict[str, Any] = {
        "python_version": platform.python_version(),
        "python_implementation": platform.python_implementation(),
        "python_executable": sys.executable,
        "python_isolated_mode": bool(sys.flags.isolated),
        "upstream_checkout": str(source_checkout.resolve()),
        "materialized_checkout": str(materialized_checkout.resolve()),
        "upstream_native_hashes": native_hashes,
        "platform": platform.platform(),
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "processor": platform.processor(),
        "logical_cpu_count": os.cpu_count(),
        "total_memory_bytes": total_memory_bytes(),
        "python_distribution_versions": dependency_versions,
        "timer": "time.perf_counter_ns",
        "garbage_collector_during_measurement": "disabled",
        "input_hashes": input_hashes,
        **sanitized_environment_metadata(),
    }
    return commit, False, metadata


def verify_candidate_inputs(
    repo_root: Path,
    protocol: dict[str, Any],
    source_pin: str,
    initially_dirty: bool,
    runner_hash: str,
    native: Any,
    metadata: dict[str, Any],
) -> None:
    verify_sanitized_environment(metadata)
    if metadata.get("rust_toolchain") != pinned_rust_tools(repo_root, protocol):
        raise BenchmarkError("candidate Rust toolchain changed during the benchmark")
    commit, status = source_state(repo_root)
    if commit != source_pin or bool(status) != initially_dirty:
        raise BenchmarkError("candidate source state changed during the benchmark")
    if file_hash(Path(__file__).resolve()) != runner_hash:
        raise BenchmarkError("benchmark runner changed during the benchmark")
    extension_path = Path(native.__file__).resolve()
    if (
        str(extension_path) != metadata.get("native_extension")
        or file_hash(extension_path) != metadata.get("native_extension_sha256")
    ):
        raise BenchmarkError("candidate native extension changed during the benchmark")


def verify_upstream_inputs(
    source_checkout: Path,
    materialized_checkout: Path,
    runner_hash: str,
    metadata: dict[str, Any],
) -> None:
    verify_sanitized_environment(metadata)
    commit, status = source_state(source_checkout)
    if commit != UPSTREAM_PIN or status:
        raise BenchmarkError("upstream source state changed during the benchmark")
    if file_hash(Path(__file__).resolve()) != runner_hash:
        raise BenchmarkError("benchmark runner changed during the benchmark")
    expected_hashes = metadata.get("upstream_native_hashes")
    if not isinstance(expected_hashes, dict):
        raise BenchmarkError("upstream run metadata lost its native hashes")
    for module_name, expected_hash in cast(dict[str, object], expected_hashes).items():
        module = importlib.import_module(module_name)
        module_file = module.__file__
        if not isinstance(module_file, str):
            raise BenchmarkError(f"{module_name} lost its extension path")
        path = Path(module_file).resolve()
        if (
            materialized_checkout.resolve() not in path.parents
            or file_hash(path) != expected_hash
        ):
            raise BenchmarkError("upstream native extension changed during the benchmark")


def parse_vocab_sizes(value: str) -> list[int]:
    try:
        sizes = [int(part) for part in value.split(",") if part]
    except ValueError as error:
        raise argparse.ArgumentTypeError("vocabulary sizes must be comma-separated integers") from error
    if not sizes or len(set(sizes)) != len(sizes) or any(not 257 <= size <= 1_000_000 for size in sizes):
        raise argparse.ArgumentTypeError(
            "vocabulary sizes must be unique values in [257, 1000000]"
        )
    return sizes


def positive(value: str) -> int:
    number = int(value)
    if number <= 0:
        raise argparse.ArgumentTypeError("sample counts must be positive")
    return number


def nonnegative(value: str) -> int:
    number = int(value)
    if number < 0:
        raise argparse.ArgumentTypeError("warm-up count must be nonnegative")
    return number


def emit(records: Iterable[dict[str, Any]], output: TextIO) -> None:
    for record in records:
        print(json.dumps(record, sort_keys=True, separators=(",", ":")), file=output)


def self_test_materialization() -> None:
    with tempfile.TemporaryDirectory(prefix="greatgramma-benchmark-self-test-") as directory:
        repo = Path(directory)
        (repo / ".gitignore").write_text("*.so\n", encoding="utf-8")
        (repo / "tracked.py").write_text("value = 1\n", encoding="utf-8")
        commands = (
            ["git", "init", "--quiet"],
            ["git", "add", ".gitignore", "tracked.py"],
            [
                "git",
                "-c",
                "user.name=GreatGramma benchmark",
                "-c",
                "user.email=benchmark@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ],
        )
        for command in commands:
            result = subprocess.run(
                command,
                cwd=repo,
                env=child_process_environment(),
                check=False,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
            )
            if result.returncode != 0:
                raise AssertionError(f"materialization fixture failed: {result.stderr}")
        (repo / "ignored_native.so").write_bytes(b"not a real extension")
        with materialized_commit(repo) as (materialized, _):
            if not (materialized / "tracked.py").is_file():
                raise AssertionError("tracked source was omitted from materialization")
            if (materialized / "ignored_native.so").exists():
                raise AssertionError("ignored native artifact entered materialization")
        (repo / "untracked.py").write_text("value = 2\n", encoding="utf-8")
        try:
            with materialized_commit(repo):
                pass
        except BenchmarkError:
            pass
        else:
            raise AssertionError("untracked source was accepted for materialization")


def self_test(protocol_path: Path) -> None:
    protocol, protocol_hash = load_protocol(protocol_path)
    policy = enforce_environment_policy(protocol)
    for name in (
        "CPATH",
        "C_INCLUDE_PATH",
        "LIBRARY_PATH",
        "PYTHONMALLOC",
        "PYTHONPATH",
        "LD_PRELOAD",
        "RUSTFLAGS",
    ):
        hostile = dict(os.environ)
        hostile[name] = "hostile"
        if not environment_policy_violations(policy, hostile):
            raise AssertionError(f"environment override {name} escaped the policy")
    without_path = dict(os.environ)
    without_path.pop("PATH")
    if not environment_policy_violations(policy, without_path):
        raise AssertionError("missing PATH escaped the environment policy")
    with tempfile.TemporaryDirectory(prefix="greatgramma-import-self-test-") as directory:
        shadow = Path(directory) / "json.pyc"
        shadow.write_bytes(b"hostile sourceless import shadow")
        result = subprocess.run(
            [
                sys.executable,
                "-I",
                "-c",
                "import json, pathlib; print(pathlib.Path(json.__file__).resolve())",
            ],
            cwd=directory,
            env=child_process_environment(),
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        if result.returncode != 0 or str(Path(directory).resolve()) in result.stdout:
            raise AssertionError("isolated Python imported a working-directory shadow")
    values = token_bytes(257)
    if values[:256] != tuple(bytes((value,)) for value in range(256)):
        raise AssertionError("singleton-byte coverage changed")
    if values[-1] != b"" or generated_token(256) != b"\xa5\x00\x01\x00\x00":
        raise AssertionError("synthetic token generator changed")
    first = corpus_descriptor(protocol, protocol_hash, 257)
    second = corpus_descriptor(protocol, protocol_hash, 257)
    if first != second or percentile([5, 1, 4, 2, 3], 95) != 5:
        raise AssertionError("corpus hashing or quantiles are nondeterministic")
    validate_packed_mask(b"\xff\x00", 9, 8)
    validate_packed_mask(b"\xff\x01", 9, 9)
    validate_upstream_acceptance({token: None for token in range(8)}, 8)
    for invalid in (b"\xff\x01", b"\xff\x80", b"\xff"):
        try:
            validate_packed_mask(invalid, 9, 8)
        except BenchmarkError:
            pass
        else:
            raise AssertionError("malformed packed mask passed validation")
    try:
        validate_upstream_acceptance({0: None, 2: None}, 2)
    except BenchmarkError:
        pass
    else:
        raise AssertionError("malformed upstream acceptance passed validation")
    self_test_materialization()
    print("Python benchmark self-test: ok")


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--protocol", type=Path, default=DEFAULT_PROTOCOL)
    parser.add_argument(
        "--upstream-checkout",
        type=Path,
        help="run the frozen upstream adapter from this clean pinned checkout",
    )
    parser.add_argument("--vocab-sizes", type=parse_vocab_sizes)
    parser.add_argument("--compile-samples", type=positive, default=7)
    parser.add_argument("--operation-samples", type=positive, default=1000)
    parser.add_argument("--warmup", type=nonnegative, default=10)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--_rss-worker", choices=("candidate", "upstream"), help=argparse.SUPPRESS)
    parser.add_argument("--rss-vocab-size", type=int, help=argparse.SUPPRESS)
    parser.add_argument("--_materialized-checkout", type=Path, help=argparse.SUPPRESS)
    return parser.parse_args(argv)


def run_benchmarks(
    args: argparse.Namespace,
    protocol: dict[str, Any],
    protocol_hash: str,
    environment_policy: dict[str, Any],
    vocab_sizes: list[int],
) -> int:
    repo_root = Path(__file__).resolve().parents[1]
    upstream_source = (
        args.upstream_checkout.resolve()
        if args.upstream_checkout is not None
        else None
    )
    records: list[dict[str, Any]] = []
    with contextlib.ExitStack() as materializations:
        native: Any | None = None
        if upstream_source is None:
            materialized_root, expected_pin = materializations.enter_context(
                materialized_commit(repo_root)
            )
            expected_native_path, rust_tools = build_candidate(
                materialized_root, protocol
            )
            sys.path.insert(0, str(materialized_root / "python"))
            try:
                api = importlib.import_module("greatgramma")
                native = importlib.import_module("greatgramma._native")
            except ImportError as error:
                raise BenchmarkError(
                    "greatgramma native extension is unavailable after a fresh build"
                ) from error
            implementation = "greatgramma-rust"
            source_pin, dirty, environment_record = environment(
                repo_root, native, expected_native_path, rust_tools
            )
            if source_pin != expected_pin:
                raise BenchmarkError("candidate materialization pin changed")
            monitor_type = None
        else:
            dependency_versions = exact_upstream_versions(protocol)
            materialized_root, expected_pin = materializations.enter_context(
                materialized_commit(upstream_source, expected_pin=UPSTREAM_PIN)
            )
            monitor_type = load_upstream(materialized_root)
            implementation = "greatgramma-upstream-python"
            source_pin, dirty, environment_record = upstream_environment(
                upstream_source, materialized_root, dependency_versions
            )
            if source_pin != expected_pin:
                raise BenchmarkError("upstream materialization pin changed")
            api = None

        runner_hash = file_hash(Path(__file__).resolve())
        run = {
            "schema": RESULT_SCHEMA,
            "kind": "run",
            "protocol_sha256": protocol_hash,
            "implementation": implementation,
            "source_pin": source_pin,
            "dirty": dirty,
            "runner_sha256": runner_hash,
            "environment_policy": environment_policy,
            "environment": environment_record,
            "parameters": {
                "vocab_sizes": vocab_sizes,
                "compile_samples": args.compile_samples,
                "operation_samples": args.operation_samples,
                "warmup_count": args.warmup,
                "adapter": "upstream" if upstream_source is not None else "candidate",
                "argv": sys.argv,
            },
        }
        records = [run]
        gc_was_enabled = gc.isenabled()
        gc.disable()
        try:
            for vocab_size in vocab_sizes:
                if upstream_source is None:
                    records.extend(
                        benchmark_corpus(
                            api,
                            protocol,
                            protocol_hash,
                            vocab_size,
                            compile_samples=args.compile_samples,
                            operation_samples=args.operation_samples,
                            warmup_count=args.warmup,
                        )
                    )
                    worker_implementation = "candidate"
                else:
                    records.extend(
                        benchmark_upstream_corpus(
                            monitor_type,
                            protocol,
                            protocol_hash,
                            vocab_size,
                            compile_samples=args.compile_samples,
                            operation_samples=args.operation_samples,
                            warmup_count=args.warmup,
                        )
                    )
                    worker_implementation = "upstream"
                records.append(
                    measure_preparation_rss(
                        worker_implementation,
                        args.protocol,
                        corpus_descriptor(protocol, protocol_hash, vocab_size),
                        upstream_source,
                        materialized_root,
                    )
                )
        finally:
            if gc_was_enabled:
                gc.enable()

        _, final_protocol_hash = load_protocol(args.protocol)
        if final_protocol_hash != protocol_hash:
            raise BenchmarkError("benchmark protocol changed during the benchmark")
        if upstream_source is None:
            if native is None:
                raise BenchmarkError("candidate native extension was not retained")
            verify_candidate_inputs(
                repo_root,
                protocol,
                source_pin,
                dirty,
                runner_hash,
                native,
                environment_record,
            )
        else:
            verify_upstream_inputs(
                upstream_source,
                materialized_root,
                runner_hash,
                environment_record,
            )
    if args.output is None:
        emit(records, sys.stdout)
    else:
        with args.output.open("w", encoding="utf-8", newline="\n") as output:
            emit(records, output)
    return 0


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        if args._rss_worker is not None:
            if args.rss_vocab_size is None:
                raise BenchmarkError("RSS worker has no vocabulary size")
            return rss_worker(
                args._rss_worker,
                args.protocol,
                args.rss_vocab_size,
                args.upstream_checkout,
                args._materialized_checkout,
            )
        if args.self_test:
            self_test(args.protocol)
            return 0
        protocol, protocol_hash = load_protocol(args.protocol)
        environment_policy = enforce_environment_policy(protocol)
        sampling = release_sampling(protocol)
        actual_sampling = {
            "compile_samples": args.compile_samples,
            "operation_samples": args.operation_samples,
            "warmup_count": args.warmup,
        }
        if actual_sampling != sampling:
            raise BenchmarkError(
                f"release benchmark sampling must be {sampling!r}"
            )
        default_sizes = protocol.get("default_vocab_sizes")
        if args.vocab_sizes is None:
            if not isinstance(default_sizes, list):
                raise BenchmarkError("protocol has invalid default vocabulary sizes")
            raw_sizes = cast(list[object], default_sizes)
            if any(type(size) is not int for size in raw_sizes):
                raise BenchmarkError("protocol has invalid default vocabulary sizes")
            vocab_sizes = parse_vocab_sizes(
                ",".join(str(size) for size in cast(list[int], raw_sizes))
            )
        else:
            vocab_sizes = args.vocab_sizes
        return run_benchmarks(
            args, protocol, protocol_hash, environment_policy, vocab_sizes
        )
    except (BenchmarkError, OSError) as error:
        print(f"Python benchmark failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
