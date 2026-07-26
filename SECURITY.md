# Security Policy

## Reporting a Vulnerability

This repository contains documents, not code, so it is rarely the right place to report a
vulnerability. Report implementation vulnerabilities against
[`setonix-os/kernel`](https://github.com/setonix-os/kernel/security/advisories/new).

Report **here** if the flaw is in the design itself — a threat model that omits a real adversary, an
RFC whose scheme is unsound, a capability rule that permits widening. A design flaw found on paper is
the cheapest kind to fix, which is the entire argument for this repository existing.

Use [GitHub Security Advisories](https://github.com/setonix-os/docs/security/advisories/new), or email
<matejg03@gmail.com>. You will get an acknowledgement within a week.

## Threat Model

`threat-model.md` is the authoritative statement. The constitution §9 holds the seed it expands:

**Assets:** app integrity, user-data confidentiality, capability integrity, update-channel
authenticity.

**Adversaries:** malicious or compromised apps; compromised app authors shipping malicious signed
updates; network attackers; hostile inputs to servers.

**Trust boundaries:** kernel/userspace; app/broker; app/app; device/author.

**Out of scope initially:** physical access, hardware side channels, compromised toolchain.

## Supported Versions

Setonix is pre-release. Nothing here describes a shipped system, and no security guarantees are offered.

## Disclosure

Coordinated disclosure. We will agree a timeline, credit you unless you prefer otherwise, and publish
an advisory once the design is corrected.
