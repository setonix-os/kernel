Audit the project's documents for internal coherence. Run this before any constitutional amendment lands, and periodically regardless.

If `$ARGUMENTS` names documents, audit those. Otherwise audit `CLAUDE.md` and everything under `docs/`.

## Why this exists

Constitution §1 and §5.4: coherence beats accumulation. A document set decays in a specific way — not
by becoming wrong, but by accumulating clauses that were each right when written and now disagree with
each other. That decay is invisible from inside any single document, which is why it needs a pass of
its own.

## 1. Contradictions

Find clauses that cannot both be true, or that a reasonable reader would act on differently.

Report each as a pair, quoting both sides, and say which one you believe is load-bearing and which is
the stale survivor. **Do not resolve it** — §4 and §11.1 reserve constitutional amendments to the
maintainer. Present the choice.

## 2. Orphaned claims

Statements about things that no longer exist, live somewhere else, or were never built. File paths,
repository layouts, subsystem names and roadmap phases all rot quietly. Verify anything the document
asserts about the filesystem or the repositories actually holds.

## 3. Derivation from the primitive

Every pillar, doctrine and RFC should be traceable to:

> An application is a self-contained, signed, identity-bearing, immutable, content-addressed blob —
> and every resource it touches is a scheme mediated by a broker.

List anything that cannot be derived. That list is the set of darlings awaiting a decision — §1 says
they do not go in however attractive they are, so an undecided darling sitting in the documents is a
real finding, not a nitpick.

## 4. Undocumented decisions

Places where the documents assume a decision that is written down nowhere. These are the most
expensive gaps, because the reasoning is lost while the consequence persists. Each one should become
an RFC.

## 5. The Borrow Ledger

Constitution §4. Check that every subsystem the documents discuss appears in the ledger with a verdict
and an author, and that no verdict has been silently outgrown by what the design now needs. The
filesystem row is the known live case: flag it if it is still unsettled.

## 6. Single source of truth

`STYLE.md` § Single Source of Truth. Find any fact stated in two places, name which should be canonical
and which should become a cross-reference. Duplicated prose is how contradictions get born.

## 7. Scope drift

Constitution §6. Flag anything that has quietly acquired a non-goal: desktop polish, POSIX
bug-compatibility, a third architecture, Apple Silicon, GPU acceleration beyond the framebuffer. Also
flag ambition inflation — adoption targets, user counts and market framing that the constitution
deliberately pruned when the project was renamed.

## Output

One section per numbered check, each with **pass** or the findings. Quote the text. End with the three
findings you would fix first and why those three.

Be willing to conclude that the documents are coherent. A coherence audit that manufactures findings
to look thorough is worse than useless, because it trains the reader to ignore the next one.
