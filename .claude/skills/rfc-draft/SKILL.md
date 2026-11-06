---
# SPDX-License-Identifier: GPL-3.0-or-later
name: rfc-draft
description: "Draft or revise an RFC under docs/rfcs/, and check it before review. Use for any design decision (constitution §5.5)."
---

`CONTRIBUTING.md` § Writing an RFC is the rule for what an RFC holds. This skill is how to get there
without paying for it in review rounds.

## Before writing

Run `git fetch`, and list the RFCs on `origin/main` and on open branches: a number or a topic may
already be taken. Read the constitution's Borrow Ledger for the subsystem, and the threat-model
obligations it must discharge.

## Before asking for review, check your own text

1. **Absolutes.** Test each "never", "nothing", "every", "cannot" and "only" against the design, and
   state its guards and limits. If a counter-example exists, the sentence is wrong.
2. **Citations.** Run `/fact-check` over what the RFC cites.
3. **Superseded records.** When amending or superseding a decision, grep the tree for its number and
   update what leans on it. An accepted RFC changes only by a dated amendment.
4. **Seams.** Name what this RFC takes from its siblings, and record a debt in the document that owes
   it, not only in the document that noticed it.
5. **Other RFCs' section numbers.** Check that each one says what you claim, not merely that it exists.
6. **The graves.** All of them, by name, as constitution §3 lists them.

## Hard parts first

For a design with hard cases, write down every case and the result each must give before drafting
mechanisms. Meeting them one review round at a time costs far more.

If reviews keep finding holes at the seams, stop and model it (`/pr-review` § When to stop).

## Lessons

- **2026-11-06 — a number is not a reservation.** A bootstrap draft sat uncommitted while another
  session used its number for a different RFC, and three of its rules were nearly lost. Push a draft
  early, on its own branch, whatever `/pre-push` says of finished work.
- **2026-11-06 — one word can overclaim.** "Nothing is lost" was false while the project's own
  changelog recorded the exception. This is why step 1 exists.
