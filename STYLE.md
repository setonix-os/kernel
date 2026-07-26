# Style Guide

Conventions for the Setonix documents. The kernel repository has its own
[STYLE.md](https://github.com/setonix-os/kernel/blob/main/STYLE.md) covering Rust, assembly and YAML;
this file covers prose.

---

## General Rules

| Rule | Setting |
|------|--------|
| Indentation | 4 spaces (no tabs) |
| Max line length | 170 characters |
| Charset | UTF-8 |
| Final newline | Always |
| Trailing whitespace | Trim (except Markdown) |
| Line endings | LF, in the repository and the working tree |

Enforced by `.editorconfig` and `.gitattributes`. Install the
[EditorConfig plugin](https://marketplace.visualstudio.com/items?itemName=EditorConfig.EditorConfig).

---

## Single Source of Truth

Each fact has exactly one canonical location. Everything else cross-references it.

| Information | Canonical Source |
|-------------|------------------|
| The constitution | `CLAUDE.md` — and nowhere else, by design |
| Threat model | `docs/threat-model.md` |
| Design decisions | `docs/rfcs/` |
| Contribution process | `CONTRIBUTING.md` |
| British spelling word list | `.github/scripts/check-british-spelling.sh` |
| Formatting rules | `.editorconfig` |

This repository is where the rule matters most. The constitution exists in one place so that no
session, no contributor and no repository can be working from a stale copy of project law.

---

## Markdown

### Headings

ATX-style (`##`), with blank lines before and after. One `#` per document.

### Lists

`-` for unordered, `1.` for ordered. Four-space indentation for nesting.

### Code Blocks

Always fenced, always with a language. Use `text` for console output and plain data.

### Tables

Preferred for anything with two or more parallel attributes — the constitution's Borrow Ledger is the
model. Tables are exempt from the line-length limit.

### Trailing Whitespace

Exempt from trimming, since Markdown needs it for line breaks.

### Linting

```bash
markdownlint-cli2 "**/*.md"
```

Configured by `.markdownlint.json` and `.markdownlint-cli2.jsonc`. `CODE_OF_CONDUCT.md` is excluded
and must not be modified.

---

## Prose

The constitution sets the register: short declarative sentences, lineage named for every borrowed
idea, verdicts stated rather than hedged. Some habits worth keeping:

- **Name the source.** "*(Haiku)*", "*(L4)*", "*(seL4; Fuchsia's downgradeable handle rights)*". An
  idea without a lineage is harder to evaluate and harder to abandon.
- **Record what was rejected**, not only what was chosen. Especially the attractive things — the
  constitution calls this killing darlings, and the record of which darling died is what stops it
  coming back.
- **Prefer a verdict to a survey.** "Write ourselves" and "port code" are useful; "there are several
  approaches" is not.
- **State costs.** What a decision makes harder is as much a part of the decision as what it enables.

---

## British Spelling 🇬🇧

British spelling throughout. Enforced in CI by `.github/scripts/check-british-spelling.sh`, which
holds the canonical word list.

Exceptions: quoted external text, proper nouns, and API identifiers where American spelling is the
convention. `LICENCE` and `CODE_OF_CONDUCT.md` are excluded as legal and upstream text.

Noongar words are used with acknowledgement and spelled as the language custodians spell them — never
anglicised for convenience.

---

*Last updated: 2026-07-26*
