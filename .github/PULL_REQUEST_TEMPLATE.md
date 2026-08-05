# Pull Request

## Summary

<!-- What does this change do, and why? One paragraph. -->

## Explanation for the maintainer

<!--
CONSTITUTION.md §5.2: nothing merges un-understood. §11.4: every non-trivial
change ships with an explanation the maintainer can verify their understanding
against.

Explain the change as you would to someone who will have to debug it at 2am
without you. Name the invariants it relies on and the ones it establishes.
-->

## Which pillar does this serve?

<!--
CONSTITUTION.md §1: every addition must justify itself as a consequence of the
primitive. If it cannot, it does not go in — however attractive it is.
State the pillar, or state plainly that this is toil (tooling, tests, docs).
-->

## If this changes a document or the constitution

<!--
Delete this section for a pure code change. Keep it for anything touching
CONSTITUTION.md, docs/rfcs/, docs/threat-model.md, or the contributor docs.

Constitution amendments are the maintainer's alone (§4, §11.1). When one lands
it must leave no stale clause behind: an amendment that contradicts another
clause is worse than no amendment, because it makes every future reader guess.
State what it supersedes and what code or decisions it invalidates.
-->

- [ ] Cross-references still resolve; no clause is left contradicting another (`/check-coherence`)
- [ ] If this is a constitution amendment, it is the maintainer's, and `docs/CHANGELOG.md` records it
- [ ] No section other documents cite has been renumbered

## Borrow Ledger

- [ ] I checked the Borrow Ledger (CONSTITUTION.md §4) for this subsystem
- [ ] The verdict for this subsystem is: <!-- write ourselves / port code / n/a -->
- [ ] Vendored code retains its original licence notices; new files carry an SPDX header

## `unsafe` register

<!--
CONSTITUTION.md §11.3. Leave "None" if there are no new `unsafe` blocks — do not
delete this section, its emptiness is itself the useful signal.
-->

- New `unsafe` blocks: <!-- None, or a list of file:line with the invariant each relies on -->
- [ ] Every new `unsafe` block carries a `// SAFETY:` comment
- [ ] Every `unsafe` block is inside a module CLAUDE.md designates for it
- [ ] I have listed them in the session summary / PR description above

## Checklist

- [ ] Builds for both Tier-1 targets (`aarch64-unknown-none-softfloat` and `x86_64-unknown-none`)
- [ ] Clippy passes for every package, exactly as CI runs it (see `CONTRIBUTING.md` § Development Setup)
- [ ] Formatted (`cargo fmt --all --check`)
- [ ] Toolchain pins agree (`bash .github/scripts/check-toolchain-pin.sh`)
- [ ] British spelling (`bash .github/scripts/check-british-spelling.sh`)
- [ ] Markdown is lint-clean (`markdownlint-cli2 "**/*.md"`)
- [ ] Boots in QEMU where applicable
- [ ] `CHANGELOG.md` updated under `[Unreleased]`
- [ ] Commit messages follow Conventional Commits (see `CONTRIBUTING.md`)

## Related

<!-- Fixes #123 / relates to an RFC under docs/rfcs/ -->
