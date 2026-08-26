# Phase 2 Lean Blueprint

This Blueprint is the reviewable roadmap for the Phase 2 proof. It records the
intended dependency graph, the named assumption boundary, and the three public
root theorems. It is documentation, not proof evidence: graph colors,
declaration links, `\leanok`, and successful HTML/PDF rendering do not replace
Lean elaboration, the proof manifest, generated-coverage reconciliation, or the
axiom audit.

The source of truth is [`src/content.tex`](src/content.tex). Generated files are
local build products. Roadmap content stays directly in `content.tex`; the
source checker rejects nested `\input` or `\include` chapters so rendered nodes
cannot bypass its state and dependency checks.

- `blueprint/web/` contains the plasTeX HTML output.
- `blueprint/print/` contains the printable PDF output.
- `blueprint/lean_decls` is emitted by plasTeX from `\lean{...}` markers.
- `blueprint/src/web.paux` is plasTeX's local parse cache.

Do not commit or publish those generated paths. Until the repository has an
explicit publication license, CI may only build and discard them.

## Documentation states

Every theorem-like node has exactly one `\bpstatus{...}` marker:

| State | Required markers | Meaning |
| --- | --- | --- |
| `planned` | one `\notready`; no `\lean` or `\leanok` | The stable roadmap node exists, but no stable Lean declaration exists yet. |
| `linked` | exactly one `\lean`; no `\notready` or `\leanok` | A stable handwritten declaration exists, but authoritative proof gates have not established completion. |
| `proved` | exactly one `\lean` and one `\leanok`; no `\notready` | The declaration is covered by the proof manifest and has passed elaboration and axiom gates. |
| `assumption` | an `assumption` environment, one `\notready`, optional single `\lean`, and no `\leanok` | A named condition or trusted boundary remains conditional even if its proposition is formalized. |

Lean links may target only stable handwritten declarations under
`Greatgramma.Spec`, `Greatgramma.Invariants`, `Greatgramma.Refinement`, or
`Greatgramma.Theorems`. Generated Aeneas names must never appear in the
Blueprint.

## Local checks and rendering

Use the repository's `just blueprint-check` and `just blueprint-build` recipes
once the U1 workspace setup is present. They intentionally invoke the tools
below directly because the Lean project lives under `proofs/`, rather than at
the repository root.

The direct source check, which needs only Python 3, is:

```bash
python3 scripts/check-blueprint-coverage.py --self-test
```

HTML rendering requires Python, Graphviz, and the pinned
`leanblueprint==0.0.20` proof-documentation environment. From the repository
root, the direct setup and renderer commands are:

```bash
uv sync --locked --group proof-docs --no-install-project
uv run --frozen --no-sync --project . --directory blueprint/src \
  plastex -c plastex.cfg web.tex
```

PDF rendering requires `latexmk` and XeLaTeX. From `blueprint/src/`, run:

```bash
latexmk -xelatex -interaction=nonstopmode -halt-on-error \
  -outdir=../print print.tex
```

The HTML build writes `blueprint/lean_decls`. Once nodes acquire `\lean`
links, the project-local Lean 4.31 declaration checker must resolve that file
from the nested `proofs/` Lake project. Declaration existence is a traceability
check only; it does not establish the linked theorem's statement or proof:

```bash
cd proofs
lake exe blueprintCheckDecls ../blueprint/lean_decls
```
