---
# SPDX-License-Identifier: GPL-3.0-or-later
name: steward
description: "Keep the project's skills true: fold in what this session taught, fix stale commands, merge or split skills. Run from /handoff at the end of a working session, and whenever a skill's instructions turn out wrong."
---

The skills are how a session starts from what earlier sessions learned, so a stale one misleads
every session after it.

1. Read this session's commits and recall its friction. Pain is the best input.
2. A lesson goes in the skill it concerns, dated, with its cause, in the same pull request as the fix
   that taught it (`CLAUDE.md` § Claude Code skills). Write it as a rule: a story is read once.
3. Run or dry-read each command in the skills you touched, and fix what is stale.
4. Delete advice that no longer applies. Keep a skill short enough to read whole.
5. A multi-step procedure done by hand for the second time becomes a skill, with a description that
   says when to use it.

A rule that belongs to everyone goes in `CLAUDE.md`, not a skill. Constitution text is the
maintainer's: flag it, do not take it.

## Lessons

- **2026-11-06 — the steward is the one who notices.** This skill exists because lessons had
  gathered in a handoff file, in no skill, where a later session would never read them.
