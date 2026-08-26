# Cycle reviews

One file pair per reviewed autoimprovement cycle, written by
`pnpm run coder:review` (runbook §6):

- `YYYY-MM-DD-<lever-slug>.md` — the review a human reads and adopts from.
- `YYYY-MM-DD-<lever-slug>.json` — the same review as data
  (`openagents.coder_review_document.v1`), so the adopt step can become a
  diff rather than a reading exercise. Its proposals are
  `openagents.coder_candidate.v1` candidates: the same type an optimizer
  mutation will carry, so a reflection and a mutation are one object.

Over time this directory is a dataset of which process changes actually
helped, which is the reason each file keeps its evidence refs rather than
only its conclusion.

**A refused review leaves nothing here.** Every claim in a review cites the
artifacts the reviewer was given — `trial:<task>#step-<id>`,
`trial:<task>#outcome`, `row:<suite>#<recordedAt>`, `ledger:<id>`,
`diff:<path>` — and a citation that does not resolve refuses the whole
review by name, before any file is written. A file in this directory is a
record of a judgment about a run that happened; a document that reads like
one and is not is the failure the loop exists to catch.

A review whose reviewer ref begins `replay:` was replayed from a recorded
response rather than produced fresh. Read it as a record of the replay.
