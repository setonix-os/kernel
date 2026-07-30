<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Setonix threat model

| Field | Value |
|-------|-------|
| Status | **Authoritative** once merged — this document completes Phase 0 |
| Expands | [Constitution §9](../CONSTITUTION.md), which remains the seed and the summary |
| Cited by | RFCs argue their designs against the numbered obligations below |
| Maintained | Amendments are logged in [docs/CHANGELOG.md](CHANGELOG.md); flaws in this model are security reports — see [SECURITY.md](../SECURITY.md) |

## 1. Purpose

This document says what Setonix defends, against whom, and — most importantly — what the design is
therefore **obliged** to guarantee. The obligations carry stable numbers (`O-1`, `O-2`, …) so that
design documents can cite them: an RFC for the capability table or the IPC fast path should be able to
say "this mechanism discharges O-3 and O-4 at this boundary" and be checked against exactly that.
Obligations are never renumbered or deleted; one that is overtaken is marked *superseded*, for the same
reason rejected RFCs are kept.

A threat model for a system this young is necessarily a model of the *design*, not the deployment:
today the implementation is a kernel that boots, greets, and reports its own faults. §8 states which
obligations bind at which phase, so the model can be authoritative without pretending the broker
exists. What makes this worth writing now rather than later is §5.5 of the constitution: the
capability, IPC and app-format designs are about to be argued on paper, and this is the paper they are
argued against.

## 2. The system under threat

Everything in Setonix follows from one primitive:

> An application is a self-contained, signed, identity-bearing, immutable, content-addressed blob —
> and every resource it touches is a scheme mediated by a broker.

Security-relevant consequences of that primitive, restated as the mechanisms this model leans on:

- **Capabilities are the only authority** (§3). Unforgeable, kernel-validated, explicitly passed,
  reducible in rights and never widenable. There is no root, no ambient namespace, no permission bit —
  and therefore no privilege-escalation target of the classical kind.
- **All communication is kernel-mediated IPC.** Address spaces are disjoint; the syscall and IPC
  surface is the entire kernel attack surface from userspace.
- **Drivers are userspace processes.** A compromised driver is a compromised *process* holding device
  capabilities — severe, but bounded by what it holds, not by ring 0.
- **The store is authoritative for app bytes** (RFC-0001). Content-addressed, immutable,
  self-verifying: blobs are named by hash and verified against that name, so at-rest tampering is
  detected rather than prevented-by-hope. Only the store service holds write capability to its
  substrate.
- **Updates come signed from their author** (pillar 3). There is no curating middleman to compromise —
  which also means the author's key *is* the update channel, and the model must treat it accordingly.

## 3. Assets

Expanded from the §9 seed. Ordered by how much of the design exists to protect them.

- **A1 — Capability integrity.** The unforgeability, non-widenability and revocability of every
  capability in the system. This asset guards all the others: a single forged or widened capability
  converts any other asset's protections into suggestions.
- **A2 — App integrity.** An app's executable tree is exactly what its author signed — at rest in the
  store, and at the moment of loading. (Integrity *in memory after loading* is the MMU's job and
  belongs to A1's regime: writable-executable memory is a capability question.)
- **A3 — User-data confidentiality and integrity.** An app's data is reachable only through
  capabilities its owner granted. No app reads or corrupts another's data; no scheme leaks one
  client's data to another.
- **A4 — Update-channel authenticity and freshness.** What installs as version N+1 of an app is what
  its author published as N+1 — not an older signed version replayed (rollback), not a different
  author's blob, and not silence disguised as up-to-dateness (freeze).
- **A5 — Availability, as degradation rather than denial.** A server OS earns its keep by staying up.
  Setonix's specific availability claim is bounded: the failure of a driver or service is contained
  and restartable (§3), and no unprivileged process can starve the kernel or another process of the
  resources it was granted. Whole-system availability against a determined network flood is the
  deployment's problem, not the kernel's; *cross-process* starvation is ours.

## 4. Adversaries

Expanded from the §9 seed, with capabilities stated so threats below can be checked against them.

- **M1 — A malicious or compromised app.** Runs arbitrary code inside its own address space; holds
  every capability it was legitimately granted; speaks the syscall and IPC surface fluently and
  adversarially; can be many processes colluding. This is the *primary* adversary: pillar 2 exists so
  that a hostile app is an expected condition, not a breach.
- **M2 — A compromised app author.** Holds the author's signing key (theft, coercion, or a malicious
  author from the start) and can publish updates that verify perfectly. Can also *withhold* updates.
  What M2 cannot do, by construction, is touch apps or data outside what installations of their app
  are granted — M2's blast radius is M1's, times their install base.
- **M3 — A network attacker.** Observes, modifies, replays, reorders, delays or blocks any network
  traffic, including the update channel. Cannot break the cryptography.
- **M4 — Hostile input to server workloads.** Crafted requests to whatever service the machine
  exists to run. M4's goal is to become M1 by exploiting the app; the OS's obligation is that
  succeeding at that buys exactly M1's position and nothing more.
- **M5 — A compromised userspace driver.** M1, holding device capabilities. Named separately because
  hardware access changes the game: a driver whose device can bus-master (DMA) can, on hardware
  without an IOMMU/SMMU programmed against it, read and write physical memory the capability system
  never granted. This is the one adversary that can void A1 from below on the wrong hardware.

Out-of-scope adversaries are listed in §7.

## 5. Trust boundaries

The §9 seed's four, plus two the design has since made explicit.

- **B1 — Kernel / userspace.** The syscall and IPC surface. Everything crossing it is
  attacker-controlled by assumption (M1).
- **B2 — App / broker.** The broker holds authority apps do not; every request to it is a potential
  confused-deputy attack.
- **B3 — App / app.** Disjoint address spaces make isolation the default and spatial: two mutually
  untrusting apps share nothing unless a capability says so. The residual channels between them are
  IPC (B1's regime) and any scheme node both can reach (B2's regime), so B3 has no surface of its own —
  it is a property that B1 and B2 must jointly preserve, which is itself the claim to be checked.
- **B4 — Author / device.** The seed's "device/author": the crossing where a remote author's
  signed artefact becomes bytes on the user's machine. Everything on the author's side is untrusted
  until a signature is checked; everything the network carries between them is M3's. This is the update
  and install boundary, and A4 lives here.
- **B5 — App / store.** New, from RFC-0001. An app asks the store for bytes by hash; the store is
  authoritative and verifying. The boundary matters because a compromised store, or a substrate
  tampered with beneath it, must not be able to serve bytes that do not match the name requested —
  detection at this boundary is what makes immutability a property rather than a hope.
- **B6 — Driver / hardware.** New, made explicit by the userspace-driver decision (§3). A driver is a
  process (B1), but the device it holds is not bound by the capability system: a bus-mastering device
  can read and write physical memory directly. The boundary is therefore between the driver's granted
  authority and what its *hardware* can reach, and it is only enforceable with an IOMMU/SMMU programmed
  against the driver. This is the one boundary that hardware, not the kernel, ultimately holds.

## 6. Security obligations

The obligations the design must discharge, numbered for citation. Each carries a status:

- **Built** — the mechanism exists in the tree today and is exercised by CI.
- **Designed** — the design commits to it; the mechanism is not yet implemented. Most obligations are
  here, because the kernel currently boots, greets and reports faults, and little else.
- **Deferred** — acknowledged and deliberately not met in the initial target, with a stated reason
  (§7) or a named precondition.

The point of writing them now, mostly unbuilt, is §5.5: the coming RFCs are measured against these,
and a mechanism that cannot name the obligation it discharges has not justified itself.

### Capability integrity (A1)

- **O-1 — Unforgeability.** Userspace cannot fabricate a capability referring to an object it was not
  granted; capabilities are kernel-held tokens, never bare integers userspace can guess or construct.
  *(seL4.)* **Designed.**
- **O-2 — Non-widenability.** No operation increases the rights carried by a held capability;
  derivation only narrows. A held read capability can never become a read-write one. *(seL4; Fuchsia's
  downgradeable handle rights.)* **Designed.**
- **O-3 — Revocability.** Authority once granted can be withdrawn, and revocation reaches capabilities
  transitively derived from the revoked one. *(KeyKOS/EROS.)* **Designed.**
- **O-4 — No ambient authority.** Every resource access names a capability; there is no path to any
  resource that a capability does not gate — no root, no ambient namespace, no permission bit checked
  against process identity. *(The primitive itself.)* **Designed.**

### Kernel integrity (B1)

- **O-5 — Argument validation.** The kernel trusts no userspace-supplied pointer, length, or
  capability index; every syscall and IPC argument is validated against the caller's authority before
  use. **Designed** (there is no syscall surface yet).
- **O-6 — Kernel memory safety.** No spatial or temporal memory-safety violation in the kernel. `unsafe`
  is confined to modules the constitution designates (`arch/**`, `mm/**`), each block carrying a
  soundness argument, enforced by `unsafe_code = "deny"` at the workspace level and greppable in one
  command. **Built** — this is real today: the lint policy compiles the rule, and the `unsafe`
  register is reviewed per pull request.
- **O-7 — No unprivileged kernel exhaustion.** A process cannot make the kernel allocate unboundedly on
  its behalf, nor hold a kernel lock across a wait a process controls. This is the kernel's half of A5.
  **Designed.**

### Isolation and confidentiality (B3, A3)

- **O-8 — Address-space disjointness.** No process reads or writes another's memory except through a
  region both hold a capability to; W^X is a corollary (see O-13). **Designed** (the MMU regime is not
  built).
- **O-9 — No ambient side channel.** IPC and shared scheme nodes are the *only* cross-process channels;
  there is no global namespace, shared clock-writing surface, or ambient mutable state that leaks data
  or coarse timing between apps beyond what a granted capability already implies. Fine-grained hardware
  timing side channels are out of scope (§7). **Designed.**

### Broker and namespace (B2)

- **O-10 — No confused deputy.** The broker acts only on authority the requesting app actually holds
  and presents; it never substitutes its own, higher authority for a request. The request carries the
  capability. *(The classic confused-deputy defence; capabilities are the standard answer.)*
  **Designed.**
- **O-11 — No scheme escape.** A capability naming a node in a scheme's namespace grants no access
  outside the subtree it names; `..` and absolute re-rooting resolve within the granted view, not
  above it. *(Plan 9 per-process namespaces.)* **Designed.**

### App integrity and the store (A2, B5)

- **O-12 — Verified bytes.** An app's tree is checked against the author's signature at install and
  against its content hash at load; the store never serves a blob whose bytes do not match the name
  requested. *(Nix content-addressing; Haiku immutable packages; RFC-0001.)* **Designed.**
- **O-13 — W^X.** No page is simultaneously writable and executable, so a data-only memory-corruption
  bug in an app cannot be promoted to code execution. Enforced by the MMU regime. **Designed.**

### Update channel (A4, B4; adversaries M2, M3)

- **O-14 — Author continuity.** An update installs only if signed by the same author identity as the
  installed version, or by a successor key that identity explicitly authorised. A different author's
  signature is rejected however cryptographically valid. **Designed.**
- **O-15 — Rollback and freeze resistance.** The update path rejects a correctly-signed *older*
  version, and surfaces staleness rather than presenting an un-updated app as current — the two moves
  M3 uses to keep a known-vulnerable version installed while breaking nothing cryptographically.
  **Designed.**
- **O-16 — Bounded key compromise.** Author-key revocation is a first-class operation, and a compromised
  author's blast radius is bounded to that author's own apps — never system-wide. This is the price and
  the point of pillar 3: removing the distro gatekeeper also removes it as a single compromise target,
  and pushes the trust to per-author keys the system must be able to revoke. **Designed.**

### Drivers and hardware (B6, A5; adversary M5)

- **O-17 — Contained drivers.** A userspace driver's authority is exactly its device capabilities; a
  driver crash is contained and the driver restartable without kernel compromise. *(QNX; Redox.)*
  **Designed.**
- **O-18 — Confined DMA.** On hardware with an IOMMU/SMMU, device DMA is confined to memory the
  driver's capabilities cover. Without a programmed IOMMU this obligation *cannot* hold — a
  bus-mastering device bypasses the capability system entirely — and the model records that as a
  hardware precondition, not a defect to be fixed in software. **Designed** on IOMMU-equipped hardware;
  **Deferred** where no IOMMU is present (see §7).

### Supply chain and provenance (cross-cutting)

- **O-19 — Pinned, verified build.** The build environment has a single source of truth for every tool
  version; tools are fetched from their authors and verified by hash or signature, not taken on trust
  from a distribution. CI runs the same image it pins, so drift is a build failure. **Built** —
  §7 of the constitution, the devcontainer, and `check-toolchain-pin.sh`.
- **O-20 — Signed project artefacts.** Every commit is GPG-signed and verified; the paper trail is
  author-signed from the root commit. **Built** for commits; release-artefact signing is **Designed**,
  landing with the first release.

## 7. Out of scope

The §9 seed names three exclusions. Each is stated here with its reasoning and what would bring it into
scope, because an exclusion without a reason is indistinguishable from an oversight.

- **Physical access and the evil maid.** The initial target is server workloads (§6 of the
  constitution), where the operator has physical control by assumption. No secure boot, measured boot,
  or disk encryption is in the initial model. *Brought in by:* TPM-measured boot and full-disk
  encryption keyed to it — a post-Phase-4 hardening item, not a Phase-1 obligation.
- **Hardware side channels.** Spectre/Meltdown-class speculative leaks, cache and timing channels,
  Rowhammer. The microkernel's small trusted core and address-space disjointness reduce the surface but
  do not close these; they are properties of the silicon, not the kernel. *Brought in by:* cache
  partitioning per domain, speculation barriers on the IPC path, and constant-time discipline in
  crypto (which is the crypto library's obligation, not the kernel's). Named as future hardening.
- **A toolchain compromised below our pins.** O-19 verifies what we fetch and pins what we build with,
  but we trust the authors of `rustc`, LLVM, QEMU and every vendored crate up to the point of that
  verification. A reproducible-builds attestation and a bootstrappable toolchain would raise this
  floor. *Brought in by:* those, if the threat model ever demands defence against a subverted compiler.

Two further exclusions the design makes explicit, because they are boundaries the kernel structurally
cannot police:

- **The consenting user (confused user, not confused deputy).** The broker's job is to make a
  capability request *legible* — to show plainly what an app is asking for. It cannot make the
  decision. A user who grants a malicious app the capability it requests has authorised the result, and
  no kernel mechanism can distinguish that from a legitimate grant. This is a consent-UX obligation on
  whatever surfaces the broker's prompts, distinct from O-10.
- **Whole-system availability against a determined flood.** A5 claims *cross-process* non-starvation
  (O-7), not survival of a network-level denial-of-service aimed at the whole machine. That is the
  deployment's problem — rate limits, upstream filtering — not the kernel's.

## 8. Obligations by phase

The model is authoritative without pretending the broker exists. This section says which obligations
bind when, against the constitution's §10 roadmap, so the gap between *designed* and *built* is never
hidden.

| Phase | What lands | Obligations that become buildable, or bind |
|-------|-----------|---------------------------------------------|
| **Phase 1 — Iron** *(current)* | scheduler, IPC, capabilities, MMU | O-1..O-5, O-7, O-8, O-13 become implementable and must be discharged by the code that lands them; O-6, O-19, O-20 are already **Built** and must stay so |
| **Phase 2 — Voice** | scheme registry, first userspace driver | O-9, O-10, O-11, O-17 bind; O-18 becomes relevant as soon as a driver can DMA |
| **Phase 3 — App format** | signing, store, updater | O-12, O-14, O-15, O-16 bind; O-13's W^X becomes load-path-enforced |

Today, three of twenty obligations are Built. That ratio is the honest status of a Phase-1 kernel, and
stating it is the point: this document is the specification the ratio is meant to climb against, not a
description of a system that already holds the line.

## 9. Residual risks accepted

Given the scope, these risks are acknowledged and *not* mitigated, by decision rather than omission:

- **A malicious author within their own install base (M2).** Contained to that author's apps and
  revocable (O-16), but a user who installs a malicious app gets what they installed. The OS bounds the
  blast radius; it does not second-guess the user's trust.
- **A user who consents to a harmful grant.** See §7. The broker informs; it cannot decide.
- **DMA on IOMMU-less hardware (M5).** O-18 cannot hold there; such hardware is outside the security
  target, and the model says so rather than implying a protection that the silicon denies.
- **Diagnostic verbosity.** The exception reporter and panic handler print kernel addresses to the
  console. On the operator's serial line this is a feature; if a deployment exposes that console, it
  becomes an information leak. A deployment concern, flagged here so it is a decision and not a surprise.

## 10. Assumptions the model rests on

The obligations hold only if these hold. Each is a thing this model does not itself defend, and whose
failure invalidates conclusions above:

- **Cryptography is sound.** M3 cannot break the signature and hash primitives; a broken primitive
  voids O-12, O-14, O-15 at once.
- **The CPU implements its ISA faithfully.** No malicious silicon; the exception levels, MMU and
  (where present) IOMMU do what the architecture says.
- **The IOMMU, where present, is programmed correctly by us.** O-18 is our obligation to configure, not
  the hardware's to volunteer.
- **Author key management is the author's, up to our revocation.** The system supports revocation
  (O-16); it cannot prevent an author from losing their key, only bound the consequence.
- **The verified toolchain and vendored code are honest below O-19's checks.** See §7.

## 11. This is a living document

The obligations are the contract between this model and the code:

- **RFCs cite obligations.** A design document for a subsystem states which O-numbers it discharges,
  and at which boundary, and is reviewed against exactly that. "This mechanism discharges O-3 and O-4
  at B1" is a checkable claim; "it is secure" is not.
- **New surface, new obligation.** A subsystem that opens an attack surface this model does not cover
  adds an obligation, appended with the next free number. Obligations are never renumbered or deleted;
  a superseded one is marked, for the same reason a rejected RFC is kept (RFC-0002's practice).
- **A flaw in this model is a security report,** not a pull request — see [SECURITY.md](../SECURITY.md).
  A design flaw found on paper is the cheapest kind to fix, which is the whole argument for writing this
  before the code.
- **Amendments are logged** in [docs/CHANGELOG.md](CHANGELOG.md), dated, with reasoning — project law
  and the documents that expand it change in the open or not at all.
