# Setonix development container — the pinned, reproducible workshop.
#
# Constitution §7: every tool version is pinned, upgrades are deliberate commits,
# and *CI runs this same image*. That last clause is what makes the rest of this
# file load-bearing rather than a convenience: anything a contributor is asked to
# run must exist here, because there is no second environment to fall back on.
# If you add a check to CONTRIBUTING.md's checklist, add its tool here.

FROM mcr.microsoft.com/devcontainers/base:bookworm

# Keep in lockstep with rust-toolchain.toml at the repo root. Enforced by
# .github/scripts/check-toolchain-pin.sh, which runs in CI. CI takes its Rust
# from this image and must never install a toolchain of its own — that is the
# whole point, and the script fails the build if one reappears.
ARG RUST_VERSION=1.95.0

# Node supplies markdownlint-cli2 below. Pinned to a major line: Debian
# bookworm ships Node 18, which markdownlint-cli2 no longer supports.
ARG NODE_MAJOR=22
ARG MARKDOWNLINT_CLI2_VERSION=0.23.1

# --- System toolchain -------------------------------------------------------
# QEMU for both Tier-1 targets (qemu-system-arm ships qemu-system-aarch64),
# multi-arch gdb, UEFI firmware (ovmf = x86_64, qemu-efi-aarch64 = AArch64),
# mtools + dosfstools for rootless FAT/ESP boot images.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
       qemu-system-x86 \
       qemu-system-arm \
       gdb-multiarch \
       ovmf \
       qemu-efi-aarch64 \
       mtools \
       dosfstools \
       build-essential \
       curl ca-certificates git gnupg \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# --- Node and markdownlint, pinned -------------------------------------------
# Installed as root so the global install lands in /usr/lib/node_modules and is
# usable by every user in the image, including CI's.
#
# CONTRIBUTING.md tells contributors to run markdownlint-cli2. Since CI runs this
# image, the tool has to live here: otherwise the instruction is unfollowable
# inside the canonical environment, and CI would need a Node of its own — a
# second source of truth for a version, which §7 exists to prevent.
RUN export DEBIAN_FRONTEND=noninteractive \
    && curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && npm install -g "markdownlint-cli2@${MARKDOWNLINT_CLI2_VERSION}" \
    && npm cache clean --force \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# --- Rust, pinned -----------------------------------------------------------
USER vscode
ENV RUSTUP_HOME=/home/vscode/.rustup \
    CARGO_HOME=/home/vscode/.cargo \
    PATH=/home/vscode/.cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain ${RUST_VERSION} --profile minimal \
    && rustup target add aarch64-unknown-none x86_64-unknown-none \
    && rustup component add rust-src rustfmt clippy llvm-tools rust-analyzer

# --- Claude Code -------------------------------------------------------------
# Native installer (installs to ~/.local/bin). Alternative: the official
# Claude Code dev-container feature — see code.claude.com/docs/en/devcontainer.
RUN curl -fsSL https://claude.ai/install.sh | bash
ENV PATH=/home/vscode/.local/bin:$PATH
