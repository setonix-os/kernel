Review all changed or specified files for `STYLE.md` compliance.

If `$ARGUMENTS` names no files, check everything modified since the last commit.

## What the tools already cover

Run these first and report their output rather than re-deriving it by eye:

```bash
cargo fmt --all --check
cargo clippy --package setonix-kernel --all-features --target aarch64-unknown-none-softfloat -- -D warnings
cargo clippy --package setonix-kernel --all-features --target x86_64-unknown-none -- -D warnings
cargo clippy --package setonix-capability --target aarch64-unknown-none-softfloat -- -D warnings
cargo clippy --package setonix-capability --target x86_64-unknown-none -- -D warnings
cargo clippy --package setonix-capability --all-targets --all-features -- -D warnings
cargo clippy --package xtask --all-targets --all-features -- -D warnings
markdownlint-cli2 "**/*.md"
bash .github/scripts/check-toolchain-pin.sh
bash .github/scripts/check-british-spelling.sh
```

Your job is the part no tool checks.

## Rust — beyond rustfmt

- Naming: crates and modules `snake_case`, types `PascalCase`, functions `snake_case`,
  constants `SCREAMING_SNAKE_CASE`
- Every public item has a `///` doc comment; `missing_docs` is a warning, not a suggestion
- Doc comments explain *why* and state invariants — a comment that restates the code is noise
- Physical and virtual addresses use distinct newtypes, never bare `usize`
- No architecture name appears above `arch/mod.rs`
- 100-column limit for Rust (`rustfmt.toml`), 170 for everything else (`.editorconfig`)

## YAML (GitHub Actions)

- 4-space structure indentation
- 2-space continuation from list items
- Blank lines between top-level keys and between jobs
- Comments on their own line, never inline (the trailing `# vX.Y.Z` pin comment is the one exception)
- Every `uses:` is pinned to a full commit SHA with a `# vX.Y.Z` comment — a bare tag is a finding

## JSON, JSONC, TOML

- 4-space indentation

## Markdown

- ATX headings with blank lines before and after
- Dash lists (`-`), numeric ordered lists (`1.`)
- Fenced code blocks, always with a language

## Cross-cutting

- **Single source of truth**: flag any fact stated in two places. Name which one should be canonical
  and which should become a cross-reference. This is the rule most often broken and the most
  expensive to leave, because the copies diverge silently.
- British spelling in documentation and comments (CONSTITUTION.md §11.6)

## Output

Report every violation with file path, line number and what is wrong. Group by file. If a rule in
`STYLE.md` is itself ambiguous or now contradicts the configuration files, say so — that is a finding
about `STYLE.md`, not about the code.
