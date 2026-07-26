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
   the provenance the licence obliges us to preserve (CLAUDE.md §11.5).

If `$ARGUMENTS` names specific files, restrict your attention to those.

## Then

Report every violation with file path, line number, the offending word and its British equivalent,
grouped by which of the three categories above it falls into. Offer to fix category 1 in bulk, and
propose the exact `ALLOWED_PATTERNS` additions for category 2 rather than applying them silently.

If the script exits clean, say so in one line. Do not pad the result.
