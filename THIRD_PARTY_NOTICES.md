# Third-party dependencies and reference provenance

This file records the dependency inputs declared by the repository and the
external projects used as behavioral references. It is an
inventory, not a substitute for the upstream license texts. Upstream licenses
do not license GreatGramma Rust itself; this repository does not yet declare a
project license and is not ready for public distribution.

No third-party source is vendored in this repository. Before distribution, the
release owner must generate and review a complete transitive license report for
the exact Rust and Python lockfiles, include any required license texts or
notices, and select a license for this project.

## Rust dependencies

The following direct dependencies come from the workspace manifests. Exact
Rust versions are locked in `Cargo.lock`.

| Package | Version | Upstream license | Used by |
|---|---:|---|---|
| [cfgrammar](https://crates.io/crates/cfgrammar) | 0.15.0 | Apache-2.0/MIT | `greatgramma` |
| [llguidance](https://crates.io/crates/llguidance) | 1.8.0 | MIT | `greatgramma` |
| [lrtable](https://crates.io/crates/lrtable) | 0.15.0 | Apache-2.0/MIT | `greatgramma` |
| [regex-syntax](https://crates.io/crates/regex-syntax) | 0.8.11 | MIT OR Apache-2.0 | `greatgramma` |
| [serde_json](https://crates.io/crates/serde_json) | 1.0.151 | MIT OR Apache-2.0 | `greatgramma` |
| [pyo3](https://crates.io/crates/pyo3) | 0.29.0 | MIT OR Apache-2.0 | `greatgramma-python` |
| [libfuzzer-sys](https://crates.io/crates/libfuzzer-sys) | 0.4.13 | MIT OR Apache-2.0 | bounded development fuzz target only |

The path dependency from `greatgramma-python` to `greatgramma` is project code,
not a third-party package.
`Cargo.lock` is the authoritative inventory of transitive Rust packages for a
checkout. The remaining locked packages and their registry-manifest license
expressions are recorded below. Some are target-conditional and may not be
linked into every build.

| Locked package | Version | Registry-manifest license expression |
|---|---:|---|
| ahash | 0.8.12 | MIT OR Apache-2.0 |
| aho-corasick | 1.1.5 | Unlicense OR MIT |
| anyhow | 1.0.104 | MIT OR Apache-2.0 |
| autocfg | 1.5.1 | Apache-2.0 OR MIT |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT |
| bytemuck_derive | 1.12.0 | Zlib OR Apache-2.0 OR MIT |
| cfg-if | 1.0.4 | MIT OR Apache-2.0 |
| derivre | 0.3.12 | MIT |
| equivalent | 1.0.2 | Apache-2.0 OR MIT |
| fnv | 1.0.7 | Apache-2.0 OR MIT |
| getrandom | 0.3.4 | MIT OR Apache-2.0 |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 |
| heck | 0.5.0 | MIT OR Apache-2.0 |
| indexmap | 2.14.0 | Apache-2.0 OR MIT |
| itoa | 1.0.18 | MIT OR Apache-2.0 |
| libc | 0.2.189 | MIT OR Apache-2.0 |
| llguidance | 1.8.0 | MIT |
| lrtable | 0.15.0 | Apache-2.0 OR MIT |
| memchr | 2.8.3 | Unlicense OR MIT |
| num-traits | 0.2.19 | MIT OR Apache-2.0 |
| once_cell | 1.21.4 | MIT OR Apache-2.0 |
| packedvec | 2.0.0 | Apache-2.0 OR MIT |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 |
| pyo3-build-config | 0.29.0 | MIT OR Apache-2.0 |
| pyo3-ffi | 0.29.0 | MIT OR Apache-2.0 |
| pyo3-macros | 0.29.0 | MIT OR Apache-2.0 |
| pyo3-macros-backend | 0.29.0 | MIT OR Apache-2.0 |
| quote | 1.0.47 | MIT OR Apache-2.0 |
| r-efi | 5.3.0 | MIT OR Apache-2.0 OR LGPL-2.1-or-later |
| regex | 1.13.1 | MIT OR Apache-2.0 |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 |
| serde | 1.0.229 | MIT OR Apache-2.0 |
| serde_core | 1.0.229 | MIT OR Apache-2.0 |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 |
| sparsevec | 0.3.0 | Apache-2.0 OR MIT |
| strum | 0.28.0 | MIT |
| strum_macros | 0.28.0 | MIT |
| syn | 2.0.119 | MIT OR Apache-2.0 |
| syn | 3.0.4 | MIT OR Apache-2.0 |
| target-lexicon | 0.13.5 | Apache-2.0 WITH LLVM-exception |
| toktrie | 1.8.0 | MIT |
| unicode-ident | 1.0.24 | (MIT OR Apache-2.0) AND Unicode-3.0 |
| version_check | 0.9.5 | MIT/Apache-2.0 |
| vob | 4.0.0 | Apache-2.0/MIT |
| wasip2 | 1.0.4+wasi-0.2.12 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| wit-bindgen | 0.57.1 | Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT |
| zerocopy | 0.8.56 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zerocopy-derive | 0.8.56 | BSD-2-Clause OR Apache-2.0 OR MIT |
| zmij | 1.0.23 | MIT |

This inventory does not by itself satisfy distribution obligations; required
license texts and notices still need a release-time review.

## Python build, runtime, and development dependencies

The following versions or ranges are declared in `pyproject.toml`. `uv.lock`
is the reproducible development lockfile for this checkout. Optional runtime
packages are not imported by `greatgramma`; they are needed only by the
Transformers adapter.

| Package | Declared version | Upstream license | Role |
|---|---:|---|---|
| [maturin](https://github.com/PyO3/maturin) | 1.14.1 | Apache-2.0 OR MIT | build backend and development |
| [PyTorch](https://github.com/pytorch/pytorch) | `>=2.13,<2.14` | BSD-3-Clause | optional tensor runtime |
| [Transformers](https://github.com/huggingface/transformers) | `>=5.14,<5.15` | Apache-2.0 | optional generation adapter |
| [Pyright](https://github.com/microsoft/pyright) | 1.1.411 | MIT | development type checking |
| [pytest](https://github.com/pytest-dev/pytest) | 9.1.1 | MIT | development tests |
| [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) | 0.13.2 | MIT OR Apache-2.0 | bounded development fuzz runner |

Python wheels may include platform-specific or bundled dependencies. Their
notices must be derived from the exact wheel build, not inferred from this
source inventory.

## Behavioral references

The projects below informed behavior or design.
They are not source dependencies unless also listed above.

| Project | Frozen source | Upstream license and attribution | Role |
|---|---|---|---|
| GreatGramma | [commit `4c21981386fc6d457efa381d1eb1863623d50fa1`](https://github.com/large-loris-models/greatgramma/commit/4c21981386fc6d457efa381d1eb1863623d50fa1) | MIT; Copyright (c) 2024 Kanghee Park ([license](https://github.com/large-loris-models/greatgramma/blob/4c21981386fc6d457efa381d1eb1863623d50fa1/LICENSE)) | Behavioral reference |
| llguidance | [commit `c75b0d90e12c941b881566e0a77bb5fa24cd2731`](https://github.com/guidance-ai/llguidance/commit/c75b0d90e12c941b881566e0a77bb5fa24cd2731) | MIT; Copyright (c) Microsoft Corporation ([license](https://github.com/guidance-ai/llguidance/blob/c75b0d90e12c941b881566e0a77bb5fa24cd2731/LICENSE)) | Compiler dependency and implementation reference |

## derivre version and reference revision

The build uses the crates.io `derivre` 0.3.12 release transitively through
llguidance. Its `v0.3.12` tag is
[commit `f5a4d66a0e2f177304f570a6f2e87f447b4d7808`](https://github.com/guidance-ai/derivre/commit/f5a4d66a0e2f177304f570a6f2e87f447b4d7808).
During design, the project also inspected
[commit `60ddc2be07a84c323f979556e2fd698cf38ad56d`](https://github.com/guidance-ai/derivre/commit/60ddc2be07a84c323f979556e2fd698cf38ad56d),
which is eight commits ahead of that tag. The later commit is a reference only;
it is not the code selected by `Cargo.lock`.

If third-party material is copied, adapted, linked, or distributed later, its
applicable license text, notices, attribution, and relationship to changed
files must be reviewed and recorded at that time.
