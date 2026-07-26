# Pull Request

## Summary

<!-- What does this change do, and why? One paragraph. -->

## Explanation for the maintainer

<!--
CLAUDE.md §5.2: nothing merges un-understood. §11.4: every non-trivial change
ships with an explanation the maintainer can verify their understanding against.

Explain the change as you would to someone who will have to debug it at 2am
without you. Name the invariants it relies on and the ones it establishes.
-->

## Which pillar does this serve?

<!--
CLAUDE.md §1: every addition must justify itself as a consequence of the
primitive. If it cannot, it does not go in — however attractive it is.
State the pillar, or state plainly that this is toil (tooling, tests, docs).
-->

## Borrow Ledger

- [ ] I checked the Borrow Ledger (CLAUDE.md §4) for this subsystem
- [ ] The verdict for this subsystem is: <!-- write ourselves / port code / n/a -->
- [ ] Vendored code retains its original licence notices; new files carry an SPDX header

## `unsafe` register

<!--
CLAUDE.md §11.3. Leave "None" if there are no new `unsafe` blocks — do not
delete this section, its emptiness is itself the useful signal.
-->

- New `unsafe` blocks: <!-- None, or a list of file:line with the invariant each relies on -->
- [ ] Every new `unsafe` block carries a `// SAFETY:` comment
- [ ] Every `unsafe` block is inside a module CLAUDE.md designates for it
- [ ] I have listed them in the session summary / PR description above

## Checklist

- [ ] Builds for both Tier-1 targets (`aarch64-unknown-none-softfloat` and `x86_64-unknown-none`)
- [ ] Clippy passes (`cargo clippy --all-targets --all-features -- -D warnings`)
- [ ] Formatted (`cargo fmt --all --check`)
- [ ] Toolchain pins agree (`bash .github/scripts/check-toolchain-pin.sh`)
- [ ] British spelling (`bash .github/scripts/check-british-spelling.sh`)
- [ ] Markdown is lint-clean (`markdownlint-cli2 "**/*.md"`)
- [ ] Boots in QEMU where applicable
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] Commit messages follow Conventional Commits (see `CONTRIBUTING.md`)

## Related

<!-- Fixes #123 / relates to an RFC in the docs repository -->
