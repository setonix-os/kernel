---
name: Bug Report
about: Report a bug to help us improve
title: '[BUG] '
labels: bug
assignees: ''
---

## Bug Description

A clear and concise description of what the bug is.

## Security Reminder ⚠️

If this is a **security vulnerability**, please do NOT open an issue. Instead, report it privately
via [GitHub Security Advisories](https://github.com/setonix-os/kernel/security/advisories/new).

**Before submitting, ensure you have:**

- [ ] Redacted anything sensitive from logs, memory dumps or register traces
- [ ] Removed any private signing keys or app-author identities from the report

## Steps to Reproduce

1.
2.
3.

## Expected Behaviour

What you expected to happen.

## Actual Behaviour

What actually happened. For a hang, say what the last console output was.

## Environment

Delete the rows that do not apply.

| Field | Value |
|-------|-------|
| Architecture | `aarch64` / `x86_64` |
| Machine | QEMU `virt` / QEMU `q35` / real hardware (give model) |
| Firmware | AAVMF / OVMF / vendor UEFI (give version) |
| Emulation | TCG / KVM |
| Rust version | output of `rustc --version` |
| Commit | short SHA of the kernel you built |
| Host | Windows + WSL 2 / Linux / macOS |
| Devcontainer | yes / no |

## Console output

<!--
The serial console is the primary diagnostic. Paste the full output from reset
onwards, not just the last line — the boot sequence is usually where the real
cause is visible. Use a fenced block.
-->

```text

```

## Does it reproduce on the other architecture?

- [ ] Yes — happens on both `aarch64` and `x86_64`
- [ ] No — this architecture only
- [ ] Not tested

<!--
Worth the effort before filing: a fault that appears on exactly one architecture
usually means the HAL boundary has leaked, which is a different and more
interesting bug than the symptom suggests.
-->

## Additional context

Anything else — a `gdb` backtrace over the QEMU stub, a QEMU command line, a bisected commit range.
