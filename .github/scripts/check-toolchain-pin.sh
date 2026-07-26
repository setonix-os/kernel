#!/usr/bin/env bash
# Verifies that every place this repository names a Rust version agrees:
#
#   1. `channel` in rust-toolchain.toml                    — the source of truth
#   2. `ARG RUST_VERSION` in .devcontainer/Dockerfile      — the workshop
#   3. `dtolnay/rust-toolchain@<sha> # vX.Y.Z` in CI       — the robots
#
# Item 2 is an addition to the house version of this script, and it is the whole
# reason the script earns its place here: CLAUDE.md §7 promises the toolchain is
# "pinned, never drifting", but the devcontainer and rust-toolchain.toml are two
# separate files that a human must remember to bump together. Now they cannot
# silently disagree.
#
# Any trailing annotation on the CI pin (e.g. " (latest stable)") is ignored —
# only the X.Y.Z token is matched. Exits non-zero on mismatch.
#
# Resolves paths relative to the repository root regardless of the caller's CWD,
# so this works whether invoked from CI as
# `bash .github/scripts/check-toolchain-pin.sh` or directly.

set -euo pipefail

# Stable LC_ALL so `sort -u` orders bytes consistently across runners.
export LC_ALL=C

# cd to the repo root (script lives at <root>/.github/scripts/...).
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "${script_dir}/../.." && pwd)
cd "${repo_root}"

fail=0

# --- 1. rust-toolchain.toml — the source of truth ---------------------------

toml_channel=$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ {print $2; exit}' rust-toolchain.toml)

if [[ -z "$toml_channel" ]]; then
    echo "ERROR: could not extract channel from rust-toolchain.toml" >&2
    exit 1
fi

# --- 2. Devcontainer Dockerfile --------------------------------------------

dockerfile=".devcontainer/Dockerfile"

if [[ ! -f "$dockerfile" ]]; then
    echo "ERROR: $dockerfile not found — the canonical dev environment is missing" >&2
    exit 1
fi

docker_version=$(
    sed -nE 's/^[[:space:]]*ARG[[:space:]]+RUST_VERSION[[:space:]]*=[[:space:]]*([0-9]+\.[0-9]+\.[0-9]+).*/\1/p' \
        "$dockerfile" | head -n1
)

if [[ -z "$docker_version" ]]; then
    echo "ERROR: could not extract ARG RUST_VERSION from $dockerfile" >&2
    fail=1
elif [[ "$docker_version" != "$toml_channel" ]]; then
    echo "ERROR: $dockerfile ARG RUST_VERSION ($docker_version) does not match" >&2
    echo "       rust-toolchain.toml channel ($toml_channel)." >&2
    echo "       Bump both together in one deliberate commit (CLAUDE.md §7)." >&2
    fail=1
fi

# --- 3. CI action pins -----------------------------------------------------

mapfile -t pin_versions < <(
    grep -h 'uses:[[:space:]]*dtolnay/rust-toolchain@' .github/workflows/*.yml \
        | sed -nE 's|.*#[[:space:]]+v?([0-9]+\.[0-9]+\.[0-9]+).*|\1|p' \
        | sort -u
)

if [[ ${#pin_versions[@]} -eq 0 ]]; then
    echo "ERROR: no dtolnay/rust-toolchain pins found in .github/workflows/" >&2
    fail=1
elif [[ ${#pin_versions[@]} -ne 1 ]]; then
    echo "ERROR: dtolnay/rust-toolchain pins disagree across workflows: ${pin_versions[*]}" >&2
    fail=1
elif [[ "${pin_versions[0]}" != "$toml_channel" ]]; then
    echo "ERROR: rust-toolchain.toml channel ($toml_channel) does not match action pin (v${pin_versions[0]})" >&2
    echo "       Bump both together when updating Rust." >&2
    fail=1
fi

# --- 4. Bare-metal targets must be present ---------------------------------
# Tier 1 is x86_64 *and* AArch64 from day one (CLAUDE.md §6). A toolchain file
# that has quietly lost a target would let a one-architecture kernel pass CI.

for target in aarch64-unknown-none x86_64-unknown-none; do
    if ! grep -q "$target" rust-toolchain.toml; then
        echo "ERROR: rust-toolchain.toml is missing Tier-1 target '$target'" >&2
        fail=1
    fi
done

if [[ $fail -ne 0 ]]; then
    exit 1
fi

echo "OK: Rust pinned to $toml_channel across rust-toolchain.toml, $dockerfile and CI"
echo "OK: both Tier-1 bare-metal targets present"
