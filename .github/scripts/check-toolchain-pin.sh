#!/usr/bin/env bash
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
#   3. No workflow may install a toolchain of its own. This is the check that
#      keeps the arrangement honest: the moment a `dtolnay/rust-toolchain` or a
#      `setup-node` reappears in .github/workflows/, CI has a second source of
#      truth for a version and §7's guarantee is gone — silently, because
#      everything would still build.
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

for target in aarch64-unknown-none x86_64-unknown-none; do
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

# Installing a toolchain by hand inside a workflow step defeats the same rule.
if grep -rnE '^[[:space:]]*(run:.*)?(sh\.rustup\.rs|rustup (default|toolchain install))' \
        .github/workflows/ >/dev/null 2>&1; then
    echo "ERROR: a workflow installs or switches a Rust toolchain by hand." >&2
    echo "       The image's pinned toolchain is the only one CI may use (§7)." >&2
    grep -rnE '^[[:space:]]*(run:.*)?(sh\.rustup\.rs|rustup (default|toolchain install))' \
        .github/workflows/ >&2
    fail=1
fi

if [[ $fail -ne 0 ]]; then
    exit 1
fi

echo "OK: Rust pinned to $toml_channel in rust-toolchain.toml and $dockerfile"
echo "OK: both Tier-1 bare-metal targets present"
echo "OK: no workflow installs a toolchain outside the devcontainer"
