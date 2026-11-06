#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Verifies that this repository has exactly one source of truth for every tool
# version, and that nothing has quietly grown a second one.
#
# Constitution §7 requires that CI runs the same image as the devcontainer. That
# turns the usual multi-way pin comparison into something simpler and stronger:
#
#   1. `channel` in rust-toolchain.toml must match `ARG RUST_VERSION` in
#      .devcontainer/Dockerfile. Two files, one version.
#   2. Both Tier-1 bare-metal targets must still be listed. A toolchain file that
#      had quietly lost one would let a one-architecture kernel pass CI.
#   3. No workflow, and no script CI runs, may install a toolchain of its own.
#      This is the check that keeps the arrangement honest: the moment a
#      `dtolnay/rust-toolchain` or a `setup-node` reappears in .github/workflows/,
#      or a `rustup toolchain install` in .github/scripts/, CI has a second source of
#      truth for a version and §7's guarantee is gone — silently, because
#      everything would still build.
#   4. Every action a workflow `uses:` is owned by GitHub (`actions/`, `github/`)
#      or by this organisation (`setonix-os/`). The repository's Actions policy
#      allows nothing else, so a third-party action would fail only once CI ran
#      it; this check fails it before the push. Anything else a workflow needs is
#      a `run:` step, in the open, where its review is the review of the diff.
#
# Resolves paths relative to the repository root regardless of the caller's CWD.

set -euo pipefail

export LC_ALL=C

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "${script_dir}/../.." && pwd)
cd "${repo_root}"

fail=0

# --- 1. One Rust version, named twice --------------------------------------

toml_channel=$(awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ {print $2; exit}' rust-toolchain.toml)

if [[ -z "$toml_channel" ]]; then
    echo "ERROR: could not extract channel from rust-toolchain.toml" >&2
    exit 1
fi

dockerfile=".devcontainer/Dockerfile"

if [[ ! -f "$dockerfile" ]]; then
    echo "ERROR: $dockerfile not found — the canonical environment is missing," >&2
    echo "       and CI has nothing to run inside (§7)." >&2
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
    echo "       Bump both together in one deliberate commit (§7)." >&2
    fail=1
fi

# --- 2. Both Tier-1 targets present ----------------------------------------

for target in aarch64-unknown-none-softfloat x86_64-unknown-none; do
    if ! grep -q "$target" rust-toolchain.toml; then
        echo "ERROR: rust-toolchain.toml is missing Tier-1 target '$target' (§6)" >&2
        fail=1
    fi
done

# --- 3. No workflow may bring its own toolchain ----------------------------
# Each pattern is an action that would install a language runtime or compiler
# outside the container, creating a version that can drift from the Dockerfile's.

forbidden_actions=(
    'dtolnay/rust-toolchain'
    'actions-rs/toolchain'
    'actions/setup-node'
    'actions/setup-python'
    'actions/setup-go'
    'actions/setup-java'
)

for action in "${forbidden_actions[@]}"; do
    if grep -rn "uses:[[:space:]]*${action}" .github/workflows/ >/dev/null 2>&1; then
        echo "ERROR: .github/workflows/ uses '${action}', which installs a toolchain" >&2
        echo "       outside the devcontainer. §7 requires CI to run the same image," >&2
        echo "       so tool versions belong in .devcontainer/Dockerfile and nowhere" >&2
        echo "       else. Offending lines:" >&2
        grep -rn "uses:[[:space:]]*${action}" .github/workflows/ >&2
        fail=1
    fi
done

# Installing a toolchain by hand inside a workflow step, or in a script CI runs
# (the checks job runs check-all.sh), defeats the same rule.
# Deliberately unanchored: the install idiom usually appears mid-line inside a
# `run: |` block scalar (`curl ... sh.rustup.rs | sh`), where an anchored
# pattern never matches.
if grep -rnE --exclude=check-toolchain-pin.sh '(sh\.rustup\.rs|rustup (default|toolchain install))' \
        .github/workflows/ .github/scripts/ >/dev/null 2>&1; then
    echo "ERROR: a workflow or CI script installs or switches a Rust toolchain by hand." >&2
    echo "       The image's pinned toolchain is the only one CI may use (§7)." >&2
    grep -rnE --exclude=check-toolchain-pin.sh '(sh\.rustup\.rs|rustup (default|toolchain install))' \
        .github/workflows/ .github/scripts/ >&2
    fail=1
fi

# --- 4. Only GitHub's and our own actions ----------------------------------
# Every `uses:` value, in any of YAML's three quotings, with a trailing comment
# stripped. A list item (`- uses:`) and a mapping key (`uses:`) are both
# matched, which is every form a step or a reusable-workflow call can take.
# A `./` path is an action in this repository, and so ours by definition.

uses_lines=$(grep -rnE '^[[:space:]]*(-[[:space:]]+)?uses:' .github/workflows/ || true)

if [[ -z "$uses_lines" ]]; then
    echo "ERROR: found no \`uses:\` in .github/workflows/; the action-owner check" >&2
    echo "       would pass by checking nothing. Has the layout changed?" >&2
    fail=1
fi

while IFS= read -r line; do
    [[ -z "$line" ]] && continue
    value=${line#*uses:}
    value=${value%%#*}
    value=$(sed -E "s/^[[:space:]]*['\"]?//; s/['\"]?[[:space:]]*$//" <<<"$value")
    case "$value" in
        actions/* | github/* | setonix-os/* | ./*) ;;
        *)
            echo "ERROR: a workflow uses an action owned by neither GitHub nor setonix-os:" >&2
            echo "       ${line}" >&2
            echo "       The repository allows no other, so CI would refuse '${value}'." >&2
            echo "       Write the step as \`run:\` instead (CLAUDE.md § Toolchain)." >&2
            fail=1
            ;;
    esac
done <<<"$uses_lines"

if [[ $fail -ne 0 ]]; then
    exit 1
fi

echo "OK: Rust pinned to $toml_channel in rust-toolchain.toml and $dockerfile"
echo "OK: both Tier-1 bare-metal targets present"
echo "OK: no workflow or CI script installs a toolchain outside the devcontainer"
echo "OK: every action a workflow uses is GitHub's or setonix-os's"
