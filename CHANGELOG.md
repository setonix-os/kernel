# Changelog

All notable changes to the Setonix kernel are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Release codenames follow the six Noongar seasons — Birak, Bunuru, Djeran, Makuru, Djilba, Kambarang.

## [Unreleased]

### Added

- **How the work is done is now written down where a session reads it.** Seven skills: `/checkpoint`,
  `/pre-push`, `/review`, `/model-routing`, `/rfc-draft`, `/fact-check` and `/steward`. `TODO.md` holds
  where the work stands and what is left, replacing separate handoff files. `CLAUDE.md` gains
  § Never do these, including a rule against leaving anything behind.
- **One command runs everything CI's checks job runs.** `.github/scripts/check-all.sh` holds the job's
  steps, in its order, and both workflows now call it instead of listing the steps themselves — so
  the command a contributor runs before pushing cannot drift from CI. It stops at the first failing
  step and names it. Among those steps is a new gate, `.github/scripts/check-code-spans.sh`, which
  fails on an inline code span broken across a line end (matched as CommonMark matches spans, so
  double-backtick spans and stray backticks are handled) and on typographic quotes in our own
  Markdown. Its first run found one broken span, in this changelog.
- **The capability table itself — RFC-0003's second increment.** `setonix-capability` grows the owned
  `Capability` value and the flat per-process `CapabilityTable` (§4, Option B): the structure every
  future syscall resolves against.
    - `Capability<O>` is **owned and deliberately neither `Clone` nor `Copy`** (§6): transfer between
      tables is a Rust move, and the sole duplication is `derive` — explicit, rights-checked,
      subset-only (O-2). A `compile_fail` doctest (pinned to E0277, instantiated with a real
      `ObjectRef` type) guards the absence of `Clone` on every test run, rather than asserting it in
      prose; the type is also `#[must_use]`, since dropping a capability is a close and must never
      happen by accident. Minting reads the generation from the object itself, so a capability can
      never be back-dated to a generation its object no longer occupies.
    - `CapabilityTable<O, N>` — a fixed-capacity flat array (the kernel has no allocator; bounded by
      construction), free-list recycling, O(1) everything. Resolution is one index plus two generation
      checks: slot against handle (the ABA defence, O-1) and object against capability (destruction
      revocation, O-3). A slot's generation is bumped **when it is vacated**, so a closed handle dies
      at the instant of closing, not merely when its slot is reused; a slot whose generation cannot
      advance is **retired** outright — capacity is the price of failing closed. All three of the RFC
      amendment's load-bearing invariants are now *stated where they bind*: a resolve yields a borrow,
      never a copy, and the caller holds that borrow across its whole check→act window — the property
      any future multi-core synchronisation story must preserve, documented on `resolve` and in the
      `ObjectRef` contract rather than assumed.
    - The trait bounds carry the design: `insert`/`remove` are object-blind slot mechanics (the
      plumbing a transfer is built from), while `resolve`/`derive` require the new `ObjectRef` trait —
      one method, *what generation is the object at now* — which is all the table ever asks of a
      kernel object. The kernel crate implements it when kernel objects exist; a test double implements
      it today, which is what keeps the whole scheme host-testable.
    - A full table hands the capability **back** in the error rather than dropping it: destroying
      in-flight authority because a receiver had no room would turn a resource limit into silent
      revocation.
    - Twenty-seven new host tests, hardened by an adversarial multi-lens review with mutation testing
      (every surviving mutant found got a test that kills it). Among them: an exhaustive forged-handle
      sweep (only the exact minted handle resolves — for both `resolve` *and* `remove`, so a dangling
      handle can neither use nor steal a reused slot's occupant); object destruction making parent and
      derived child inert with no list of holders, while a *removed* parent leaves its derived sibling
      untouched (the flat table has no parent link — pinned so RFC-0003a cannot regress it silently);
      generation-exhaustion retirement alone and amid live neighbours; free-list LIFO order;
      transfer-as-move between two tables; the two defensive fail-closed branches driven by
      deliberately corrupted private state; and an 8192-operation churn test against a shadow model —
      inserts, subset-random derivations, object destructions, object-blind cleanup and removals
      interleaved — in which no dead handle ever resolves or removes anything. The lifecycle is also a
      running doctest.
- **The capability table begins — RFC-0003 turns into code, first increment.** A new `no_std`
  workspace crate, `setonix-capability`, holding the pure, architecture-independent logic of the
  capability spine so it can be **host-unit-tested** — a bare-metal target has no test harness, so the
  security-critical logic lives where it can be exercised exhaustively. This increment is the value
  types the table is built from, each with a compiler-checked invariant:
    - `Rights` — a closed bitmask (`DUPLICATE`, `TRANSFER`, `READ`, `WRITE`, `REVOKE`) whose only
      attenuation operation, `diminish`, is subset-only. Non-widenability (O-2) is a property of the
      type: there is no constructor from arbitrary bits and no operation that adds a right. A test
      checks *exhaustively* over all 32 rights combinations that a successful `diminish` is always a
      subset of its source.
    - `Generation` — a 64-bit counter, **fail-closed on exhaustion** (`next` returns `None` rather than
      wrapping to a value a stale handle could match), the defence behind O-1's reuse case and O-3's
      destruction case.
    - `Handle` — the userspace-facing name: a table index plus the generation its slot held when
      minted; carries no authority by itself.
    - `CapabilityError` — a fail-closed error for every table operation.
  Twelve host tests, clippy-clean under `-D warnings`, and built for both Tier-1 targets to prove it
  stays `no_std`. CI gains a capability host-test group and per-target build. The owned no-`Clone`
  `Capability` value and the flat table follow in the next increment (§5.3: small, reviewable steps).
- **Exception vectors, and a reporter that says what went wrong in one line** — the instrument the
  soft-float bug had to be diagnosed without. `boot.s` installs a 2 KiB-aligned sixteen-entry vector
  table into `VBAR_EL1` before the first Rust instruction runs, so even the earliest fault is reported
  rather than vectoring to address zero and dying mute. Every entry routes to a reporter that prints
  which vector fired, the exception class decoded into a sentence — EC 0x07 names `CPACR_EL1.FPEN`
  outright — and raw `ESR`/`ELR`/`FAR`/`SPSR`, then halts.
    - The common stub re-asserts `SPSel` before touching memory. Exception entry already sets
      `PSTATE.SP` to 1, so the handler always starts on SP_ELx whichever vector group it arrived
      through — the `msr` is defence in depth, not the thing that keeps the never-used SP_EL0 group
      off the uninitialised SP_EL0.
    - **Self-testing in CI**: a `provoke-exception` feature (never on by default) boots, greets, then
      executes `brk #0`; the boot job greps the console for the decoded "BRK instruction" report. The
      handler is proven on every pull request, not trusted.
    - `cargo xtask` grows `--features` passthrough for exactly that.
    - Terminal by design: until there is a scheduler, every exception is a report and a halt —
      recovery is policy, and there is nothing yet to recover to. When the timer interrupt arrives,
      the IRQ entries grow a real save/restore frame; the clobber-freely discipline in `vectors.s` is
      documented as ending on that day.
    - The EL1 assumption in `boot.s` is now load-bearing (`VBAR_EL1`, `SPSel`) and its header says so:
      an explicit CurrentEL check and EL2 descent are owed before real hardware or
      `virtualization=on`.
- **The kernel boots.** On QEMU aarch64 `virt` it reaches the PL011 UART and
  prints `Kaya!` and the resident critter (constitution §8). Nothing else: no
  scheduler, no IPC, no capability table, no MMU. What it establishes is the chain
  underneath all of those — linker script, boot stub, stack, `.bss` zeroing, the
  hardware-abstraction boundary and the console device.
    - `kernel/src/arch/mod.rs` — the hardware-abstraction boundary, deliberately
      two functions wide. It grows one function at a time, as callers above
      genuinely need them, because a HAL designed ahead of its callers ends up
      shaped like whichever architecture was written first.
    - `kernel/src/arch/aarch64/` — boot stub, PL011 console, core control. The
      boot stub parks every core but the first, installs the stack, zeroes `.bss`,
      and enters Rust.
    - `kernel/src/arch/x86_64/` — compiles and links against the same
      architecture-independent kernel, keeping the boundary honest, but does not
      boot: `q35` has no bare-ELF equivalent of `-kernel`.
    - `kernel/link/aarch64.ld` — load address 0x4000_0000, `.text.boot` first and
      `KEEP`-ed, 8-byte-aligned `.bss` bounds to match the zeroing loop's stride,
      64 KiB boot stack.
    - Panic handler that reports location and message on the console before
      halting the core, written as separate lines so that a fault partway through
      still leaves evidence on the wire.
- `xtask` — build, run and boot-test automation, with **no dependencies at all**.
  A tool that builds a system whose fourth pillar is exact, auditable dependency
  closures should not itself pull in eighty crates.
    - `cargo xtask boot-test` boots under QEMU, watches the serial console for an
      expected string against a deadline, and reaps the emulator afterwards — the
      kernel halts rather than exits, so an unreaped QEMU would wedge a CI runner.
    - Failure messages are advice rather than errno: a missing QEMU says how to
      install it, and `--arch x86_64` explains why that path cannot boot yet
      instead of failing obscurely.
- Repository skeleton and pinned development environment.
    - `.devcontainer/` with QEMU for both Tier-1 architectures, `gdb-multiarch`, OVMF and AAVMF UEFI
      firmware, `mtools`, `dosfstools`, and a pinned Node plus `markdownlint-cli2`. The linter is there
      because `CONTRIBUTING.md` asks contributors to run it: since CI runs this image, a tool missing
      from it is an instruction that cannot be followed inside the canonical environment.
    - `rust-toolchain.toml` pinning Rust and both bare-metal targets.
    - Workspace `Cargo.toml` carrying the lint policy, so the first crate to land is already bound by
      it rather than retro-fitted.
- Lint policy enforcing the constitution mechanically.
    - `unsafe_code = "deny"` at the workspace level, so a designated module must opt in visibly with
      `#![allow(unsafe_code)]` and the full set of such modules is greppable in one command.
    - `undocumented_unsafe_blocks` and `missing_safety_doc` denied, making the `// SAFETY:` convention
      a compiler-checked requirement rather than a review habit.
- CI for pull requests and `main`, running **inside the devcontainer image**.
    - Every job executes in the image built from `.devcontainer/Dockerfile` — the same one
      contributors get from "Reopen in Container". Nothing installs a toolchain of its own, so no tool
      version has a second source of truth and there is no "works on my machine" gap to argue about.
      This is constitution §7 taken literally.
    - The image is built from the Dockerfile *as it appears in the pull request*, so a change to the
      environment is tested by the run that proposes it. Layers come from the image `main` publishes,
      so an unchanged Dockerfile costs a pull rather than a build.
    - `main` publishes the image only after it has both built the kernel **and** booted it, so a pull
      request never caches from an image that was not itself proven.
    - Build and Clippy across both Tier-1 bare-metal targets on every pull request.
    - `boot` job runs the kernel under QEMU and requires the console greeting; `main` additionally
      boots the release image, which differs enough in layout and inlining that debug-only boot tests
      have let real faults through in other kernels.
    - Every action pinned to a commit SHA with a version comment.
- `.github/scripts/check-toolchain-pin.sh`, verifying that the Rust version named in
  `rust-toolchain.toml` and in `.devcontainer/Dockerfile` agree, and that both Tier-1 targets are still
  present. Its third check is an assertion of *absence*: no workflow may use `dtolnay/rust-toolchain`,
  `actions/setup-node` or similar, nor install a toolchain by hand. Since CI runs the devcontainer
  image, reintroducing any of those would silently recreate the second source of truth §7 exists to
  remove — silently, because everything would still build.
- `.github/scripts/check-british-spelling.sh`, enforcing constitution §11.6 in CI, with `vendor/`
  excluded so that vendored MIT code is never reworded.
- Custom Claude Code commands: `/audit-kernel`, `/check-style`, `/check-spelling` and
  `/check-primitive` — the last auditing a change against the primitive, the Borrow Ledger and the
  known graves.
- Contributor documentation: `CONTRIBUTING.md`, `STYLE.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, issue
  and pull-request templates.

### Fixed

- **Contributor documents reconciled with the tree.** `README.md`'s Status no longer denies the
  capability table; `CONTRIBUTING.md` drops the `tests/` row for a directory that does not exist, no
  longer presumes a UEFI boot path, says "signed" rather than "GPG-signed", and lists the threat model
  and research notes; `SECURITY.md` points at the threat model instead of restating its seed.
- **CI now enforces what it claimed to.** `RUSTFLAGS="-Dwarnings"` never reached cargo:
  `devcontainers/ci` forwards into the container only the variables its `env` input names, plus the
  `GITHUB_*` file paths, so every build, test and boot accepted rustc warnings — run 42213389069's log
  shows `devcontainer exec` handed those four paths and nothing more. Every `devcontainers/ci` step
  now names `RUSTFLAGS` and `CARGO_TERM_COLOR`, and the tree builds clean with them; each `runCmd`
  fails at once if `RUSTFLAGS` arrives unset, since the action drops a forwarded name silently, and
  `check-all.sh` supplies `-Dwarnings` when run locally without it. Every cargo build, check and
  test runs `--locked` (`cargo fmt` takes no such flag), as does xtask's own build of the
  kernel, so a stale committed lockfile fails CI rather than being quietly rewritten. The main
  workflow is read-only by default and only its publishing `boot` job holds `packages: write`. Pull
  requests now boot the release build as main does, and run for every base branch, so a stacked pull
  request gets CI too; no pull-request job can publish the image. Checkouts no longer persist the
  job's token for later steps.
- **The British-spelling gate had been checking nothing in CI.** Inside the devcontainer the
  checkout belongs to the runner's user, not root, so git refused it as "dubious ownership"; the
  script read its file list through a process substitution, whose failure `set -e` cannot see, and
  reported "OK: no files to check". The spelling gate and the new code-span gate now trust exactly
  the repository root for that one call, fail when git does, and refuse an empty file list. The pin
  check now also scans `.github/scripts/`, since CI's commands live there, and `check-all.sh` runs
  each step under its own `set -e`, so a step that is a shell function cannot pass a failure.
- **The x86_64 docs no longer say `q35` has no bare-ELF `-kernel`.** `CLAUDE.md`, the README, the
  x86_64 module, `xtask`'s refusal and a CI comment all claimed it. QEMU's x86 loader does take a
  64-bit ELF through `-kernel` when it carries a Xen PVH note (`XEN_ELFNOTE_PHYS32_ENTRY`,
  `hw/i386/x86-common.c`), and refuses this image only because it has none — reproduced on the
  pinned QEMU 11.0.3, which reports "Error loading uncompressed kernel without PVH ELF Note". The
  corrected text states the fact and leaves the choice between PVH and a UEFI stub to the x86_64
  boot RFC.
- **xtask validates its workspace root at the source.** CodeQL's one open alert flagged the image
  path as depending on a user-provided value (`CARGO_MANIFEST_DIR`). There is no privilege boundary
  in xtask for a hostile value to cross — cargo sets the variable, and the tool runs with the
  invoker's own authority — but the honest response was to make the validation real rather than
  argue: the root is now canonicalised and checked to hold the workspace manifest, so a mangled
  environment is reported at the source instead of surfacing downstream, and the image path's
  containment under the root is asserted rather than assumed. One new test pins the behaviour.
- **A repository-wide bug hunt, and what it found.** Eight review passes over the source, the
  tooling, the configuration and the documents, each finding independently verified before anything
  was changed. Nothing found was a kernel logic defect — the capability crate and the boot path both
  survived the sweep — but a great deal of the tree described itself inaccurately, which for a
  project whose thesis is that documents and code must not diverge is the defect that matters.
    - **`xtask` fails closed on malformed input and on QEMU's own failures.** `boot-test` piped
      QEMU's stderr into a pipe nobody drained: every emulator-side diagnostic was discarded, and
      enough of them would have blocked QEMU on a full buffer and surfaced as a bogus timeout blamed
      on the kernel. stderr is now inherited, stdin is nulled so the stdio chardev can no longer
      leave an interactive terminal in raw mode after the reap, and a QEMU that dies before the
      first console line now says so instead of asserting a timeout that never elapsed. Argument
      parsing no longer lets a flag swallow the next flag as its value, silently default when a
      value is forgotten, or ignore an unrecognised token — `--arch=x86_64` used to build aarch64
      without a word. Seven unit tests now cover the pure helpers, so CI's
      `cargo test --package xtask` verifies something for the first time.
    - **The forged-handle sweep now covers `remove` as well as `resolve`**, which is what this
      changelog already claimed of it. Every forged (index, generation) pair in the space is denied
      at both entry points, so a dangling handle can neither use nor steal a reused slot's occupant —
      proven rather than asserted.
    - **Comments that misdescribed the machine.** Two credited "QEMU's firmware" with configuring the
      PL011, when no firmware runs in the `-kernel` boot path at all — the console works because
      QEMU's device model transmits without initialisation, which is the load-bearing fact. The
      `SPSel` write in `vectors.s` was described as what keeps an SP_EL0-group exception off the
      uninitialised SP_EL0; per the architecture, exception entry already sets `PSTATE.SP`, the
      vector group records the *interrupted* context's stack, and the instruction is defence in
      depth. The x86_64 placeholder claimed AArch64 needs no stack before its first print,
      contradicting `boot.s`, which installs one before anything else.
    - **Configuration that could not do what it said.** The aarch64 debug configuration pointed at
      the pre-soft-float target directory, silently attaching to stale symbols. `rust-toolchain.toml`
      instructed bumping a `dtolnay/rust-toolchain` pin that no longer exists and that
      `check-toolchain-pin.sh` fails the build over — following the comment would have broken CI.
      That same script's hand-install detector was anchored to line starts and so could never match
      the idiom it exists to catch. `.gitignore` ignored a GDB history file GDB never writes;
      `.gitattributes`' `*.S` rule never matched this tree's lowercase `.s` assembly on the
      case-sensitive filesystems it exists for; the devcontainer's history volume had no `HISTFILE`
      pointing into it. The cache-cleanup workflow and script are deleted outright: no workflow here
      uses `actions/cache`, and the script described another repository's cache groups — absent beats
      disabled, which is now one of the constitution's own graves.
    - **Documents that had drifted from the tree.** The clippy command in `CONTRIBUTING.md`, the pull-
      request template and `STYLE.md` was unscoped, so it lint the kernel for the host triple and
      never for either Tier-1 target — not what CI runs, and contradicting the same file's own rule
      about always naming the package. The RFC-writing guidance and `/check-primitive` still listed
      four graves after §3 grew to seven, so an audit run from either silently skipped three. The
      toolchain was described as pinned in three places in four separate files. `README.md` still
      advertised zero-copy page transfer as the IPC design for bulk data — the mechanism RFC-0004
      pruned as L4's abandoned long IPC. The feature-request template called Setonix "a
      credential-handling project", boilerplate from another repository. Twenty-nine comments across
      fifteen files cited constitution sections as "CLAUDE.md §N", stale since the rename.
    - `build.rs` and the two CI scripts gained the SPDX headers every other file in the tree carries.

### Changed

- **CI uses only GitHub's actions and our own.** `devcontainers/ci` and `docker/login-action` leave
  both workflows ahead of the repository's Actions policy being restricted to actions owned by GitHub
  and by `setonix-os`. They are replaced by the plain `docker` commands the action ran for this
  repository, as read from its source at the pinned SHA and from run 42219570760's log, held once in
  a new `.github/scripts/devcontainer.sh` that both workflows call and that documents every flag.
    - `devcontainer.sh build` is `docker buildx build --load` with `BUILDKIT_INLINE_CACHE=1` and
      main's image as `--cache-from`, so the image still carries the cache metadata the next run
      reads; it first prints the docker and buildx versions and the builder, so a change in GitHub's
      runner image shows in the log. This pull request's own run pulled that cache anonymously and
      rebuilt nothing.
    - `devcontainer.sh build --publish`, used only by main's boot job, builds with `--no-cache`
      instead. Any branch in this repository can push to the registry by editing a workflow, so the
      image main publishes must not inherit a layer from it. The price is a full build, QEMU's
      compile included, on every push to main: the last one took the whole boot job eight minutes,
      well inside its thirty.
    - `devcontainer.sh run` is `docker run --pull never`, so it can only use the image just built,
      with the checkout bind-mounted at `/root/kernel`, the folder `devcontainer.json` names, as the
      image's own root, and with `RUSTFLAGS` and `CARGO_TERM_COLOR` passed by `-e` as the action's
      `env` input passed them. Ownership is unchanged: the checkout still belongs to the runner's
      uid, because the action's UID-matching pass was a no-op for this image.
    - Main's boot job alone publishes, only in `setonix-os/kernel`, and only after every boot has
      passed: `docker login` from standard input, then `docker push` of a tag naming the commit
      followed by `latest`, each tried three times, then a `docker logout` that runs whatever
      happened. Login, push and logout stay in the workflow, so the token never reaches the script.
      The login now comes just before the push rather than before the build, because the cache pull
      needs no credentials; the package must therefore stay public.
    - Four things the action did are deliberately not reproduced, because CI used none of them:
      `devcontainer.json`'s mounts and `containerEnv`, which serve interactive sessions; its
      `postCreateCommand` and the CLI's login-shell environment probe, since the probe yields the
      image's own `PATH` and `check-all.sh` already prints the versions; the `GITHUB_*` file-command
      mounts, which nothing in the container writes to; and the `devcontainer.metadata` label, which
      nothing reads, because "Reopen in Container" builds the Dockerfile.
    - `check-toolchain-pin.sh` now reads every `uses:` in the workflows and in any composite action
      under `.github/actions/`. It fails an action owned by anyone but GitHub or `setonix-os`, one
      not pinned to a full commit SHA, and the toolchain installers it already banned, now however
      their value is quoted. Because a grep cannot parse YAML, it accepts `uses:` in one plain form
      and fails every other spelling of the key, such as a flow mapping, a quoted, escaped or
      aliased key or a space before the colon, each of which a narrower pattern would have waved
      through; a local action path containing `..` fails too. `CLAUDE.md`
      § Toolchain states the owner rule and defers to `STYLE.md` § Action Pinning for the pin.
    - Neither workflow grants `packages: read` any longer, since nothing reads the registry with
      the token; main's boot job keeps `packages: write`. Every `run:` step runs under bash with
      `pipefail`, and `CI Success` reads the job results from `env` rather than interpolating them
      into its script.
- **The five Claude Code commands are now skills.** `/audit-kernel`, `/check-coherence`,
  `/check-primitive`, `/check-spelling` and `/check-style` move from `.claude/commands/` to
  `.claude/skills/<name>/SKILL.md`, by `git mv` so their history follows them. The slash names still
  invoke them, so the references in `CONTRIBUTING.md` and the pull-request template stand. Each
  gains a description saying when to use it, which also lets a session reach for it unprompted, and
  the two with real lessons behind them gain a dated **Lessons** section. `CLAUDE.md` § Claude Code
  skills states the rule that keeps them current: a lesson lands in its skill in the same pull
  request as the fix that taught it.
- Contributor documents no longer assume a permanent team of one. `SECURITY.md`, `CONTRIBUTING.md` and
  `CLAUDE.md` now say "deliberately small team" — the project anticipates at least one more member, and
  a document that promises "one maintainer" would start lying the day they join. The constitution's §5
  is deliberately untouched: its wording is role-based ("the maintainer") and still true, and the real
  §5 amendment — who reviews, who may merge — depends on the member's actual role, which is the
  maintainer's to write when that person exists.
- **The docs repository was merged into this repository, history and all.** The constitution now lives
  at `CONSTITUTION.md` in the repository root, bound into every session by the repo-root `CLAUDE.md`;
  RFCs and the documents' changelog live under `docs/`. The import was a true history merge rather than
  a file copy, so every signed commit of the paper trail is ancestry of `main` and remains publicly
  reachable after the old repository goes private. Rationale and consequences are recorded where they
  belong: `docs/CHANGELOG.md` and RFC-0002's second amendment.
- **Both Tier-1 targets are now soft-float**, `aarch64-unknown-none` becoming
  `aarch64-unknown-none-softfloat`. Found by booting: the kernel reached
  `console::write_str` and then took a synchronous exception to `0x200` — the
  `VBAR_EL1 = 0` vector — because LLVM had emitted an FP/SIMD instruction for
  ordinary data movement inside `read_volatile`'s debug precondition check, and
  `CPACR_EL1.FPEN` is 0 at reset. No console output at all, since the fault landed
  one instruction before the first character.
    - The plain `aarch64-unknown-none` target permits NEON; its `-softfloat`
      variant does not. `x86_64-unknown-none` was already soft-float, so the two
      Tier-1 architectures had been quietly asymmetric — precisely the class of
      difference building both from day one is meant to expose.
    - Right on the merits regardless of the bug: a microkernel whose IPC fast path
      is the product must not touch registers it would then have to save and
      restore on every context switch. FP/SIMD belongs to userspace, enabled per
      process and saved lazily, once there is a userspace to enable it for.
    - Nothing in the kernel source changed. Only the target triple did.
- **The devcontainer no longer builds itself out of distribution packages.** Debian 13 ships QEMU
  10.0.11; upstream is at 11.0.3. A whole major version of the emulator this kernel is tested on,
  withheld by a packaging decision nobody on this project made — pillar 3's gatekeeper problem turning
  up inside our own build, before the kernel had finished printing its first line. Rust, Node and QEMU
  now come from their authors at pinned versions, each verified before use:
    - QEMU is built from the upstream source tarball, checked against a SHA-256 pinned in the
      Dockerfile *and* against the project's own detached GPG signature. Author signing is pillar 3 as
      QEMU already practises it, so it costs nothing to honour. Only the two Tier-1 softmmu targets are
      built.
    - Rust comes from `rustup-init` in `static.rust-lang.org`'s versioned archive, verified against the
      checksum published beside it, installed into `/opt/rust` so the toolchain belongs to the image
      rather than to a user.
    - Node comes from `nodejs.org`, verified against the release's own `SHASUMS256.txt`.
    - Multi-stage, so the download and build tooling appears in no final layer, and the base image is
      pinned by digest rather than by the moving `debian:13-slim` tag.
    - apt remains for shared libraries and for tools that publish no upstream binary at all — GDB, the
      UEFI firmware images, `mtools`, `dosfstools`. Each is labelled in the Dockerfile as a gatekeeper
      not yet removed, with the reason it has not been.
    - The image ends with a verification block that invokes every tool, so a missing shared library or
      a botched `COPY` fails the build rather than someone's afternoon.
- Base image moved from Debian 12 (bookworm) to **Debian 13 (trixie)**, which also brings gdb 16.3 in
  place of 13.x.
- The container now runs as root with the workspace at `/root/kernel`, matching the maintainer's other
  embedded projects. The toolchain in `/opt` belongs to the image, which keeps `CARGO_HOME` writable
  without chown games when CI runs the same image.
- Rust pin moved 1.88.0 → 1.95.0 → **1.97.1**, the current stable. Recorded because a toolchain bump is
  a deliberate act under constitution §7, not a detail. Verified: `fmt` and `clippy` clean on both
  Tier-1 targets and on `xtask` under `-D warnings` with the new compiler, no source changes needed.
- Every GitHub Action pin refreshed to its latest release: `actions/checkout` v6.0.3 → v7.0.1,
  `actions/setup-node` → v7.0.0, `actions/cache` → v6.1.0, all still full-SHA pinned.
- CI no longer guards its compile steps on the workspace having members, and the boot job no longer
  guards on `xtask` existing — both now do.
- `clippy::redundant_pub_crate` and, in one documented instance,
  `clippy::missing_const_for_fn` are allowed. The first suggests plain `pub` for
  every `pub(crate)` item in a private module, which in a binary crate is not
  merely noisy but wrong. The second would have made the console function `const`
  on x86_64 only, where it is a no-op — a divergence in the hardware-abstraction
  boundary's signature, which is precisely what the boundary exists to prevent.
- `clippy.toml` adds architecture, firmware and peripheral names to
  `doc-valid-idents`, so that `doc_markdown` stops demanding backticks around
  proper nouns like AArch64 and OVMF.

[Unreleased]: https://github.com/setonix-os/kernel/commits/main
