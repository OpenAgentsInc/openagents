# Retained v18 family records

These files are copied from the completed v18 family on coderos. They do not
include or read the separate artifact-v2 #9584 outcomes.

- `original/`: unchanged driver, state, launch log, and historical tally.
  The driver's cost field coerces unknown totals to zero; use the corrected
  `../measurement.json`, not that field, for counted spend.
- `attempts/`: all 18 official attempt records, collection manifests, usage
  ledgers, doctor outputs, launch configurations, outcome fields, and numeric
  verifier summaries.
- `cards/`: the 18 existing Gym run cards, in JSON and Markdown. Their qualitative
  text is retained for provenance. Confirmation analysis uses only counts and
  outcome fields, as the frozen protocol requires.
- `candidates/batch.json`: the original post-run candidate grading batch,
  including all invalid grades and discovery errors.
- `setup-refusals/`: all five earlier setup-only outcomes and their remote
  file inventory. They remain outside the model pass/fail denominator.
- `loop-counts.json` and `timing-counts.json`: outcome and count extraction
  without interpreting confirmation transcript prose.
- `evidence-verification.json`: all 153 resolvable original collection-manifest
  references matched their retained SHA-256 digests at publication.
- `trace-inventory.json`: remote paths, sizes, and digests for native traces,
  artifacts, and verifier files. Full transcripts remain on coderos at these
  exact locations. This inventory does not embed their contents.
- `audit.json`: artifact and policy hashes, source state, and checkout history.

The parent `evidence-index.json` seals every file in this directory. The
[report](../../../../../docs/terminal-bench/2026-09-25-microluna-v18-family.md)
distinguishes a completed-cohort loss from a strict protocol verdict of
inconclusive, and explains the accounting and candidate-coverage limitations.
