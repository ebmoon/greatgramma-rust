#!/usr/bin/env bash

set -euo pipefail

fail() {
    echo "Aeneas smoke failed: $*" >&2
    exit 1
}

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
repo_root=$(CDPATH= cd -- "${script_dir}/.." && pwd -P)
proof_root="${repo_root}/proofs"
pin_manifest="${proof_root}/aeneas-manifest.env"
generated_manifest="${proof_root}/Generated.manifest"
generated_baseline="${proof_root}/Generated"

[ -f "${pin_manifest}" ] || fail "missing ${pin_manifest}"
[ -f "${generated_manifest}" ] || fail "missing ${generated_manifest}"
[ -f "${proof_root}/lakefile.lean" ] || fail "missing ${proof_root}/lakefile.lean"
[ -f "${proof_root}/lean-toolchain" ] || fail "missing ${proof_root}/lean-toolchain"
# This file is repository-owned configuration containing readonly string values.
# shellcheck source=../proofs/aeneas-manifest.env
source "${pin_manifest}"

if [ -z "${AENEAS_ROOT:-}" ]; then
    fail "set AENEAS_ROOT to an Aeneas checkout at ${AENEAS_COMMIT}; its charon/ checkout must be at ${CHARON_COMMIT} (or set CHARON_ROOT separately)"
fi

[ -d "${AENEAS_ROOT}" ] || fail "AENEAS_ROOT is not a directory: ${AENEAS_ROOT}"
aeneas_root=$(CDPATH= cd -- "${AENEAS_ROOT}" && pwd -P)
charon_root_input=${CHARON_ROOT:-"${aeneas_root}/charon"}
[ -d "${charon_root_input}" ] || fail "Charon checkout not found at ${charon_root_input}; set CHARON_ROOT to commit ${CHARON_COMMIT}"
charon_root=$(CDPATH= cd -- "${charon_root_input}" && pwd -P)

command -v git >/dev/null 2>&1 || fail "git is required to verify the pinned source checkouts"
command -v cargo >/dev/null 2>&1 || fail "cargo is required to build pinned Charon sources"
command -v lake >/dev/null 2>&1 || fail "lake is required to elaborate the generated Lean module (${LEAN_TOOLCHAIN})"
command -v lean >/dev/null 2>&1 || fail "lean is required to verify the generated Lean module (${LEAN_TOOLCHAIN})"

aeneas_head=$(git -C "${aeneas_root}" rev-parse --verify HEAD 2>/dev/null) || fail "AENEAS_ROOT is not a Git checkout"
[ "${aeneas_head}" = "${AENEAS_COMMIT}" ] || fail "Aeneas checkout is ${aeneas_head}; expected ${AENEAS_COMMIT}"
charon_head=$(git -C "${charon_root}" rev-parse --verify HEAD 2>/dev/null) || fail "CHARON_ROOT is not a Git checkout"
[ "${charon_head}" = "${CHARON_COMMIT}" ] || fail "Charon checkout is ${charon_head}; expected ${CHARON_COMMIT}"

if ! git -C "${aeneas_root}" diff --quiet --ignore-submodules -- ||
    ! git -C "${aeneas_root}" diff --cached --quiet --ignore-submodules --; then
    fail "Aeneas checkout has tracked changes; the extraction input must match the pinned commit"
fi
if ! git -C "${charon_root}" diff --quiet --ignore-submodules -- ||
    ! git -C "${charon_root}" diff --cached --quiet --ignore-submodules --; then
    fail "Charon checkout has tracked changes; the extraction input must match the pinned commit"
fi

aeneas_bin="${aeneas_root}/bin/aeneas"
[ -x "${aeneas_bin}" ] || fail "missing executable ${aeneas_bin}; build the pinned Aeneas checkout"

scratch=$(mktemp -d "${TMPDIR:-/tmp}/greatgramma-aeneas.XXXXXX")
cleanup() {
    if [ "${AENEAS_KEEP_TMP:-0}" = "1" ]; then
        echo "Aeneas smoke scratch retained at ${scratch}" >&2
    else
        rm -rf -- "${scratch}"
    fi
}
trap cleanup EXIT

# Charon reports only its semver, so an ignored bin/charon from another commit
# could otherwise survive a checkout and satisfy every version check below.
# Build from the verified clean source tree into a fresh target directory and
# use Cargo's direct outputs.
echo "Building pinned Charon sources at ${CHARON_COMMIT}..."
(
    cd "${charon_root}/charon"
    CARGO_TARGET_DIR="${scratch}/charon-target" CARGO_PROFILE_RELEASE_DEBUG=true \
        cargo build --release --locked --bin charon --bin charon-driver
)
charon_bin="${scratch}/charon-target/release/charon"
charon_driver="${scratch}/charon-target/release/charon-driver"
[ -x "${charon_bin}" ] || fail "pinned source build did not produce ${charon_bin}"
[ -x "${charon_driver}" ] || fail "pinned source build did not produce ${charon_driver}"

[ -f "${aeneas_root}/charon-pin" ] || fail "missing ${aeneas_root}/charon-pin"
recorded_charon_pin=$(grep -Eo '[0-9a-f]{40}' "${aeneas_root}/charon-pin" | tail -n 1 || true)
[ "${recorded_charon_pin}" = "${CHARON_COMMIT}" ] || fail "Aeneas records Charon ${recorded_charon_pin:-unknown}; expected ${CHARON_COMMIT}"

[ -f "${aeneas_root}/backends/lean/lean-toolchain" ] || fail "missing Aeneas Lean toolchain pin"
aeneas_lean_toolchain=$(tr -d '\r\n' < "${aeneas_root}/backends/lean/lean-toolchain")
[ "${aeneas_lean_toolchain}" = "${LEAN_TOOLCHAIN}" ] || fail "Aeneas requires ${aeneas_lean_toolchain}; expected ${LEAN_TOOLCHAIN}"
repo_lean_toolchain=$(tr -d '\r\n' < "${proof_root}/lean-toolchain")
[ "${repo_lean_toolchain}" = "${LEAN_TOOLCHAIN}" ] || fail "proofs/lean-toolchain is ${repo_lean_toolchain}; expected ${LEAN_TOOLCHAIN}"

charon_toolchain=$("${charon_bin}" toolchain-version 2>/dev/null) || fail "could not query Charon's embedded Rust toolchain"
[ "${charon_toolchain}" = "${RUST_TOOLCHAIN}" ] || fail "Charon embeds ${charon_toolchain}; expected ${RUST_TOOLCHAIN}"
charon_version=$("${charon_bin}" version 2>/dev/null) || fail "could not query Charon's version"
[ "${charon_version}" = "${CHARON_VERSION}" ] || fail "Charon reports ${charon_version}; expected ${CHARON_VERSION} from the pinned commit"

expected_aeneas_version=$(git -C "${aeneas_root}" describe --always --dirty)
aeneas_version=$("${aeneas_bin}" -version 2>/dev/null) || fail "could not query Aeneas's embedded version"
[ "${aeneas_version}" = "aeneas ${expected_aeneas_version}" ] || fail "Aeneas binary reports '${aeneas_version}'; expected 'aeneas ${expected_aeneas_version}'"

llbc_path="${scratch}/${LLBC_FILE}"
lean_project="${scratch}/lean"
mkdir -p "${lean_project}"

echo "Extracting ${CORE_CRATE} with pinned Charon..."
(
    cd "${repo_root}/${CORE_CRATE}"
    "${charon_bin}" cargo --preset=aeneas --dest-file="${llbc_path}"
)
[ -s "${llbc_path}" ] || fail "Charon did not produce a nonempty ${LLBC_FILE}"

echo "Translating ${LLBC_FILE} with pinned Aeneas..."
"${aeneas_bin}" \
    -backend lean \
    -namespace "${LEAN_NAMESPACE}" \
    -dest "${lean_project}" \
    -abort-on-error \
    -warnings-as-errors \
    "${llbc_path}"

generated_entry="${lean_project}/${GENERATED_ENTRY}"
[ -s "${generated_entry}" ] || fail "Aeneas did not produce ${GENERATED_ENTRY}"

cp "${proof_root}/lakefile.lean" "${lean_project}/lakefile.lean"
cp "${proof_root}/lean-toolchain" "${lean_project}/lean-toolchain"
ln -s "${aeneas_root}/backends/lean" "${lean_project}/aeneas"

lean_version=$(
    cd "${lean_project}"
    lean --version
) || fail "could not start the pinned Lean toolchain"
case "${lean_version}" in
    *"version ${LEAN_VERSION}"*) ;;
    *) fail "Lean reports '${lean_version}'; expected version ${LEAN_VERSION}" ;;
esac

echo "Elaborating ${GENERATED_ENTRY} with ${LEAN_TOOLCHAIN}..."
(
    cd "${lean_project}"
    lake build "${LEAN_NAMESPACE}"
)

actual_manifest="${scratch}/Generated.manifest"
(
    cd "${lean_project}"
    find . -maxdepth 1 -type f -name '*.lean' ! -name 'lakefile.lean' -print | sed 's#^\./##' | LC_ALL=C sort
) > "${actual_manifest}"
expected_manifest="${scratch}/Expected.manifest"
grep -Ev '^[[:space:]]*(#|$)' "${generated_manifest}" | LC_ALL=C sort > "${expected_manifest}"
if ! diff -u "${expected_manifest}" "${actual_manifest}"; then
    fail "generated file inventory drifted from proofs/Generated.manifest"
fi

while IFS= read -r relative_path; do
    [ -n "${relative_path}" ] || continue
    baseline_path="${generated_baseline}/${relative_path}"
    [ -f "${baseline_path}" ] || fail "missing checked-in baseline ${baseline_path}; retain the scratch with AENEAS_KEEP_TMP=1, review the elaborated output, and add it explicitly"
    if ! diff -u "${baseline_path}" "${lean_project}/${relative_path}"; then
        fail "generated Lean drifted from ${baseline_path}"
    fi
done < "${expected_manifest}"

echo "Aeneas smoke passed: pinned extraction, Lean elaboration, and generated-file drift are clean."
