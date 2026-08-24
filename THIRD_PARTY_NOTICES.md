# Reference provenance and third-party notices

The current bootstrap does not vendor, copy, or adapt third-party source code.
It inspects the projects below as behavioral references, formal specifications,
or future build/verification tooling. The listed licenses and attributions
describe those upstream projects; they do not license GreatGramma Rust.

| Project | Canonical frozen source | Upstream license and attribution at that revision | Bootstrap role |
|---|---|---|---|
| GreatGramma | [repository](https://github.com/large-loris-models/greatgramma); [commit `4c21981386fc6d457efa381d1eb1863623d50fa1`](https://github.com/large-loris-models/greatgramma/commit/4c21981386fc6d457efa381d1eb1863623d50fa1) | MIT; Copyright (c) 2024 Kanghee Park ([license file](https://github.com/large-loris-models/greatgramma/blob/4c21981386fc6d457efa381d1eb1863623d50fa1/LICENSE)) | Behavioral reference only |
| llguidance | [repository](https://github.com/guidance-ai/llguidance); [commit `c75b0d90e12c941b881566e0a77bb5fa24cd2731`](https://github.com/guidance-ai/llguidance/commit/c75b0d90e12c941b881566e0a77bb5fa24cd2731) | MIT; Copyright (c) Microsoft Corporation ([license file](https://github.com/guidance-ai/llguidance/blob/c75b0d90e12c941b881566e0a77bb5fa24cd2731/LICENSE)) | Implementation reference only |
| derivre | [repository](https://github.com/guidance-ai/derivre); [commit `60ddc2be07a84c323f979556e2fd698cf38ad56d`](https://github.com/guidance-ai/derivre/commit/60ddc2be07a84c323f979556e2fd698cf38ad56d) | MIT; Copyright (c) Microsoft Corporation ([license file](https://github.com/guidance-ai/derivre/blob/60ddc2be07a84c323f979556e2fd698cf38ad56d/LICENSE)) | Planned public-API DFA spike; not yet a dependency |
| Constrained-decoding formalization | [repository](https://github.com/ucsd-formal/constrained-decoding-formalization); [commit `112af02fa29b3e50c1048c36b4b8d18975dee102`](https://github.com/ucsd-formal/constrained-decoding-formalization/commit/112af02fa29b3e50c1048c36b4b8d18975dee102) | Apache-2.0; Copyright 2026 Arnav Dandu ([license file](https://github.com/ucsd-formal/constrained-decoding-formalization/blob/112af02fa29b3e50c1048c36b4b8d18975dee102/LICENSE)) | Formal-specification reference only |
| Aeneas | [repository](https://github.com/AeneasVerif/aeneas); [commit `5d08da45a405913bbee6fd544e01debf8154ac9d`](https://github.com/AeneasVerif/aeneas/commit/5d08da45a405913bbee6fd544e01debf8154ac9d) | Apache-2.0; the root license has no project-specific copyright line or `NOTICE` file at this revision ([license file](https://github.com/AeneasVerif/aeneas/blob/5d08da45a405913bbee6fd544e01debf8154ac9d/LICENSE.md)) | Future verification tool only |
| Charon | [repository](https://github.com/AeneasVerif/charon); [commit `f5208b1c4ce287898a1fc015d41a20da3baf0974`](https://github.com/AeneasVerif/charon/commit/f5208b1c4ce287898a1fc015d41a20da3baf0974) | Apache-2.0; the root license has no project-specific copyright line or `NOTICE` file at this revision ([license file](https://github.com/AeneasVerif/charon/blob/f5208b1c4ce287898a1fc015d41a20da3baf0974/LICENSE.md)) | Future Rust extraction tool only |

## derivre version-to-commit relationship

At the frozen derivre commit, `Cargo.toml` declares version `0.3.12`.
The canonical [`v0.3.12` tag](https://github.com/guidance-ai/derivre/releases/tag/v0.3.12)
resolves to commit
[`f5a4d66a0e2f177304f570a6f2e87f447b4d7808`](https://github.com/guidance-ai/derivre/commit/f5a4d66a0e2f177304f570a6f2e87f447b4d7808),
not to the frozen commit above. The frozen commit is eight commits ahead of
that tag ([upstream comparison](https://github.com/guidance-ai/derivre/compare/f5a4d66a0e2f177304f570a6f2e87f447b4d7808...60ddc2be07a84c323f979556e2fd698cf38ad56d)).
Therefore the full frozen commit, rather than the version string or tag, is the
reproducible input for the planned API spike.

If third-party material is later copied, adapted, linked, or distributed, its
applicable license text, notices, attribution, and relationship to the changed
files must be reviewed and recorded at that time.
