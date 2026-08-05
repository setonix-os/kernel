Audit a proposed or recent change against the project's constitution before it is allowed to land.

This command has no counterpart in the maintainer's other projects — it exists because Setonix has a
written constitution that most projects do not, and because CONSTITUTION.md §1 and §5.4 are the two rules
most easily lost during a productive afternoon.

If `$ARGUMENTS` names files or a commit range, audit those. Otherwise audit everything changed since
the last commit.

## 1. The primitive

> An application is a self-contained, signed, identity-bearing, immutable, content-addressed blob —
> and every resource it touches is a scheme mediated by a broker.

For each substantive addition, state which pillar it is a consequence of, or say plainly that it is
toil (tooling, tests, docs, build). **If it is neither, that is the finding.** CONSTITUTION.md §1 is
explicit that an addition which cannot justify itself does not go in, however attractive it is.
Name the darling and recommend killing it.

## 2. Mechanism, not policy

CONSTITUTION.md §3: the kernel provides mechanism and never policy. Look for policy that has crept below
the syscall boundary — a default, a heuristic, a scheduling decision or a permission rule that
userspace should own. Early microkernels are listed among the known graves for exactly this.

## 3. The Borrow Ledger

CONSTITUTION.md §4 and §11.2. For each subsystem touched:

- What is the ledger's verdict — write ourselves, or port?
- Does the change respect it? Hand-written code appearing in a "port" row, or ported code in a
  "write ourselves" row, is a finding either way.
- If the verdict is unclear or looks wrong for what the code now needs, **say so and stop**. §11.2
  requires asking the maintainer, not deciding.

## 4. Understanding

CONSTITUTION.md §5.2 and §11.4: nothing merges un-understood. Judge whether the change ships an
explanation the maintainer could verify their understanding against. A subtle change with no
rationale is a finding even when the code is correct — especially then.

## 5. Known graves

Check the change does not walk into one of the graves §3 names:

- Multi-copy IPC (Mach)
- Policy in the kernel (early microkernels)
- Baroque capability hierarchies (KeyKOS)
- Bolted-on multicore support
- Drivers pulled into the kernel for the fast path (vhost-net)
- Compiled-in but unused device paths (VENOM)
- The catch-all right that decays into root (CAP_SYS_ADMIN)

## 6. Licence hygiene

CONSTITUTION.md §11.5: new files carry a GPLv3 SPDX header; vendored MIT code keeps its notices verbatim.

## 7. Scope

CONSTITUTION.md §6. Flag anything that quietly serves a non-goal — desktop polish, bug-for-bug POSIX
compatibility, a third architecture, Apple Silicon, GPU acceleration beyond the UEFI framebuffer.

## Output

One section per numbered check above, each with a verdict of **pass**, **finding**, or **ask the
maintainer**. Be concrete: quote the code or the clause. End with a single recommendation — land,
land with changes, or do not land — and the one sentence of reasoning behind it.

Where a conflict is between the change and the constitution itself, raise the conflict rather than
resolving it in either direction (CONSTITUTION.md §11.1).
