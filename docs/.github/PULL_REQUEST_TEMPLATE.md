# Pull Request

## Summary

<!-- What does this document change, and why? One paragraph. -->

## Nature of the change

- [ ] New RFC or design document
- [ ] Amendment to the constitution (`CLAUDE.md`) — **maintainer only**
- [ ] Threat model expansion
- [ ] Correction, clarification or typo

## If this amends the constitution

<!--
CLAUDE.md is project law and is revised only by the maintainer (§4, §11.1).
State what changes, what it supersedes, and what code or decisions it
invalidates. A constitutional amendment that leaves a stale clause elsewhere in
the document is worse than no amendment.
-->

## Coherence check

<!--
CLAUDE.md §1 and §5.4: coherence beats accumulation. Does this document add a
consequence of the primitive, or a darling that needs killing?
-->

## Checklist

- [ ] British spelling (`bash .github/scripts/check-british-spelling.sh`)
- [ ] Markdown is lint-clean (`markdownlint-cli2 "**/*.md"`)
- [ ] Cross-references still resolve; no clause is left contradicting another
- [ ] `CHANGELOG.md` updated under `[Unreleased]` if this is a substantive change

## Related

<!-- Fixes #123 / supersedes RFC-000X -->
