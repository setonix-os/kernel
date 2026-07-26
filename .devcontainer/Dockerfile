# Setonix development container — the pinned, reproducible workshop.
#
# Constitution §7: every tool version is pinned, upgrades are deliberate commits,
# and *CI runs this same image*. Anything a contributor is asked to run must exist
# here, because there is no second environment to fall back on. If you add a check
# to CONTRIBUTING.md's checklist, add its tool here.
#
# WHY THIS FILE DOES NOT USE APT FOR ITS PRIMARY TOOLS
#
# Debian 13 ships QEMU 10.0.11. Upstream QEMU is at 11.0.3. A whole major version
# of the emulator this project tests its kernel with, withheld by a packaging
# decision nobody here made — which is precisely the failure pillars 3 and 4 exist
# to remove. Building the development environment out of distro packages would
# mean depending on the thing Setonix is arguing against.
#
# So every tool that publishes upstream binaries or sources is fetched from its
# author, at a pinned version, and verified by hash or signature before use. That
# is pillar 3 (ship straight from author to user) and pillar 4 (exact, immutable,
# content-addressed closure) applied to our own workshop, by hand, because the
# host operating system will not do it for us.
#
# apt remains for exactly two things, each justified at its use site: shared
# libraries, and the handful of tools that publish no upstream binary at all. Each
# of those is a gatekeeper we have not yet removed, and is labelled as such.

# ---------------------------------------------------------------------------
# Fetch base — download and verification tooling, present in no final layer.
# Pinned by digest, not by tag: `debian:13-slim` is a moving target.
#
# KNOWN INHERITED VULNERABILITIES (checked 2026-07-26, `docker scout cves`)
#
# This base carries one critical and two high CVEs, all in Perl:
#   CVE-2026-12087  Socket < 2.041, out-of-bounds heap read in pack_ip_mreq_source
#   CVE-2026-48959  IO-Compress < 2.220
#   CVE-2026-48962  IO-Compress < 2.220
#
# All three are marked "Not Fixed" by Debian. Upstream fixed them — Perl 5.43.11
# and IO-Compress 2.220 — and the distribution has not shipped it, so
# `apt-get upgrade` changes nothing and re-pinning to a newer digest changes
# nothing. The same gatekeeper pattern as the QEMU version below, this time in the
# security path rather than the feature path.
#
# Accepted, with reasons, rather than ignored: Perl is `Essential` in Debian and
# cannot be removed; nothing in this image or in CI invokes it; and this is a
# build environment that never ships to a user. Re-pin the digest when Debian
# fixes it. If that wait ever becomes untenable, the answer is a base that is not
# a distribution — which is the argument this whole file is making.
# ---------------------------------------------------------------------------
FROM debian:13-slim@sha256:020c0d20b9880058cbe785a9db107156c3c75c2ac944a6aa7ab59f2add76a7bd AS fetch-base

USER root:root
WORKDIR /root
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        ca-certificates wget curl jq tar xz-utils gnupg && \
    rm -rf /var/lib/apt/lists/*

# ---------------------------------------------------------------------------
# Node — from nodejs.org, verified against the release's own SHASUMS256.txt.
# Exists solely to run markdownlint-cli2, which requires Node >= 22 (Debian 13
# ships 20.19, so apt could not supply it even if we wanted it to).
#
# Latest LTS line rather than Current: for a lint tool there is nothing to gain
# from a release line that is still changing, and Node's own guidance is LTS for
# anything you depend on.
# ---------------------------------------------------------------------------
FROM fetch-base AS node

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ARG NODE_VERSION=v24.18.0

RUN NODE_FILE="node-${NODE_VERSION}-linux-x64.tar.xz" && \
    NODE_BASE_URL="https://nodejs.org/dist/${NODE_VERSION}" && \
    wget -q --tries=3 --timeout=30 --retry-connrefused "${NODE_BASE_URL}/${NODE_FILE}" && \
    wget -q --tries=3 --timeout=30 --retry-connrefused "${NODE_BASE_URL}/SHASUMS256.txt" && \
    sha256sum --check --ignore-missing SHASUMS256.txt && \
    mkdir -p /opt/node && \
    tar -xJf "${NODE_FILE}" -C /opt/node --strip-components=1 && \
    rm "${NODE_FILE}" SHASUMS256.txt

# Separate layer, and its ARG declared after the fetch above, so that bumping the
# linter does not re-download and re-extract Node.
ARG MARKDOWNLINT_CLI2_VERSION=0.23.1

RUN PATH="/opt/node/bin:${PATH}" npm install -g "markdownlint-cli2@${MARKDOWNLINT_CLI2_VERSION}" && \
    PATH="/opt/node/bin:${PATH}" npm cache clean --force

# ---------------------------------------------------------------------------
# Rust — rustup-init from static.rust-lang.org's versioned archive, verified
# against the checksum published beside it, then used to install the exact
# toolchain rust-toolchain.toml names.
#
# Installed into /opt/rust rather than a home directory so that the toolchain
# belongs to the image, not to one user.
# ---------------------------------------------------------------------------
FROM fetch-base AS rust

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Keep RUST_VERSION in lockstep with rust-toolchain.toml at the repo root.
# .github/scripts/check-toolchain-pin.sh fails the build if they disagree, and
# also fails if any workflow installs a toolchain of its own.
ARG RUST_VERSION=1.97.1
ARG RUSTUP_VERSION=1.29.0

ENV RUSTUP_HOME=/opt/rust/rustup \
    CARGO_HOME=/opt/rust/cargo

RUN RUSTUP_BASE_URL="https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu" && \
    wget -q --tries=3 --timeout=30 --retry-connrefused "${RUSTUP_BASE_URL}/rustup-init" && \
    wget -q --tries=3 --timeout=30 --retry-connrefused "${RUSTUP_BASE_URL}/rustup-init.sha256" && \
    sha256sum --check rustup-init.sha256 && \
    chmod +x rustup-init && \
    ./rustup-init -y --no-modify-path --profile minimal \
        --default-toolchain "${RUST_VERSION}" \
        --component rust-src \
        --component rustfmt \
        --component clippy \
        --component llvm-tools \
        --component rust-analyzer \
        --target aarch64-unknown-none-softfloat \
        --target x86_64-unknown-none && \
    rm rustup-init rustup-init.sha256

# ---------------------------------------------------------------------------
# QEMU — built from upstream source. This is the stage that earns this file's
# structure: it is the difference between testing the kernel on the emulator its
# authors released and testing it on the one a packager froze three years of
# fixes ago.
#
# Verified twice. The tarball's SHA-256 is pinned below, so a reviewer can see
# exactly which bytes are expected and changing the version forces changing the
# hash in the same commit. The detached signature is then checked against the
# QEMU release key — author signing, which is pillar 3 as the QEMU project
# already practises it.
#
# Only the two Tier-1 softmmu targets are built. A full target list would
# multiply the build time for emulators this project will never run.
# ---------------------------------------------------------------------------
FROM fetch-base AS qemu

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Build dependencies first, and before the version ARGs below, so that bumping
# QEMU re-runs only the download and the compile — never this apt layer.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        build-essential python3 python3-venv ninja-build pkg-config \
        libglib2.0-dev libpixman-1-dev zlib1g-dev libslirp-dev libfdt-dev \
        flex bison && \
    rm -rf /var/lib/apt/lists/*
# libfdt-dev is not optional for this project: the aarch64 `virt` machine is
# described to the guest by a device tree, so QEMU will not configure without
# device-tree support. Its tarball carries a `dtc` subproject, but building that
# means letting meson fetch it at build time — an unpinned download, which is the
# one thing this file exists to avoid. The system library is the better answer:
# it is infrastructure, in the same category as glib and pixman, rather than a
# tool whose version changes our results.

ARG QEMU_VERSION=11.0.3
ARG QEMU_SHA256=da5fcffc32762820568b828ed430a728864d34d50b6d2f30358597760cbb0523
# Fingerprint of the QEMU release signing key, as published by the project. A
# public-key fingerprint is public by definition — there is no secret here, and
# naming it a fingerprint rather than a key says so precisely.
ARG QEMU_SIGNING_FINGERPRINT=CEACC9E15534EBABB82D3FA03353C9CEF108B584

# Download and verify in its own layer, separate from the compile below: a failed
# or re-run build should not mean fetching 135 MB again.
RUN QEMU_FILE="qemu-${QEMU_VERSION}.tar.xz" && \
    QEMU_BASE_URL="https://download.qemu.org" && \
    wget -q --tries=3 --timeout=60 --retry-connrefused "${QEMU_BASE_URL}/${QEMU_FILE}" && \
    wget -q --tries=3 --timeout=30 --retry-connrefused "${QEMU_BASE_URL}/${QEMU_FILE}.sig" && \
    echo "${QEMU_SHA256}  ${QEMU_FILE}" | sha256sum --check && \
    gpg --batch --keyserver keyserver.ubuntu.com --recv-keys "${QEMU_SIGNING_FINGERPRINT}" && \
    gpg --batch --verify "${QEMU_FILE}.sig" "${QEMU_FILE}" && \
    mkdir -p /root/qemu-src && \
    tar -xJf "${QEMU_FILE}" -C /root/qemu-src --strip-components=1 && \
    rm "${QEMU_FILE}" "${QEMU_FILE}.sig"

RUN cd /root/qemu-src && \
    ./configure \
        --prefix=/opt/qemu \
        --target-list=aarch64-softmmu,x86_64-softmmu \
        --enable-slirp \
        --disable-docs \
        --disable-gtk \
        --disable-sdl \
        --disable-curses \
        --disable-guest-agent \
        --disable-werror && \
    make -j"$(nproc)" && \
    make install && \
    cd /root && rm -rf /root/qemu-src

# ---------------------------------------------------------------------------
# Final image.
# ---------------------------------------------------------------------------
FROM debian:13-slim@sha256:020c0d20b9880058cbe785a9db107156c3c75c2ac944a6aa7ab59f2add76a7bd

LABEL org.opencontainers.image.title="Setonix Kernel Development Environment" \
    org.opencontainers.image.description="Pinned, reproducible workshop for building, booting and debugging the Setonix microkernel" \
    org.opencontainers.image.authors="MatejGomboc" \
    org.opencontainers.image.source="https://github.com/setonix-os/kernel" \
    org.opencontainers.image.vendor="https://github.com/setonix-os" \
    org.opencontainers.image.licenses="GPL-3.0-or-later"

USER root:root
WORKDIR /root

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ENV RUSTUP_HOME=/opt/rust/rustup \
    CARGO_HOME=/opt/rust/cargo \
    PATH="/opt/rust/cargo/bin:/opt/qemu/bin:/opt/node/bin:/root/.local/bin:${PATH}"

# LAYER ORDER IN THIS STAGE IS DELIBERATE
#
# Docker invalidates every layer after the first one that changes, so the cheapest
# thing to get wrong is the order. Rarely-changing work goes underneath
# frequently-changing work:
#
#   1. apt          — changes only when the dependency list does
#   2. COPY qemu    — changes only on a QEMU version bump (the rarest, and the
#                     most expensive to rebuild, so it sits lowest of the three)
#   3. COPY node    — changes on a Node or linter bump
#   4. COPY rust    — changes on every toolchain bump, so the most volatile
#   5. Claude Code  — unpinned, so treat it as changing constantly
#   6. verification — must be last; it checks the result of everything above
#
# Previously the COPYs came first, which meant a Rust bump re-ran the apt install
# underneath it for no reason.
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        libglib2.0-0t64 \
        libpixman-1-0 \
        zlib1g \
        libslirp0 \
        libfdt1 \
        gcc \
        libc6-dev \
        gdb-multiarch \
        ovmf \
        qemu-efi-aarch64 \
        mtools \
        dosfstools \
        git \
        ca-certificates \
        curl \
        xz-utils && \
    rm -rf /var/lib/apt/lists/*

# Each remaining distro dependency, and why it is still a distro dependency:
#
#   libglib2.0-0t64, libpixman-1-0, zlib1g, libslirp0, libfdt1
#       Shared libraries the QEMU built above links against. Building these from
#       source too would be dependency-chasing without benefit — they are
#       platform infrastructure, not tools whose version affects our results.
#       libfdt1 in particular is what lets the aarch64 `virt` machine hand the
#       guest a device tree at all.
#   gcc, libc6-dev
#       A linker for *host* binaries. The kernel itself needs no C toolchain —
#       the bare-metal targets link with rust-lld — but `xtask` is an ordinary
#       host program, and without `cc` cargo fails with "linker `cc` not found".
#       Not build-essential: that pulls in make, dpkg-dev and a compiler suite
#       this image has no other use for.
#   gdb-multiarch
#       GDB publishes no upstream binaries; a source build costs as much as
#       QEMU's for a debugger whose version matters far less. Debian 13 carries
#       16.3, which is recent. Revisit if a specific GDB feature is ever needed.
#   ovmf, qemu-efi-aarch64
#       UEFI firmware images. Upstream EDK II publishes no release binaries, and
#       these are Debian's builds of it.
#   mtools, dosfstools
#       Rootless FAT/ESP image construction. Stable, boring, and unchanged for
#       years — nothing to gain from a newer one.
#   git, ca-certificates, curl, xz-utils
#       Baseline plumbing.

COPY --from=qemu /opt/qemu/ /opt/qemu/
COPY --from=node /opt/node/ /opt/node/
COPY --from=rust /opt/rust/ /opt/rust/

# --- Claude Code -------------------------------------------------------------
# The one unpinned component in this file, and deliberately last. It is a
# development assistant, not a build input: it cannot influence the kernel image.
# Everything above it — compiler, emulator, linters — is pinned and verified
# because it *can*. Pin this too if that ever stops being true.
RUN curl -fsSL https://claude.ai/install.sh | bash

# --- Verification ------------------------------------------------------------
# Every tool is invoked, so that a missing shared library or a botched COPY fails
# the image build rather than someone's afternoon.
# Deliberately no `| head` anywhere below. `SHELL` sets `pipefail`, and truncating
# a tool's output closes the pipe under it: markdownlint-cli2 is a Node program
# and dies on the resulting EPIPE with an unhandled 'error' event, failing the
# build for a reason that has nothing to do with the tool being absent. A
# verification block should be verbose; let every tool say its whole piece.
#
# `--version` is also not universal: dosfstools has no such flag. For mkfs.fat and
# mtools the check below is functional instead — build a FAT image and write a file
# into it. That is what these tools will actually be asked to do when the x86_64
# ESP image lands, so it is worth more than a version string anyway.
RUN rustc --version && \
    cargo --version && \
    rustup target list --installed && \
    qemu-system-aarch64 --version && \
    qemu-system-x86_64 --version && \
    gdb-multiarch --version && \
    node --version && \
    markdownlint-cli2 --version && \
    printf 'int main(void){return 0;}' > /tmp/verify.c && \
    cc /tmp/verify.c -o /tmp/verify && \
    /tmp/verify && \
    rm -f /tmp/verify.c /tmp/verify && \
    echo "cc: OK (compiled and ran a host binary)" && \
    mcopy --version && \
    mkfs.fat -C /tmp/verify-esp.img 1440 > /dev/null && \
    mcopy -i /tmp/verify-esp.img /etc/hostname ::/ && \
    rm -f /tmp/verify-esp.img && \
    echo "mkfs.fat + mcopy: OK (built a FAT image and wrote to it)" && \
    git --version && \
    test -f /usr/share/AAVMF/AAVMF_CODE.fd && \
    test -f /usr/share/AAVMF/AAVMF_VARS.fd && \
    test -f /usr/share/OVMF/OVMF_CODE_4M.fd && \
    test -f /usr/share/OVMF/OVMF_VARS_4M.fd && \
    echo "OK: every pinned tool present and runnable"

# Firmware paths, recorded because the x86_64 boot path will need them and
# because the names are not stable across Debian releases — Debian 13 dropped the
# 2 MB OVMF builds entirely, so `OVMF_CODE.fd` exists on bookworm and does not
# exist here. The verification block above caught that on the first build, which
# is the entire argument for having one.
#
#   AArch64: /usr/share/AAVMF/AAVMF_CODE.fd   + AAVMF_VARS.fd
#   x86_64:  /usr/share/OVMF/OVMF_CODE_4M.fd  + OVMF_VARS_4M.fd
#
# The VARS files are templates: a run needs its own writable copy, never the
# shared one.

CMD ["/bin/bash"]
