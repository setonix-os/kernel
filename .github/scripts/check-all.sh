#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Every check CI's checks job runs, in the order it runs them. The checks job in
# ci_pr.yml and ci_main.yml calls this script and nothing else, so this file is
# the only list: a check added here is added to CI, and a check CI runs can be
# run locally before pushing with
#
#     bash .github/scripts/check-all.sh
#
# Run it inside the devcontainer (CONSTITUTION.md §7). It stops at the first
# failing step and names it.
#
#   --github    Fold each step into a GitHub Actions log group. Output only; the
#               steps are identical with or without it.
#
# Resolves paths relative to the repository root regardless of the caller's CWD.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "${script_dir}/../.." && pwd)
cd "${repo_root}"

github=0
for arg in "$@"; do
    case "$arg" in
        --github) github=1 ;;
        *)
            echo "usage: $0 [--github]" >&2
            exit 2
            ;;
    esac
done

steps=0

# step NAME COMMAND [ARG...] — run one check, and stop the whole run if it fails.
step() {
    local name=$1
    shift
    if [[ $github -eq 1 ]]; then echo "::group::${name}"; else echo "==> ${name}"; fi
    # Not `"$@" || status=$?`: on the left of || bash ignores set -e inside any
    # function the step calls, so a failing command mid-function would pass.
    # A subshell with set -e of its own, outside any || context, stops at the
    # first failure; set +e around it keeps that failure from ending this shell.
    # Call step only at top level: on the left of || or && or inside an if, bash
    # disables set -e in the subshell too, and the guarantee silently goes.
    local status
    set +e
    (
        set -e
        "$@"
    )
    status=$?
    set -e
    if [[ $github -eq 1 ]]; then echo "::endgroup::"; fi
    if [[ $status -ne 0 ]]; then
        echo >&2
        if [[ $github -eq 1 ]]; then echo "::error::check failed: ${name}" >&2; fi
        echo "ERROR: step '${name}' failed (exit ${status}):" >&2
        echo "    $*" >&2
        echo "No later step was run. Fix this one, then re-run $0." >&2
        exit "$status"
    fi
    steps=$((steps + 1))
}

# sed rather than head: head exits early, and under pipefail the writer's
# SIGPIPE would fail the step.
environment() {
    rustc --version &&
        cargo --version &&
        qemu-system-aarch64 --version | sed -n 1p &&
        markdownlint-cli2 --version
}

step "Environment" environment
step "Toolchain pins" bash .github/scripts/check-toolchain-pin.sh
step "British spelling" bash .github/scripts/check-british-spelling.sh
step "Code spans and quotes" bash .github/scripts/check-code-spans.sh
step "Formatting" cargo fmt --all --check
step "Markdown" markdownlint-cli2 "**/*.md"

# Both Tier-1 architectures on every run, so neither becomes the one that only
# builds "usually" (§6). Scoped by package: the workspace's crates have
# incompatible targets, and an unscoped --target would try to build the host
# tool for bare metal.
for target in aarch64-unknown-none-softfloat x86_64-unknown-none; do
    step "kernel clippy (${target})" \
        cargo clippy --package setonix-kernel --all-features --target "$target" -- -D warnings
    step "kernel build (${target})" cargo build --verbose --package setonix-kernel --target "$target"
    step "kernel release build (${target})" cargo build --release --verbose --package setonix-kernel --target "$target"
    # The capability crate is architecture-independent no_std logic; building it
    # for both Tier-1 targets proves it stays no_std-clean.
    step "capability clippy (${target})" cargo clippy --package setonix-capability --target "$target" -- -D warnings
    step "capability build (${target})" cargo build --package setonix-capability --target "$target"
done

step "xtask clippy (host)" cargo clippy --package xtask --all-targets --all-features -- -D warnings
step "xtask tests (host)" cargo test --verbose --package xtask

# Host-testable kernel logic. The capability table's pure logic lives in its own
# crate precisely so it can run a test harness (a bare-metal target cannot).
step "capability clippy (host)" cargo clippy --package setonix-capability --all-targets --all-features -- -D warnings
step "capability tests (host)" cargo test --verbose --package setonix-capability

echo
echo "OK: all ${steps} checks passed"
