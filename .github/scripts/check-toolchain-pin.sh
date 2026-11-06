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
#   3. No workflow, composite action or script CI runs may install a toolchain of
#      its own. This is the check that keeps the arrangement honest: the moment a
#      `dtolnay/rust-toolchain` or a `setup-node` reappears in .github/workflows/,
#      or a `rustup toolchain install` in .github/scripts/, CI has a second source of
#      truth for a version and §7's guarantee is gone — silently, because
#      everything would still build.
#   4. Every action a workflow or composite action `uses:` is owned by GitHub
#      (`actions/`, `github/`) or by this organisation (`setonix-os/`), and is
#      pinned to a full commit SHA. The repository's Actions policy allows no other
#      owner, so a third-party action would fail only once CI ran it; this check
#      fails it before the push. Anything else a workflow needs is a `run:` step,
#      in the open, where its review is the review of the diff.
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

# --- 3 and 4. What workflows and composite actions may use ------------------
# One scan of every `uses:` in .github/workflows/ and in each composite action
# under .github/actions/ (whose steps a workflow pulls in unseen). A value must:
#
#   - not be an action that installs a toolchain (rule 3);
#   - be owned by GitHub or by setonix-os, or be a local path under
#     .github/actions/ or .github/workflows/, which this scan covers (rule 4);
#   - be pinned to a full commit SHA, as STYLE.md § Action Pinning requires.
#     Dependabot writes `@<sha> # vX.Y.Z`, and the comment is stripped first.
#
# A grep cannot parse YAML, so instead of trying to, this check accepts `uses:`
# in exactly one form, `uses: VALUE` as a mapping key or as the first key of a
# list item, and fails every other way of writing it: a space before the colon,
# a quoted key, a flow mapping or sequence, a value on the next line, or an
# explicit `?` key. Each is valid YAML that GitHub would honour and a narrower
# pattern would have waved through. Full-line comments are skipped; anything
# else that merely contains the word `uses` before a colon, even in a `run:`
# script, is refused too, and is reworded rather than special-cased.

forbidden_actions=(
    'dtolnay/rust-toolchain'
    'actions-rs/toolchain'
    'actions/setup-node'
    'actions/setup-python'
    'actions/setup-go'
    'actions/setup-java'
)

quote="[\"']"
canonical='^[[:space:]]*(-[[:space:]]+)?uses:[[:space:]]'
any_uses_key="(^|[^[:alnum:]_-])${quote}?uses${quote}?[[:space:]]*:"
explicit_key='^[[:space:]]*(-[[:space:]]+)?[?]([[:space:]]|$)'
pinned='@[0-9a-f]{40}$'

reject() {
    echo "ERROR: $1" >&2
    echo "       ${where}" >&2
    shift
    for detail in "$@"; do echo "       ${detail}" >&2; done
    fail=1
}

mapfile -t action_files < <(
    find .github/workflows -type f \( -name '*.yml' -o -name '*.yaml' \) 2>/dev/null
    if [[ -d .github/actions ]]; then
        find .github/actions -type f \( -name 'action.yml' -o -name 'action.yaml' \)
    fi
)

uses_seen=0
for file in "${action_files[@]}"; do
    lineno=0
    while IFS= read -r line || [[ -n $line ]]; do
        lineno=$((lineno + 1))
        [[ $line =~ ^[[:space:]]*# ]] && continue
        where="${file}:${lineno}: ${line}"

        if [[ $line =~ $explicit_key ]]; then
            reject "an explicit \`?\` mapping key, which this check cannot read:" \
                "Write the key plainly, as \`uses: VALUE\`."
            continue
        fi
        if ! [[ $line =~ $canonical ]]; then
            if [[ $line =~ $any_uses_key ]]; then
                reject "\`uses\` written in a form this check cannot read:" \
                    "Write it as \`uses: VALUE\`, a block mapping key on one line."
            fi
            continue
        fi

        uses_seen=$((uses_seen + 1))
        value=${line#*uses:}
        value=${value%%#*}
        value=$(sed -E "s/^[[:space:]]*['\"]?//; s/['\"]?[[:space:]]*$//" <<<"$value")

        for action in "${forbidden_actions[@]}"; do
            if [[ $value == "$action" || $value == "$action"@* || $value == "$action"/* ]]; then
                reject "'${action}' installs a toolchain outside the devcontainer:" \
                    "§7 requires CI to run the same image, so tool versions belong in" \
                    ".devcontainer/Dockerfile and nowhere else."
            fi
        done

        case "$value" in
            ./.github/actions/* | ./.github/workflows/*) continue ;;
            ./*)
                reject "a local action outside .github/actions/, which this check does not scan:"
                continue
                ;;
            actions/* | github/* | setonix-os/*) ;;
            *)
                reject "an action owned by neither GitHub nor setonix-os:" \
                    "The repository allows no other, so CI would refuse '${value}'." \
                    "Write the step as \`run:\` instead (CLAUDE.md § Toolchain)."
                ;;
        esac

        if ! [[ $value =~ $pinned ]]; then
            reject "an action not pinned to a full commit SHA:" \
                "Pin '${value%@*}' to a 40-character SHA with a \`# vX.Y.Z\` comment (STYLE.md § Action Pinning)."
        fi
    done <"$file"
done

if [[ $uses_seen -eq 0 ]]; then
    echo "ERROR: found no \`uses:\` in .github/workflows/; the action checks would" >&2
    echo "       pass by checking nothing. Has the layout changed?" >&2
    fail=1
fi

# Installing a toolchain by hand inside a workflow step, a composite action, or a
# script CI runs (the checks job runs check-all.sh) defeats rule 3 the same way.
# Deliberately unanchored: the install idiom usually appears mid-line inside a
# `run: |` block scalar (`curl ... sh.rustup.rs | sh`), where an anchored
# pattern never matches.
hand_install_dirs=(.github/workflows/ .github/scripts/)
if [[ -d .github/actions ]]; then hand_install_dirs+=(.github/actions/); fi
if grep -rnE --exclude=check-toolchain-pin.sh '(sh\.rustup\.rs|rustup (default|toolchain install))' \
        "${hand_install_dirs[@]}" >/dev/null 2>&1; then
    echo "ERROR: a workflow, composite action or CI script installs or switches a Rust" >&2
    echo "       toolchain by hand. The image's pinned toolchain is the only one CI may use (§7)." >&2
    grep -rnE --exclude=check-toolchain-pin.sh '(sh\.rustup\.rs|rustup (default|toolchain install))' \
        "${hand_install_dirs[@]}" >&2
    fail=1
fi

if [[ $fail -ne 0 ]]; then
    exit 1
fi

echo "OK: Rust pinned to $toml_channel in rust-toolchain.toml and $dockerfile"
echo "OK: both Tier-1 bare-metal targets present"
echo "OK: no workflow or CI script installs a toolchain outside the devcontainer"
echo "OK: every action used is GitHub's or setonix-os's, pinned to a full SHA"
