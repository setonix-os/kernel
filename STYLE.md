# Style Guide

Code style conventions for the Setonix kernel.

---

## General Rules

| Rule | Setting |
|------|--------|
| Indentation | 4 spaces (no tabs) |
| Max line length | 100 characters for Rust, 170 for everything else |
| Charset | UTF-8 |
| Final newline | Always |
| Trailing whitespace | Trim (except Markdown) |
| Line endings | LF, in the repository and the working tree |

The general rules are enforced by `.editorconfig`; Rust's tighter limit by `rustfmt.toml`; line
endings by `.gitattributes`. Install the EditorConfig plugin for your editor:

- **VS Code:** [EditorConfig for VS Code](https://marketplace.visualstudio.com/items?itemName=EditorConfig.EditorConfig)

Why Rust is narrower than the rest: kernel code is read side by side with a datasheet or a
disassembly far more often than it is read alone, and 100 columns survives a split editor.

---

## Single Source of Truth

Avoid duplicating information across files. Each piece of information should have one canonical
location.

| Information | Canonical Source |
|-------------|------------------|
| The constitution, pillars, Borrow Ledger | `CONSTITUTION.md` |
| Design decisions and their rationale | `docs/rfcs/` |
| Repo-local rules, `unsafe` policy, HAL boundary | `CLAUDE.md` |
| Build commands | `CONTRIBUTING.md` § Development Setup |
| Coding standards | `CONTRIBUTING.md` § Coding Standards |
| Commit conventions | `CONTRIBUTING.md` § Commit Messages |
| British spelling word list | `.github/scripts/check-british-spelling.sh` |
| Rust version | `rust-toolchain.toml` |
| Lint policy | `Cargo.toml` § `[workspace.lints]` |
| Formatting rules | `.editorconfig`, `rustfmt.toml` |
| Security policy | `SECURITY.md` |

**Guidelines:**

- Reference the canonical source instead of duplicating content
- If information must appear in multiple places (e.g. PR template checklists), keep it minimal
- When updating information, update the canonical source first
- Cross-reference using `filename` § Section Name format

This project takes the rule more seriously than most, because two of its own pillars are about
avoiding divergent copies of things. A style guide that contradicts itself is a small instance of the
problem the whole system exists to solve.

---

## Rust

### Formatting

`rustfmt`, configured by `rustfmt.toml`. CI enforces it.

```bash
cargo fmt --all         # Format all code
cargo fmt --all --check # Check without modifying
```

### Linting

`clippy` with warnings as errors, configured by `[workspace.lints]` in `Cargo.toml`. CI enforces it
package by package — see `CONTRIBUTING.md` § Development Setup for the exact scoped invocations.

### Naming Conventions

| Item | Convention | Example |
|------|------------|--------|
| Crates | snake_case | `setonix_kernel` |
| Modules | snake_case | `capability_table` |
| Types | PascalCase | `PhysAddr` |
| Traits | PascalCase | `Hal` |
| Functions | snake_case | `map_page` |
| Constants | SCREAMING_SNAKE_CASE | `PAGE_SIZE` |
| Variables | snake_case | `frame_count` |

### Addresses

Physical and virtual addresses get distinct newtypes and never travel as a bare `usize`. Confusing
the two is the single most common class of kernel bug, and the type system will catch it for free.

### `unsafe`

See `CLAUDE.md` § `unsafe` policy for where it is permitted. Style rules:

- The `unsafe` block wraps the unsound operation, not the function containing it
- Every block carries a `// SAFETY:` comment naming the invariant and who upholds it
- Every `unsafe fn` has a `# Safety` section in its doc comment
- A `// SAFETY:` comment that restates what the code does, rather than why it is sound, is worse than
  none — it looks reviewed

### Documentation

- All public items must have doc comments (`///`)
- Comments explain *why* and state invariants; a comment restating the code is noise
- Use British spelling in documentation 🇬🇧

---

## Assembly

Kept to the minimum the constitution allows: boot and context switching only.

- One instruction per line, with a comment explaining the *intent* of each block
- Register usage documented at the top of every routine, including which are clobbered
- Every routine states the ABI contract it honours on entry and exit
- LF line endings, enforced by `.gitattributes`

---

## YAML (GitHub Actions)

### Indentation

**4 spaces** for structure levels — aligned with project-wide convention.

```yaml
jobs:
    build:
        name: Build
        runs-on: ubuntu-latest

        steps:
            - name: Checkout
              uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
```

### List Item Indentation

List items use **2-space continuation** from the `-` character (standard YAML behaviour):

```yaml
updates:
    - package-ecosystem: "github-actions"
      directory: "/"
      schedule:
        interval: "daily"
```

### Action Pinning

Every `uses:` is pinned to a **full commit SHA** with a trailing `# vX.Y.Z` comment. A mutable tag is
a supply-chain hole, and this project's third pillar is about author-signed artefacts — CI that
trusts a moving tag would be the first thing a reviewer laughed at.

### Multi-line Scripts (`run: |`)

Shell content inside `run: |` blocks uses **4-space indentation** for shell constructs.

### Structure

- Blank line between top-level keys (`on`, `env`, `jobs`)
- Blank line between jobs
- Comments on their own line, never inline — except the trailing `# vX.Y.Z` pin comment that
  § Action Pinning requires

### Formatter

**Format-on-save is disabled** for YAML in VS Code (`.vscode/settings.json`). The Red Hat YAML
extension cannot be configured for this mixed style. Format YAML manually.

---

## JSON, JSONC and TOML

**4 spaces**. VS Code uses the built-in JSON formatter and
[Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml).

---

## Markdown

### Headings

ATX-style, with blank lines before and after.

### Lists

`-` for unordered, `1.` for ordered.

### Code Blocks

Always fenced, always with a language.

### Tables

Preferred for anything with two or more parallel attributes — the constitution's Borrow Ledger is the
model, and the threat model's obligations-by-phase table follows it. Tables are exempt from the
line-length limit.

### Trailing Whitespace

Markdown files are exempt from trailing-whitespace trimming (needed for line breaks).

---

## Prose

The constitution sets the register for the design documents: short declarative sentences, lineage
named for every borrowed idea, verdicts stated rather than hedged. Habits worth keeping:

- **Name the source.** "*(Haiku)*", "*(L4)*", "*(seL4; Fuchsia's downgradeable handle rights)*". An
  idea without a lineage is harder to evaluate and harder to abandon.
- **Record what was rejected**, not only what was chosen. Especially the attractive things — the
  constitution calls this killing darlings, and the record of which darling died is what stops it
  coming back.
- **Prefer a verdict to a survey.** "Write ourselves" and "port code" are useful; "there are several
  approaches" is not.
- **State costs.** What a decision makes harder is as much a part of the decision as what it enables.

Noongar words are used with acknowledgement and spelled as the language custodians spell them — never
anglicised for convenience.

---

## Commit Messages

See `CONTRIBUTING.md` § Commit Messages.

---

## British Spelling 🇬🇧

See `CONTRIBUTING.md` § British Spelling for the rule, and
`.github/scripts/check-british-spelling.sh` for the enforced word list.

**Quick rule:** British spelling in documentation, comments and console output. Code identifiers may
use American spelling where it matches a Rust or hardware-API convention — a register field named in
a datasheet is quoted, not corrected.

**Exceptions, matching what the checker actually excludes:** quoted external text and proper nouns;
API, environment-variable and hardware identifiers where American spelling is the convention (each
recorded in the script's `ALLOWED_PATTERNS` with the convention it honours); and `LICENCE` and
`CODE_OF_CONDUCT.md`, excluded as legal and upstream text. Noongar words are spelled as the language
custodians spell them — see § Prose.

---

*Last updated: 2026-08-01*
