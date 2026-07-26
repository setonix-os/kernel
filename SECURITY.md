# Security Policy

## Reporting a Vulnerability

**Do not report security vulnerabilities through public GitHub issues.**

Report privately via
[GitHub Security Advisories](https://github.com/setonix-os/kernel/security/advisories/new), or by
email to <matejg03@gmail.com> if you cannot use that route.

Please include:

- The component affected, and the commit or version
- A description of the vulnerability and what an attacker gains
- Steps to reproduce, ideally with a QEMU command line
- Which Tier-1 architecture you observed it on, and whether the other is affected

You will get an acknowledgement within a week. This is a one-maintainer project, so please be patient
with the timeline for a fix — but the acknowledgement will not be silent.

## Supported Versions

Setonix is pre-release. There are no supported versions yet and no security guarantees whatsoever.
Do not run it anywhere that matters.

## Threat Model

The project's threat model lives in the [docs repository](https://github.com/setonix-os/docs) and is
the authoritative statement of what Setonix defends against. In summary, from the constitution §9:

**Assets:** app integrity, user-data confidentiality, capability integrity, update-channel
authenticity.

**Adversaries:** malicious or compromised apps; compromised app authors shipping malicious signed
updates; network attackers; hostile inputs to servers.

**Trust boundaries:** kernel/userspace; app/broker; app/app; device/author.

**Out of scope initially:** physical access, hardware side channels, compromised toolchain.

That last exclusion is worth reading carefully before assuming a finding is in scope.

## What Counts as a Vulnerability Here

Because the whole system is built on capabilities, the interesting bugs are ones that manufacture
authority rather than ones that crash:

- Any way to obtain a capability that was never granted, or to widen the rights on one you hold
- Any ambient authority — a resource reachable without a capability naming it
- Confused-deputy paths through the broker
- Escaping a scheme's namespace
- A signed-update path that accepts an artefact the named author did not sign
- Kernel memory disclosure to userspace, or userspace-triggerable kernel memory corruption
- Anything that lets a crashing userspace driver take the system with it

A driver crash that is *contained* is working as designed, not a vulnerability.

## Security-Conscious Development

- `unsafe` is confined to designated modules and denied everywhere else by the workspace lint policy
- Every `unsafe` block states the invariant that makes it sound
- Capabilities are unforgeable, kernel-validated and reducible in rights — never widenable
- CI pins every GitHub Action to a commit SHA; a mutable tag is treated as a supply-chain hole
- The Rust toolchain is pinned in three places and CI refuses to build if they disagree

## Disclosure

Coordinated disclosure. We will agree a timeline with you, credit you unless you prefer otherwise, and
publish an advisory once a fix is available.
