<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Prior-art review — virtualisation and containment

| Field | Value |
|-------|-------|
| Status | Research record — informs the threat model, RFC-0001, RFC-0003a, and the future broker, scheme-registry, driver-model, MMU, scheduler and updater RFCs |
| Date | 2026-08-01 |
| Method | Six parallel research sweeps (four planned domains, two critic-identified gaps), primary sources only, every lesson mapped to a pillar, an obligation by number, or the RFC it feeds |
| Companion | [research/0001](0001-capabilities-and-ipc-prior-art.md) stress-tested the capability and IPC designs against the microkernel lineage; this review mines the virtualisation and containerisation fields — the people who spent two decades learning microkernel lessons under production fire, and published their scars |

## Purpose

The maintainer's observation prompting this review: the virtualisation and containerisation
communities have accumulated proven best practices — and proven failures — that an original
kernel should inherit deliberately rather than rediscover. The constitution's "keep the proven,
prune the legacy" applied to a field young enough that its postmortems name CVE numbers.

The sweep covered hypervisors and microVMs (Firecracker, pKVM, NOVA, Xen, KVM, Jailhouse),
container runtimes and sandboxes (namespaces, cgroups, seccomp, gVisor, Kata, WASI), images and
supply chain (OCI, Nix, ostree, TUF, sigstore, reproducible builds, attestation), and resource
control at scale (cgroup v2, PSI, Kubernetes, Borg, seL4 MCS). A completeness critic then
checked coverage against all twenty obligations and found two load-bearing holes — O-10
(confused deputy) was the only obligation no lesson fed, and pillar 5 (schemes) was the
thinnest-covered pillar — which two further sweeps closed: control-plane confused-deputy
failures (Kubernetes API server, RBAC, Vault), and the path-resolution escape corpus plus its
cures (Capsicum, `openat2`, Landlock).

**Verdict in one paragraph.** The fields *converge on Setonix's architecture from the other
side*: the container era's survivors (gVisor, Kata, Firecracker, WASI) each rebuilt some part
of "narrow capability interface, userspace services, immutable signed artefacts" as a retrofit,
at retrofit prices. Sixteen lessons are validation of decisions already made — several with
quantified evidence the RFCs can now cite. The rest are genuinely new inputs: the store's
naming scheme must be decided early (Merkle roots — the one decision ostree shows cannot be
retrofitted cheaply); the broker inherits a decade of confused-deputy failures as design rules
and three proposed obligations; the scheme registry gets its resolution semantics practically
dictated by Capsicum and `openat2`; and three graves are proposed for the constitution's list.
Three mechanisms are rejected outright.

## Part 1 — The validation record

What the fields confirm about decisions already made, now citable with production evidence.

- **No ambient authority (O-4, pillar 2).** Linux namespaces isolate the *view* while leaving
  *authority* ambient; a decade of escapes proves the retrofit structural. Measured: enabling
  unprivileged user namespaces raised reachable privileged kernel operations from 8/40 to 27/40
  (Edera, kernel 6.18); 40+ kernel CVEs since 2020 required or were eased by user namespaces
  (CVE-2022-0185, CVE-2024-1086, CVE-2023-2640). Hiding names does not remove authority;
  Setonix's authority-isolation is what these systems reach for and cannot retrofit.
- **Capability-indexed syscalls (RFC-0003 §9) over argument inspection.** seccomp-bpf cannot
  dereference pointers *because of TOCTOU* — a racing thread swaps the argument between filter
  and use — so it can only gate "may call `open` at all", never "may open this". Authority-in-
  the-handle has no argument to re-inspect and no window to race. The mainstream filtering
  mechanism fails precisely where the capability ABI has nothing to fail at. *(kernel.org
  seccomp docs; libseccomp #59.)*
- **Immutable signed apps (pillar 1).** runc CVE-2019-5736: a mutable, container-reachable
  runtime binary overwritten through `/proc/self/exe` = host root; the accepted fix was
  literally "copy the binary into a sealed immutable memfd". The CVE class evaporates when
  binaries are content-addressed, verified and unreachable by ambient path.
- **Userspace drivers (kernel doctrine, O-17).** Linux moved the virtio network backend *into*
  the kernel for speed (`vhost_net`); CVE-2019-14835 was the resulting guest-to-host-kernel
  escape, and the industry walked it back (vhost-user, vDPA). The monolithic-driver mistake,
  recreated in the 2010s, exploited, and reversed — the recorded answer when Phase 2 drivers
  feel slow is *make IPC faster, never move the driver in*.
- **No privileged catch-all domain (O-4).** Xoar (SOSP '11) quantified Xen's dom0: 21 of 23
  guest-originated attacks hit control-VM services, only 2 the hypervisor proper. And Nexen
  (NDSS '17) classified 191 Xen XSAs: 75% sit in the privileged core, over half in per-VM
  logic, as the "thin" hypervisor grew 45K→270K LoC. Accretion into the privileged layer is
  the measurable failure mode "coherence over accumulation" exists to prevent.
- **The narrow-interface convergence.** gVisor interposes a memory-safe reimplementation
  between apps and a host surface of ~53–68 syscalls, delegating file access to a
  lesser-privileged Gofer ("the closer you are to the untrusted application, the less privilege
  you have") — the broker + userspace-services shape, paid for as an emulation tax. Kata wraps
  every pod in a VM — the industry's confession that shared-kernel isolation failed, and a
  standing caution: the justification for not paying Kata's per-app VM tax is a TCB small
  enough to stay verifiable. If the microkernel grows baroque, the hardware-isolation argument
  reasserts itself.
- **Pillar 2 shipping at ecosystem scale.** WASI's component model starts every component with
  *no ambient authority* — no global namespaces at runtime, capabilities passed explicitly at
  instantiation (preopens), unforgeable, per-resource, interposable. Answers "is a strict
  no-ambient-authority ABI usable?" with a mainstream ecosystem. Its residual ambient bits
  (clock, randomness) are a reminder to audit that Setonix's three authority-free syscalls stay
  genuinely authority-free.
- **Capability-shaped grants, independently re-derived.** Kubernetes walked its service-account
  tokens from ambient forever-valid bearer secrets to audience-bound, time-bound, *object*-bound
  tokens invalid the moment the bound object dies (KEP-1205) — "valid only while the object
  exists" is RFC-0003 §8's generation check, reinvented. And the runc maintainer's own tally —
  ~14 CVEs since 2017, all "operating on path strings" — ended in libpathrs' Root/Handle
  pattern: resolve once to a handle, operate on the handle, never the string again. The
  containerisation world spent a decade of CVEs arriving at Setonix's starting point.
- **Mechanism/policy split at hyperscale.** Borg (EuroSys '15) runs admission, priorities,
  overcommit and reclamation entirely in the userspace control plane; the kernel only enforces
  computed budgets. Across every system in this sweep, kernel-resident *policies* (OOM badness,
  v1 soft limits, CFS quota heuristics) churned and failed, while kernel-resident *accounting
  and enforcement* endured. Four decades of resource-control churn localises exactly where the
  doctrine said it would.
- **Partition-budget scheduling has production proof.** QNX adaptive partitioning — guaranteed
  minima binding only under overload, work-conserving, priority-preemptive within partitions —
  has shipped in safety-critical systems for ~20 years, confirming that budget scheduling
  composes with a synchronous message-passing microkernel. One transferable caveat: its
  accounting window and billing granularity proved subtler than documented (ECRTS '21); the
  scheduler RFC should state both as design, not leave them as implementation detail.
- **The xz backdoor is M2, realised.** CVE-2024-3094: a multi-year social-engineering campaign,
  a payload present only in the release tarball and never in the repository, perfect signatures
  throughout. The threat model's answer (blast radius = that app's grants, revocable) is the
  correct one; pillar 4's exact closures make "which installed apps contain the poisoned blob"
  a single store query — the response capability distros reconstructed by hand that weekend.

## Part 2 — The store, images and supply chain (informs RFC-0001 and the updater RFC)

The most decision-forcing part of the review: several of RFC-0001's open questions now have
field-tested answers, and the *naming scheme* emerges as the one decision that must be made
before the store is built.

### What the field teaches

- **A set of verified blobs is not a verified composition.** OCI learned that naming each layer
  by its own digest permits mix-and-match substitution, and added ChainID — a compound hash
  binding each layer to everything beneath it. TUF names the same attack. **Adopt:** an app's
  closure is named by a single Merkle root covering every member blob and edge; the author
  signs the root; the store refuses to instantiate a closure asserted by anything less.
  *(OCI image-spec; TUF.)*
- **Merkle naming answers "verify every read" at page granularity.** ostree shipped
  content-addressed atomic upgrades but verified only at download; at-rest tampering went
  undetected for a decade until composefs retrofitted fs-verity per-page Merkle verification —
  at real cost, because the object naming didn't anticipate it. Android's dm-verity has run
  per-block hash-tree verification on consumer phones since 2016, proving the read-time cost
  acceptable. **Adopt:** blobs are named by the root of a Merkle tree over fixed-size chunks,
  *decided now* even though the store substrate is deferred — verification then lives naturally
  at the store/MMU boundary, discharging O-12's "at load" clause continuously. This dissolves
  RFC-0001 open question 1.
- **The references graph is store metadata, not application knowledge.** Nix's twenty-year
  record: closure edges recorded at registration, GC as pure reachability from roots, rollback
  as repointing a root. RFC-0001 names GC as "where the difficulty actually lives"; Nix's
  answer is that it is only tractable in this shape. Corollaries: forbid blobs containing their
  own store address (keeps content-addressing exact rather than heuristic); and an exact
  recorded closure *is* a complete SBOM by construction — carry the NTIA minimum fields in the
  manifest and SBOM export becomes a store query, where post-build scanners systematically miss
  static links. *(Dolstra 2006; Nix RFC 062; NTIA 2021.)*
- **Content-addressing gives integrity, never freshness.** A signature over an immutable blob
  proves nothing about currency — an M3 attacker replaying yesterday's perfectly-signed world
  is invisible to Nix's scheme, and Nix's single long-lived cache key is a whole-ecosystem
  compromise target (the Trustix critique — which also validates pillar 3's no-central-signer
  choice). **Adopt:** O-15's freeze detection needs a separate short-expiry author-signed
  "latest as of T" statement (TUF's timestamp role, per-author); its staleness surfaces to the
  operator as a named condition, never silence.
- **The TUF attack taxonomy exceeds the threat model's current coverage.** O-15 names rollback
  and freeze; TUF's production-derived list adds **fast-forward** (inflate version counters to
  brick the update channel), **mix-and-match** (individually-valid files that never coexisted),
  **endless-data / slow-retrieval** (transport exhaustion), and **wrong-software** (a
  validly-signed artefact other than the one requested — tag mutability). Proposed threat-model
  amendments: version counters resettable only by an author-key-signed epoch bump; all-or-nothing
  closure installation against one signed root; every fetch bounded by expected length and hash
  before bytes flow; name-to-hash binding through signed metadata only.
- **Opt-in signing converges to zero.** Docker Content Trust — a real TUF deployment — retires
  in 2026 after a decade at <0.05% adoption: signing that is separate from the artefact flow
  does not get used, however sound. Validates pillar 1's signing-by-construction (the store has
  no interface for an unsigned blob). Two additions: the verifier itself is prime attack
  surface (cosign CVE-2022-23649 accepted the wrong signature's transparency bundle — keep the
  verification path minimal, one module, adversarial test vectors); and evaluate an append-only
  transparency log for author keys and version statements — the only known mechanism that makes
  an M2 attacker's covert signed update *observable*.
- **Rollback-attack resistance and operator rollback are different mechanisms.** Android AVB's
  monotonic rollback index advances only after successful boot; ostree keeps the previous
  deployment bootable through an atomic swap. Conflate them and you ship either replay
  vulnerability or anti-rollback bricking. **Adopt both halves:** the previous closure stays
  GC-rooted until the new one is health-confirmed (nearly free under content addressing);
  per-author monotonic counters bind the *network*, never the operator's local decision to
  boot the retained version.
- **Determinism dies by a thousand ambient inputs, and signatures live outside the content.**
  Timestamps cause 87–97% of reproducibility failures; Debian needed a decade to claw its
  archive back — never lose determinism rather than retrofit it (double-build-and-compare in CI
  from the first release). And an embedded signature changes the artefact's bytes, destroying
  both reproducibility and stable content-addressing — so RFC-0001's question 3 has a forced
  shape: sign the closure-root hash, keep signatures in detached metadata, as Nix and OCI
  concluded independently. *(reproducible-builds.org; Guix full-source bootstrap as the
  existence proof for §7's "toolchain below the pins" bring-in path.)*
- **A signature proves origin, not build integrity.** SolarWinds' SUNSPOT swapped sources in
  memory during builds and let the legitimate release key notarise the attacker's work; Codecov
  leaked a credential in a public image layer and served a silently-modified installer for two
  months, caught by one customer comparing hashes by hand. **Adopt:** O-20's release signing
  runs where build steps cannot reach the key (a separate signing step over a finished, hashed
  artefact); CI lints that no blob or image layer carries credential material; and note that
  the check Codecov's customer performed manually is what a content-addressed store performs on
  every serve, by construction. *(SLSA Build L3 is the field's distilled requirements list.)*
- **A content-addressed app is natively measurable.** SEV-SNP/TDX attestation reports hash the
  launch state; since a Setonix app's identity *is* its closure hash, "attest exactly which
  closures this server runs" is a report over names the system already keeps — a capability no
  incumbent OS gets for free, prepared entirely by decisions already made. Caveats from the
  wild: BadRAM (CVE-2024-21944) forged SEV-SNP attestation with $10 of hardware until firmware
  validated DIMM metadata — verifiers must pin minimum firmware/TCB versions (the AVB lesson
  one layer down), and the threat model's physical-access exclusion stays honest and stated
  when measured boot is brought in.
- **Reject: any cache keyed on less than the full input hash.** Docker's build cache keys `RUN`
  steps on the *command text* — stale or attacker-imported results serve forever — and
  BuildKit's shared cache mounts produced a container escape (CVE-2024-23651, Leaky Vessels).
  Every cached artefact in CI, xtask or the store's future substituter path is keyed on the
  hash of its complete input closure, and imported cache entries are verified like store blobs:
  untrusted input, never trusted state.

## Part 3 — The broker as attack surface (informs the broker RFC; proposes O-21…O-23)

Research/0001 flagged the broker as "an undesigned single point of authority". The
gap sweep turned that flag into design rules, mined from the component that plays the broker's
role at planetary scale — the Kubernetes API server — and its decade of confused-deputy CVEs.
O-10 was the one obligation of the twenty that no planned sweep fed; it is now the
best-evidenced.

- **The mediator must never re-originate a request under its own authority.**
  CVE-2018-1002105: the API server proxied client requests over a connection authenticated with
  *its own* credentials; crafted requests then rode that connection bearing the server's
  identity — cluster-admin from near-zero privilege. The capability model's answer is
  structural (the app's capability travels with the request), but the broker RFC must state the
  negative rule: the broker holds no standing downstream credential it could ever splice in;
  any authority it holds for a mediation is the specific capability handed to it, consumed and
  not retained.
- **Unattributable mediation is where escalation hides — proposed O-21 (mediation
  auditability).** The injected requests were invisible to audit logs precisely because the
  deputy re-originated them. And Kubernetes' own impersonation feature shows the constructive
  form: "act as" is dual-attributed (real actor + principal), never collapsed. Proposed
  obligation: every act the broker performs on an app's behalf is attributable to the specific
  principal *and* the specific capability that authorised it, bound at the kernel boundary, not
  forgeable or omittable by the broker itself.
- **A mediator that follows redirects re-targets credentials.** CVE-2020-8559: the API server
  re-sent credentialed requests to whatever location a compromised backend's redirect named.
  Scheme resolution must be fixed at grant time against the capability's named subtree; a
  mediated operation is never re-aimed by data returned from the operation itself.
- **Externalised grants carry an audience.** Legacy tokens' core defect: "any recipient can
  masquerade as the presenter to anyone else." In-kernel capabilities are immune (handles, not
  bearer blobs), but wherever the broker mints authority that leaves the kernel table — bytes
  on a wire for a network peer — the grant names its one intended holder and is rejected from
  anyone else. Never mint a grant valid for whoever holds it.
- **The grantor invariant, and the escalate-verb refusal.** RBAC's anti-escalation rule (you
  cannot grant above your own ceiling) is exactly O-2 — but Kubernetes then defined `escalate`
  and `bind` as sanctioned exceptions, and wildcard grants silently include them. The broker
  gets no operation that mints rights it does not hold, and no wildcard/blanket grant verb
  whose expansion could quietly include one — every broker grant right enumerated and
  orthogonal (the CAP_SYS_ADMIN grave, at the broker layer).
- **Proposed O-22 (broker compromise containment).** The CNCF/Trail of Bits audit's root
  finding was that the API server's *breadth* was the weakness — every added verb and proxy
  path new surface on the most privileged process. Proposed obligation: the broker's own
  compromise is contained — holding only in-flight mediation capabilities and no standing
  ambient authority, its blast radius is those grants, and the broker RFC must *prove* the
  bound. Structure the broker as several least-privilege processes (grant policy, prompt
  surface, registry — the Xoar decomposition), each holding only what its function needs.
- **Proposed O-23 (fail-closed mediation, degradation bounded).** Kubernetes admission webhooks
  force the dial: fail-open silently skips policy; fail-closed wedges the cluster (and
  aggregated timeouts break even fail-open's promise). For authority, mediation fails closed —
  fail-open on a permission broker is ambient authority by outage. What bounds the damage is
  Setonix's structure: granted capabilities are kernel-held and do not re-consult the broker
  per use, so a broker outage stalls *new* grants, never live ones. State that property as the
  reason the broker is off the hot path.
- **The broker must not gate its own bootstrap.** The self-hosted-webhook deadlock (a mediator
  gating the creation of its own pods) generalises: RFC-0003 §14.4's answer must mint init's
  capabilities — including the broker's own authority to mediate — by a one-time kernel act at
  boot, with the bootstrap grant graph stated explicitly and provably acyclic. Nothing the
  broker needs to start may depend on the broker running.
- **Reject: a bootstrap superuser.** Kubernetes' `system:masters` bypasses RBAC entirely — the
  pragmatic "who grants the first grant" answer that becomes a permanent, policy-immune
  escalation target; every RBAC-escalation writeup ends there. The bootstrap seam is
  unavoidable; a *standing identity that bypasses the capability system* is not. Init's
  capabilities are ordinary, enumerable, revocable, minted through the same mechanism as
  everything else.
- **Secret-zero has a proven pattern.** Vault's response-wrapping delivers first credentials as
  a single-use, short-TTL *reference* whose prior consumption is detectable — a stolen-in-transit
  bootstrap grant is either useless or evident. Adopt the properties (single-use +
  prior-use-detection) for delivering initial capabilities across imperfectly-trusted spawn
  channels, not Vault's token machinery.
- **Interpose at grant time, never per-operation.** gVisor's measured tax — syscalls 2–11×
  native, Sentry→Gofer calls ~72× — is the cost of a mediator in every operation. This
  independently supports RFC-0004's capability-transfer-free fast path and weighs on
  RFC-0003a's fork: grant-time forwarders keep the broker off the hot path; per-call mediation
  inherits the Gofer tax. Measure any broker design against those figures.
- **Containment granularity is a grant-time policy choice.** NOVA and seL4-as-hypervisor run
  one deprivileged VMM *per guest* (~1% overhead), so compromising your VMM owns only your own
  resources — and proves the capability-microkernel doctrine scales to full hypervisor duty
  without new kernel mechanisms. Where a service's state is per-client, the broker may choose
  instance-per-client over one multiplexing server at grant time, collapsing the cross-client
  leak surface (A3) — containment granularity as policy, exactly where policy lives.

## Part 4 — Schemes and name resolution (informs the scheme-registry RFC; O-11's mechanism)

Pillar 5 was the thinnest-covered pillar in the planned sweeps; the gap sweep found its
specification essentially written — by the systems that spent a decade paying for its absence.

- **Capsicum is O-11's peer-reviewed shape.** After `cap_enter()`, the *only* way to name a
  file is `openat()`-relative lookup from a held directory descriptor; absolute paths and
  boundary-crossing `..` are refused in the resolver; descriptor rights only narrow. A decade
  of FreeBSD deployment. Its documented adoption pain — POSIX software must be rewritten to
  pass descriptors — is a feature for Setonix, which has no POSIX surface to retrofit and
  forbids ambient naming from syscall one. Lead the scheme-registry RFC with this citation.
- **`openat2()`'s RESOLVE flags are the specification.** `RESOLVE_BENEATH`,
  `RESOLVE_IN_ROOT`, `RESOLVE_NO_MAGICLINKS`, `RESOLVE_NO_XDEV` — evaluated *during*
  resolution, atomically, against the pinned root. The scheme resolver behaves as if all four
  were permanently on against the granted subtree: bounded to the granted view, no implicit
  boundary crossing, no alias node that leaves the view. Linux bolted these onto a 30-year-old
  `namei()`; for Setonix they are the first resolver's spec, not flags.
- **Resolution is by construction, never a check.** CVE-2021-30465: runc validated a path, then
  used the path — and a `RENAME_EXCHANGE` race swapped it between the two, bind-mounting the
  host root into the container. Any resolve-then-act-on-the-string design re-does the lookup
  under attacker-mutable state. The scheme server resolves a name *once* to a kernel-held node
  object and operates on that object thereafter; "check the path then use the path" is
  forbidden the way ambient syscalls are.
- **No node whose resolution terminates outside the grant.** `/proc/self/exe` resolves across
  mount namespaces — a node *inside* the view aliasing an object *outside* it — which is how
  CVE-2019-5736 reached the host binary. The registry forbids magic/alias/pass-through nodes by
  construction; anything exposing "the caller's own executable" is an immutable store blob
  (O-12), never a live alias.
- **The server's own dependencies never come from the subtree it serves.** CVE-2019-14271:
  Docker's helper chroot'd into the container filesystem and then loaded its own NSS libraries
  from it — attacker code at host privilege. Rule for every scheme server: its code and trust
  anchors resolve from the immutable store before it binds the served view; served subtree and
  server execution environment are disjoint capability domains.
- **Spawn is a capability-transfer event.** CVE-2024-21626 (Leaky Vessels): one file descriptor
  leaked into a container's init became full host filesystem traversal via
  `/proc/self/fd/N`-as-WORKDIR. An inherited-by-default descriptor table is ambient authority
  smuggled across exec. The future spawn RFC states it as an obligation: capability inheritance
  is explicit-only and default-empty — a process receives exactly what the spawner moves to it.
  (Landlock's documented inability to restrict already-open fds is the same lesson from the
  retrofit side.)
- **Composition is intersection.** Landlock stacks up to 16 rulesets, most-restrictive-wins,
  irreversible. Adopt for namespace composition: when a process layers scheme views, effective
  authority is the intersection and no layer re-widens — O-2's monotonicity applied to
  namespaces.
- **Plan 9's model survives the corpus only capability-mediated.** The constitution names Plan 9
  per-process namespaces as pillar 2's substrate — yet mount namespaces, their Linux
  descendant, are the escape surface of this entire corpus. What failed was not composition but
  *path-string resolution against a mutable namespace*. The reconciliation the scheme-registry
  RFC must state: keep Plan 9's per-process composition; prune its string resolution. Every
  bind is a capability to an immutable subtree object; resolution walks held capabilities and
  never re-parses a path against the live namespace.
- **"A capability to a subtree" gets a concrete calculus.** Capsicum's directory descriptors —
  rights-carrying, reachability-narrowing, monotone — instantiate RFC-0003's subset-only
  derivation on the *node* object: a scheme-node capability carries content rights plus a
  traverse/derive-child right; deriving a child capability narrows the reachable subtree and
  cannot widen rights. O-11 then falls out of O-2 — you cannot derive upward because derivation
  only narrows, and there is no operation naming a node you were not handed. This closes the
  gap entirely within the accepted capability calculus; no new subsystem.

## Part 5 — Drivers, devices and DMA (informs the driver-model RFC; O-17, O-18)

- **Absent beats disabled.** VENOM (CVE-2015-3456) escaped through QEMU's floppy controller
  *even when no floppy was configured* — the code was compiled in and reachable for 11 years.
  Firecracker's answer was structural: ~50k LoC offering exactly four devices, and block-level
  ABI over filesystem passthrough as an explicit security decision. Design rules: a
  driver/scheme server no granted capability requires is not spawned and holds nothing; at
  trust boundaries prefer the narrowest ABI, pushing semantic complexity to the client side.
- **Spawn-time attenuation is the security parameter.** Firecracker's jailer drops privileges
  before the VMM's first instruction ("all vCPU threads are considered to be running malicious
  code"); crosvm sandboxes each virtio device in its own jail. The initial grant, not only
  non-widenability, is what security rests on: the broker RFC specifies the spawner/granter
  split in process creation, and drivers partition capabilities internally by role (data path
  vs control path).
- **Adopt virtio as the first driver ABI — with both sides adversarial.** The virtqueue —
  descriptor ring in shared memory established once, payload-free doorbells — is structurally
  RFC-0004's shape (out-of-band Region + small descriptor + notification), an open standard,
  and small in practice (Firecracker's block device: ~1400 LoC of Rust). The newer lesson:
  virtio assumed a trusted device side, and confidential computing inverted that — Intel/Red
  Hat are now stripping guest-side trust assumptions ("all virtio input received from the host
  must be considered untrusted", double-fetch/TOCTOU from re-read ring memory). Setonix states
  from day one what Linux retrofits: both endpoints treat the shared region as
  attacker-writable at every instant; validate a copied-out snapshot; never re-read after
  validation.
- **References into peer-mutable memory are UB — the volatile module is the obligation.**
  rust-vmm removed `as_slice` from guest memory because an ordinary Rust reference into memory
  another domain mutates is undefined behaviour; all access goes through volatile/copy
  primitives. And the wrapper itself had an out-of-bounds bug for years (RUSTSEC-2023-0056) —
  "written in Rust" does not discharge the boundary. Setonix states the mirror invariant beside
  RFC-0003's "never observed outside the kernel table": no kernel or service code forms a
  reference into a peer-writable region; access lives in one designated unsafe module with the
  soundness argument written, and that module gets the heaviest review and fuzzing in the tree
  (O-6's fine print).
- **O-18 is a mapping-discipline obligation, not a hardware checkbox.** Thunderclap (NDSS '19)
  defeated the IOMMU *usage* of three OSes without breaking the IOMMU: page-granularity windows
  over-share adjacent data, mappings outlive their I/O, shared translation domains, kernel
  pointers inside DMA-visible rings. VFIO adds that the isolatable unit is the IOMMU *group*
  (ACS topology), not the device. Discharge rules: one translation domain per driver;
  DMA-mapped pages contain only the transaction's data; DMA-window capabilities are
  transaction-lifetime (the generation mechanism fits naturally); device capabilities minted
  per IOMMU group, and where topology cannot split, the kernel refuses to pretend.
- **Anti-starvation policy lives in the userspace server, at the point of multiplexing.**
  Firecracker enforces per-device dual token buckets (ops + bandwidth) inside the VMM,
  explicitly because the guest cannot be trusted to self-limit — nothing added to the host
  kernel. The mapping is one-to-one: the driver/scheme server enforces per-client buckets
  (identified by badge — making RFC-0003a's badge mechanism a hard dependency of driver-side
  resource control), broker policy sets parameters, the kernel guarantees only its own
  non-exhaustion. Field detail worth stealing: validate bucket size against the largest single
  request at configuration time, or a client deadlocks forever (firecracker #259). This also
  exposes a threat-model gap: A5 claims cross-process starvation, but O-7 covers only the
  kernel's half — **proposed amendment:** every multiplexing server owes a bounded-service
  story per client, stated as an obligation on the driver model.

## Part 6 — Resource control and multi-tenancy (informs the scheduler and resource RFCs; O-7)

- **One accounting hierarchy, congruent with process structure, from birth.** cgroup v1's
  orthogonal per-controller hierarchies could not even agree which domain a dirtied page
  belonged to; v2's corrections (unified hierarchy, no-internal-process rule) took a decade of
  migration. Setonix makes the resource-accounting domain a single kernel object named by
  capability, to which scheduling contexts, memory grants and object quotas all attach; budget
  subdivision follows O-2's shape — top-down, subset-only, monotone.
- **Stall accounting is kernel mechanism; out-of-memory response is userspace policy.** Linux's
  in-kernel OOM killer lost to PSI + oomd at Meta's scale: the kernel gained a *measurement*
  (time stalled on CPU/memory/IO) and userspace owns the response. Build per-scheduling-context
  stall counters into the scheduler's first version, exported read-only via capability — this
  is also the O-17 driver supervisor's wedged-driver signal, and the observability answer that
  makes in-kernel extensibility unnecessary.
- **Budgets are minima, not caps.** The CFS quota saga — years of pathological throttling from
  distributing a global cap into per-CPU expiring slices, a kernel fix, a bolted-on burst
  feature, and operator consensus of "avoid limits" — teaches: scheduling contexts are
  primarily *guaranteed reservations* enforced under contention; if hard caps are offered,
  burst and redistribution semantics are specified in the RFC, never inherited implicitly;
  capping compressible resources is broker policy.
- **Design the budget-expiry-inside-server protocol now.** seL4 MCS makes time a capability and
  RFC-0004 already adopts donation — but MCS ships with a named hole: a client's budget
  expiring inside a passive server stalls the server holding state, "timeout exceptions" have
  stayed future-work across releases, and MCS verification lags baseline seL4. Adopt seL4
  RFC-14's prevention (minimum-budget threshold checked at `call` on the endpoint) plus a
  defined recovery (server abandons the request; the reply object delivers a visible failure).
  This is the temporal twin of the withheld-reply denial RFC-0004 §8 names, and belongs beside
  it — proposed as an RFC-0004 amendment.
- **Never co-schedule distrusting domains on SMT siblings.** L1TF/MDS made cross-hyperthread
  leakage practical; Linux spent three years retrofitting core scheduling onto a scheduler with
  no concept of security domain, and its docs concede partial coverage. Setonix has explicit
  domains: the scheduler RFC carries the placement invariant from its first draft
  (default-deny, domain-equivalence classes as broker policy, SMT-off supported). A sentence
  now, a rewrite later.
- **Keep the time-protection door open.** Ge et al. (EuroSys '19, in seL4): partition
  physically-indexed caches by page colouring, flush on-core state at domain switch, and — the
  non-obvious finding — partition the *kernel's own* text/data per domain. Three cheap
  structural obligations now, none building time protection yet: the frame allocator can
  allocate by colour (nearly free designed-in, a rewrite retrofitted); kernel internals accrete
  no ambient globals; the context-switch path names the single point where a domain-switch
  flush would insert.
- **Kernel-object exhaustion must be local and attributable.** The AWS Kinesis outage (17
  hours, cascading): one workload's growth hit a *global, unnamed* OS thread limit, failing
  everyone silently. Every kernel-object allocation is debited to a named per-domain quota;
  hitting a limit surfaces as an attributed error at the syscall boundary at allocation time.
  The resource RFC chooses between seL4-style user-supplied kernel memory (retype from
  Untyped — the proven strong form) and kernel-held per-domain quotas, and O-7's text then
  cites the chosen mechanism.
- **Overcommit's reclaim primitive is revocation-with-notice.** Kubernetes' eviction/OOM kill
  races and cgroup v1's self-defeating soft limits both show: for incompressible resources,
  best-effort targets and reactive killing fail; guaranteed floors plus an explicit reclaim
  protocol work (v2's `memory.min`/`low`). Setonix's frame grants are already floors — the
  kernel never overcommits a granted region. Density overcommit, where wanted, is broker
  policy whose reclaim primitive is capability revocation with notice: request release, then
  revoke; the victim observes a handleable fault. This makes RFC-0003a load-bearing for
  multi-tenancy, not only security, and adds a requirement to record there: memory revocation
  must be observable and survivable by the victim.
- **Refusing to share is a first-class mechanism.** Jailhouse partitions statically — exclusive
  CPUs, memory, devices; near-zero privileged code after setup — and gets its isolation from
  not multiplexing. The scheduler RFC supports exclusive core assignment as a cheap primitive
  (a core-set capability with no time-slicing, kernel never entered on the hot path),
  discharging A5 for the machine's main workload by construction and shrinking O-9's shared-core
  surface spatially. The partition-vs-multiplex dial is policy over mechanism, as the doctrine
  already mandates.
- **Reject: in-kernel verified extensibility (eBPF).** The verifier — tens of thousands of
  lines of hand-maintained abstract interpretation in ring 0 — has a continuing record of
  verifier-logic privilege escalations (CVE-2023-2163, CVE-2021-31440, CVE-2021-3490…), and
  the ecosystem's own remedy was retreat: unprivileged BPF disabled by default since 5.16. It
  is precisely a component the maintainer could never review line-by-line, and a second,
  informal admission boundary beside the capability system. The needs it serves are met by the
  architecture: shared-memory rings own fast-path filtering; stall/usage counters own
  observability. Setonix's extensibility is processes and capabilities; loadable kernel logic
  never enters.

## Part 7 — Memory and the MMU (informs the MMU RFC)

- **pKVM's page-ownership state machine is the Region-capability semantics.** Every physical
  page has exactly one owner; transitions are explicit — *donate* (ownership moves, page
  unmapped from donor atomically), *share/unshare* (owner keeps, borrower recorded, refcounted)
  — shipping on a billion devices. A borrow ledger enforced by hardware, in exactly the
  Rust-ownership vocabulary the constitution uses. The MMU RFC defines Region operations with
  this tri-state; RFC-0004's out-of-band shared-memory establishment becomes *share* in this
  protocol rather than an ad-hoc mechanism; transfer-of-region is *donate*, preserving O-8 by
  construction. (Secondary: pKVM guarantees memory integrity while leaving scheduling to the
  untrusted host — integrity and availability can be held by different layers.)
- **Guest-constructed structures validated in the kernel are the bug farm.** Nexen's sharpest
  finding: over half of Xen's core vulnerabilities sit in per-VM logic validating
  guest-supplied structures (page tables, emulated state). The MMU RFC never adopts "userspace
  builds a page-table-like structure, kernel validates it": the interface is mechanical
  operations on capability-named frames (map/unmap/protect) where O-5 validation is a rights
  check, not a structural walk. No instruction or device emulation in the kernel, ever.

## Synthesis — keep, prune, adopt

**Adopt now (design decisions this review settles or forces):**

1. **Store naming: Merkle root over fixed-size chunks, decided before the store is built**
   (composefs/dm-verity; ostree's retrofit price) — with closure identity as one signed
   compound root (ChainID/TUF), detached signatures (reproducible-builds), references graph as
   store metadata with GC roots (Nix), and A/B + monotonic counters separated
   operator-vs-network (AVB/ostree). Resolves RFC-0001 open questions 1–3.
2. **Scheme resolution semantics: Capsicum/`openat2` as specification** — resolve once to a
   held node capability, bounded to the granted view during resolution, no alias nodes, server
   dependencies disjoint from served subtrees, subtree capabilities expressed in RFC-0003's
   existing calculus. O-11 discharged by construction, within O-2.
3. **Broker design rules + three proposed obligations** — no re-origination, no standing
   downstream credential, no escalate verb, audience on externalised grants, fail-closed
   mediation with durable grants, acyclic bootstrap, single-use detectable first-grant
   delivery; **O-21** mediation auditability (dual-attributed), **O-22** broker compromise
   containment (proven bound), **O-23** broker availability semantics.
4. **virtio as first driver ABI, both sides adversarial; volatile-access module as O-6's named
   home; O-18 as mapping discipline** (per-driver domains, transaction-lifetime DMA windows,
   IOMMU-group-granular minting).
5. **Resource accounting from birth**: one capability-named accounting domain; stall counters
   in the first scheduler; budgets as minima; per-domain object quotas with attributed errors;
   SMT placement invariant; colour-capable frame allocator; MCS budget-threshold + expiry
   recovery (proposed RFC-0004 amendment).
6. **Spawn: capability inheritance explicit-only, default-empty** (Leaky Vessels; Landlock) —
   an obligation for the future spawn/process RFC.

**Prune (named rejects, with the graves they walk into):**

1. **In-kernel verified extensibility (eBPF)** — an unreviewable trusted compiler in ring 0;
   a second admission boundary beside the capability system.
2. **Caches keyed on less than the full input hash** (Docker build cache) — a poisoning
   surface; imported cache is untrusted input.
3. **A bootstrap superuser** (`system:masters`) — a standing identity outside the capability
   system; the bootstrap seam is a one-time kernel act instead.

**Proposed constitution graves (§3 — the maintainer's to take; recommended wording ready on
request):** drivers pulled into the kernel for the fast path (vhost-net); compiled-in but
unused device paths (VENOM); the catch-all right (CAP_SYS_ADMIN — as a review rule on Rights
growth).

**Proposed threat-model amendments:** O-21/O-22/O-23 (broker); TUF's fast-forward,
mix-and-match, bounded-download and name-binding obligations; A5's userspace half (multiplexing
servers owe bounded service per client); spawn inheritance obligation — each per §11's "new
surface, new obligation" rule.

**The meta-observation.** Research/0001 found the microkernel lineage validating Setonix's
mechanisms; this review finds the *production* world validating its economics. Every survivor
of the container-security era — gVisor, Kata, Firecracker, WASI, pKVM — is a partial rebuild of
capability discipline at some boundary, paid for at retrofit prices: a second kernel in Go, a
VM per pod, a decade of CVEs arriving at handle-not-path. Setonix's bet is that designing the
discipline in from syscall one costs less than any of the retrofits. The fields' scar tissue
says the bet is priced correctly — provided the TCB stays small enough to keep the software
boundary believable, which makes the constitution's understandability rule a security control,
not a style preference.

## Sources

Primary sources are cited inline throughout: peer-reviewed papers (NSDI, SOSP, NDSS, EuroSys,
USENIX Security, ASPLOS, ECRTS), official design documents and kernel documentation, CVE
analyses and vendor postmortems, and maintainers' own writing. Where a number is quoted (LoC,
CVE counts, adoption percentages, overhead multiples), it comes from the cited primary source,
not from summaries.
