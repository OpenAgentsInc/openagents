# Project activity history: original commit per completion

A project's activity may contain more than one completion for the same issue.
The old Coax fold rendered every historical completion with the issue's latest
commit, while the current TypeScript `model.ts` records the commit from each
individual event. The live page now reads the commit tag from the event that
created the row. This is a display fix: it does not add local Nostr signature
verification, authorize project actions, or change the relay trust boundary.

`coax/tests/nostr/project_history.py` signs an isolated public-key fixture with
two different completion hashes for one issue, uses the existing TypeScript
fold/table code as an independent expected result, and compares it with the
actual Coax parser/fold on native checked/release with ASan/UBSan and Wasm.
Before the fix the original Coax code printed the newer hash on both rows; the
fixture failed with the expected and observed logs recorded in
`/tmp/coax-0845-history-negative.log` on the development machine. After the
fix both rows match the reference. The fixture test key is public and used
only to construct test data. Coax's feed path in this test does not verify
those signatures.

The existing `project-fold.coax` compile-time captured-transcript check still
cannot allocate its production-sized Store/Fold/Tables inside the unchanged
Coax aggregate storage cap; `coax test` reports E0400 for that existing check
(20 other imported checks passed). The unfinished #0845 borrowed-runner work
tracks exercising the real transition code with borrowed buffers and a complete
authority mutation gate. This display fix does not close #0845 or #c969.
