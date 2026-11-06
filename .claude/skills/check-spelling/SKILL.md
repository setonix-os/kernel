---
name: check-spelling
description: "Audit the repository for British English spelling and offer to fix every violation. Use when prose, comments or console output change."
---

Audit the repository for British English spelling and offer to fix every violation.

## How

The word list is **not** duplicated here. It lives in
`.github/scripts/check-british-spelling.sh`, which is also what CI runs — one canonical source, per
`STYLE.md` § Single Source of Truth. Run it:

```bash
bash .github/scripts/check-british-spelling.sh
```

Then, for each reported hit, decide which of three things it is:

1. **A genuine violation** — prose, a comment or a doc comment using an American spelling. Fix it.
2. **A legitimate API identifier** — a Rust trait (`Serialize`), a hardware register or field name
   from a datasheet, a protocol method name, an environment variable. Do not rename it. Add a pattern
   to `ALLOWED_PATTERNS` in the script with a comment naming the convention it honours.
3. **Vendored code** — should already be excluded. If a `vendor/` path appears in the output, the
   exclusion has broken; fix the pathspec rather than the file. Rewording upstream MIT code corrupts
   the provenance the licence obliges us to preserve (CONSTITUTION.md §11.5).

If `$ARGUMENTS` names specific files, restrict your attention to those.

## Then

Report every violation with file path, line number, the offending word and its British equivalent,
grouped by which of the three categories above it falls into. Offer to fix category 1 in bulk, and
propose the exact `ALLOWED_PATTERNS` additions for category 2 rather than applying them silently.

If the script exits clean, say so in one line. Do not pad the result.

## Lessons

- **2026-11-06 — the script checks words, not typography.** Inline code spans broken across a line end
  passed both the spelling script and markdownlint, and were found only by a separate scan (commit
  `888a847`). `.github/scripts/check-code-spans.sh` now gates both in CI.
- **2026-11-06 — a gate that can pass on an empty input is not a gate.** In CI the container's root
  does not own the checkout, git refused it as "dubious ownership", and this script reported
  "OK: no files to check" on every run since it was added: the spelling gate had never run in CI.
  It now fails when git fails or lists nothing. When a check reports success, read its count, and
  make any new gate refuse an empty input.
