# Original candidate and tool review

This review preserves the original panel. See the [retained results](results-original/README.md) and [machine-readable review](quality-review-original.json). Replacement-block evidence is documented separately.

The original panel is protocol-invalid: run 16 used Claude CLI 2.1.288 instead of registered 2.1.287, and the coordinator rejected it. The code-check results below remain descriptive evidence; they do not restore runtime validity or support a cost-win claim for this original panel.

Reviewed all 16 final candidates and 25 captured attempts. No existing assertion was weakened or removed, no candidate added a test exclusion, and no candidate changed sleeps or timing constants. All changes stayed in `coder-connect`.

The original create-then-write regression passed in 16/16 first attempts and 25/25 total attempts. The five-case checker was broader: it prospectively added populated-directory imports. 7/16 first attempts passed all five cases; the other 9 failed only the two import cases. These import failures do not establish failure of the original regression. Run 6 also failed its own newly added import test on the first attempt; its repair kept that test and made it pass.

## Per-run results

| Run | Arm | Attempts | Original regression, first attempt | Imports, first attempt | Final checker | Tools |
| --- | --- | ---: | --- | --- | ---: | ---: |
| [1](results-original/1-A-opus-control/final-candidate.patch) | A | 2 | Pass | 0/2 | 5/5 | 22 |
| [2](results-original/2-B-opus-treatment/final-candidate.patch) | B | 1 | Pass | 2/2 | 5/5 | 19 |
| [3](results-original/3-D-sonnet-treatment/final-candidate.patch) | D | 2 | Pass | 0/2 | 5/5 | 7 |
| [4](results-original/4-C-sonnet-control/final-candidate.patch) | C | 2 | Pass | 0/2 | 5/5 | 13 |
| [5](results-original/5-B-opus-treatment/final-candidate.patch) | B | 1 | Pass | 2/2 | 5/5 | 10 |
| [6](results-original/6-C-sonnet-control/final-candidate.patch) | C | 2 | Pass | 0/2 | 5/5 | 14 |
| [7](results-original/7-A-opus-control/final-candidate.patch) | A | 1 | Pass | 2/2 | 5/5 | 16 |
| [8](results-original/8-D-sonnet-treatment/final-candidate.patch) | D | 2 | Pass | 0/2 | 5/5 | 8 |
| [9](results-original/9-C-sonnet-control/final-candidate.patch) | C | 1 | Pass | 2/2 | 5/5 | 9 |
| [10](results-original/10-D-sonnet-treatment/final-candidate.patch) | D | 2 | Pass | 0/2 | 5/5 | 6 |
| [11](results-original/11-B-opus-treatment/final-candidate.patch) | B | 1 | Pass | 2/2 | 5/5 | 18 |
| [12](results-original/12-A-opus-control/final-candidate.patch) | A | 1 | Pass | 2/2 | 5/5 | 16 |
| [13](results-original/13-D-sonnet-treatment/final-candidate.patch) | D | 2 | Pass | 0/2 | 5/5 | 7 |
| [14](results-original/14-A-opus-control/final-candidate.patch) | A | 1 | Pass | 2/2 | 5/5 | 22 |
| [15](results-original/15-C-sonnet-control/final-candidate.patch) | C | 2 | Pass | 0/2 | 5/5 | 20 |
| [16](results-original/16-B-opus-treatment/final-candidate.patch) | B | 2 | Pass | 0/2 | 5/5 | 24 |

A: Opus control. B: Opus brief. C: Sonnet control. D: Sonnet brief.

## Tool and scope review

The retained records contain 231 tool calls: 90 Edit, 9 Glob, 62 Grep, 70 Read. All observed requests stayed inside their run workspace. No unexpected tool, outside-workspace request, or hidden-source read was observed. Paths were normalized through existing filesystem aliases before classification. The JSON provides workspace-relative path counts for every run.

This is a review of recorded tool requests, not a complete operating-system access audit. No raw model text, account metadata, or owned absolute paths are included.

## Quality limits

[Run 16’s final patch](results-original/16-B-opus-treatment/final-candidate.patch) has a separate static quality concern: its repaired rename predicate treats a missing extensionless path as a directory. Renaming an ordinary extensionless file such as `notes` or `README` can therefore emit a catalog nudge from the old path, contrary to ordinary-file quietness. Its added test covers only an existing `notes.txt` destination. This source-based finding was not executed and does not alter the five-case score; the CLI mismatch independently invalidates that run.

The review does not prove correctness outside the exercised cases. A proposed no-reread repeated-directory diagnostic was withdrawn before execution because suppressing a further notification can be valid coalescing; no score was changed.

No new Cargo, model, or remote checks ran for this review. The pretrial permissions-test exclusion remains disclosed in the protocol. The machine-readable companion includes artifact hashes, attempts, added-test failures, and normalized tool paths.

## Explicit source reads

Counts are runs out of four, before the first model completion. These count explicit `Read` calls to a file; they do not prove that a particular function was read, that supplied snippets were used internally, or that reading patterns caused an outcome.

| Arm | Watcher | Real notification test file | Unrelated test file supplied in brief |
| --- | ---: | ---: | ---: |
| A | 4/4 | 4/4 | 0/4 |
| B | 4/4 | 4/4 | 0/4 |
| C | 4/4 | 3/4 | 0/4 |
| D | 0/4 | 4/4 | 0/4 |

Including repair, 16/16 runs explicitly read the watcher, 15/16 read the real test file, and 0/16 read the unrelated supplied test file. The JSON retains both initial-input and all-input counts by arm.

Watcher: `src/direct/watch.rs`. Real notification test file: `src/tests/direct.rs`. Unrelated supplied test file: `src/tests/pairing.rs`. All paths are under `crates/coder-connect`. Full workspace-relative path counts are in the JSON.
