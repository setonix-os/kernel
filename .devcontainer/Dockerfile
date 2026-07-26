# Setonix development container — the pinned, reproducible workshop.
# Constitution: CLAUDE.md §7. Every tool version is pinned; upgrades are
# deliberate commits, never drift.

FROM mcr.microsoft.com/devcontainers/base:bookworm

# Keep in lockstep with rust-toolchain.toml at the repo root and with the
# dtolnay/rust-toolchain pin in .github/workflows/. Enforced by
# .github/scripts/check-toolchain-pin.sh, which runs in CI.
ARG RUST_VERSION=1.95.0

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
       curl ca-certificates git \
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
