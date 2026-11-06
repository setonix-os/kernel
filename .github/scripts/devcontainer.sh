#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds the devcontainer image and runs a command inside it, the way CI does.
# Both workflows call this script and pass docker no flags of their own, so
# the flags exist once. That matters beyond tidiness: a pull request reuses
# main's layers only when it builds the same Dockerfile with the same build
# arguments, and the one deliberate difference, --publish, changes neither.
#
#     bash .github/scripts/devcontainer.sh build [--publish]
#     bash .github/scripts/devcontainer.sh run -- COMMAND [ARG...]
#
# It runs on the host (the runner or a contributor's machine), never inside the
# container, and needs only docker with buildx, which GitHub's runners carry.
# DEVCONTAINER_IMAGE names the image, without a tag; the workflows set it in
# their `env` block.
#
# build
#   docker buildx build --load
#       Builds .devcontainer/Dockerfile on the default builder and loads the
#       result into the local image store, where `run` finds it and from which
#       ci_main.yml pushes it.
#   --build-arg BUILDKIT_INLINE_CACHE=1
#       Writes the cache metadata into the image's own config, so the published
#       image is also the cache. Nothing else is pushed.
#   --cache-from type=registry,ref=IMAGE:latest
#       Reuses the layers main published. The pull is anonymous.
#   --publish
#       Replaces --cache-from with --no-cache. The image main publishes is
#       built from nothing, so it never inherits a layer from the registry: any
#       branch in this repository can push there by editing a workflow, and a
#       published image must not depend on what one did.
#   The docker and buildx versions and the builder are printed first, so when
#   GitHub's runner image changes, the log says how.
#
# run
#   --rm --pull never
#       The image just built, never a registry copy in its place.
#   --mount ...,target=/root/kernel and --workdir /root/kernel
#       The repository at the workspaceFolder devcontainer.json names.
#   no --user
#       The image's own user, root. The checkout stays owned by the host's
#       user, which is why the git-listing check scripts pass safe.directory.
#   -e RUSTFLAGS -e CARGO_TERM_COLOR
#       A container inherits none of the caller's environment. These two are
#       forwarded by name, with the caller's values; a name the caller has not
#       set arrives unset, which is why CI's commands check RUSTFLAGS first.
#
# This replaces the devcontainers/ci action, which drove the dev container CLI,
# and keeps what CI relied on from it. Four things are deliberately not carried
# over, because CI used none of them:
#   - devcontainer.json's mounts and containerEnv, which serve interactive
#     sessions (Claude Code's login and the shell history);
#   - its postCreateCommand, which prints versions as check-all.sh's first step
#     does, and the CLI's login-shell environment probe, which for this image
#     yields the image's own PATH;
#   - the action's GITHUB_* file-command mounts, which nothing in the container
#     writes to;
#   - the CLI's devcontainer.metadata image label, which nothing reads:
#     "Reopen in Container" builds the Dockerfile, not the published image.
#
# Resolves paths relative to the repository root regardless of the caller's CWD.

set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "${script_dir}/../.." && pwd -P)
cd "${repo_root}"

usage() {
    echo "usage: $0 build [--publish]" >&2
    echo "       $0 run -- COMMAND [ARG...]" >&2
    exit 2
}

image_ref() {
    : "${DEVCONTAINER_IMAGE:?DEVCONTAINER_IMAGE must name the image, without a tag}"
    echo "${DEVCONTAINER_IMAGE}:latest"
}

build() {
    local publish=0
    case "$#:${1:-}" in
        0:) ;;
        1:--publish) publish=1 ;;
        *) usage ;;
    esac
    local image
    image=$(image_ref)
    local cache=(--cache-from "type=registry,ref=${image}")
    if [[ $publish -eq 1 ]]; then cache=(--no-cache); fi

    if [[ ${GITHUB_ACTIONS:-} == true ]]; then echo "::group::docker, buildx and builder"; fi
    docker version
    docker buildx version
    docker buildx inspect
    if [[ ${GITHUB_ACTIONS:-} == true ]]; then echo "::endgroup::"; fi

    docker buildx build \
        --load \
        --build-arg BUILDKIT_INLINE_CACHE=1 \
        "${cache[@]}" \
        --tag "${image}" \
        --file .devcontainer/Dockerfile \
        .devcontainer
}

run() {
    [[ ${1:-} == -- ]] || usage
    shift
    [[ $# -gt 0 ]] || usage
    local image
    image=$(image_ref)
    exec docker run --rm --pull never \
        --mount "type=bind,source=${repo_root},target=/root/kernel" \
        --workdir /root/kernel \
        -e RUSTFLAGS -e CARGO_TERM_COLOR \
        "${image}" \
        "$@"
}

case "${1:-}" in
    build)
        shift
        build "$@"
        ;;
    run)
        shift
        run "$@"
        ;;
    *) usage ;;
esac
