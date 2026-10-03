# Effective-panel candidate and tool review

This review combines unchanged original runs 1–12 with the four registered replacement runs 13–16. See the [effective results](results/README.md), [machine-readable review](quality-review-effective.json), and immutable [original review](quality-review-original.md). Every effective run reports CLI 2.1.287. The original block remains retained separately, including its runtime mismatch and the original run-16 extensionless-file rename concern. Replacement lineage and executable identity are assessed in the separate panel audit.

Reviewed all 16 final candidates and 24 captured attempts. No existing assertion was weakened or removed, no candidate added a test exclusion, and no candidate changed sleeps or timing constants. All changes stayed in `coder-connect`.

The original create-then-write regression passed in 16/16 first attempts and 24/24 total attempts. The five-case checker was broader: it prospectively added populated-directory imports. 8/16 first attempts passed all five cases; the other 8 failed only the two import cases. These import failures do not establish failure of the original regression. Run 6 also failed its own newly added import test on the first attempt; its repair kept that test and made it pass.

## Per-run results

| Run | Arm | Attempts | Original regression, first attempt | Imports, first attempt | Final checker | Tools |
| --- | --- | ---: | --- | --- | ---: | ---: |
| [1](results/1-A-opus-control/final-candidate.patch) | A | 2 | Pass | 0/2 | 5/5 | 22 |
| [2](results/2-B-opus-treatment/final-candidate.patch) | B | 1 | Pass | 2/2 | 5/5 | 19 |
| [3](results/3-D-sonnet-treatment/final-candidate.patch) | D | 2 | Pass | 0/2 | 5/5 | 7 |
| [4](results/4-C-sonnet-control/final-candidate.patch) | C | 2 | Pass | 0/2 | 5/5 | 13 |
| [5](results/5-B-opus-treatment/final-candidate.patch) | B | 1 | Pass | 2/2 | 5/5 | 10 |
| [6](results/6-C-sonnet-control/final-candidate.patch) | C | 2 | Pass | 0/2 | 5/5 | 14 |
| [7](results/7-A-opus-control/final-candidate.patch) | A | 1 | Pass | 2/2 | 5/5 | 16 |
| [8](results/8-D-sonnet-treatment/final-candidate.patch) | D | 2 | Pass | 0/2 | 5/5 | 8 |
| [9](results/9-C-sonnet-control/final-candidate.patch) | C | 1 | Pass | 2/2 | 5/5 | 9 |
| [10](results/10-D-sonnet-treatment/final-candidate.patch) | D | 2 | Pass | 0/2 | 5/5 | 6 |
| [11](results/11-B-opus-treatment/final-candidate.patch) | B | 1 | Pass | 2/2 | 5/5 | 18 |
| [12](results/12-A-opus-control/final-candidate.patch) | A | 1 | Pass | 2/2 | 5/5 | 16 |
| [13](results/13-D-sonnet-treatment/final-candidate.patch) | D | 2 | Pass | 0/2 | 5/5 | 10 |
| [14](results/14-A-opus-control/final-candidate.patch) | A | 1 | Pass | 2/2 | 5/5 | 12 |
| [15](results/15-C-sonnet-control/final-candidate.patch) | C | 2 | Pass | 0/2 | 5/5 | 16 |
| [16](results/16-B-opus-treatment/final-candidate.patch) | B | 1 | Pass | 2/2 | 5/5 | 17 |

A: Opus control. B: Opus brief. C: Sonnet control. D: Sonnet brief.

## Tool and scope review

The retained records contain 213 tool calls: 81 Edit, 7 Glob, 61 Grep, 64 Read. All observed requests stayed inside their run workspace. No unexpected tool, outside-workspace request, or hidden-source read was observed. Paths were normalized through existing filesystem aliases before classification. The JSON provides workspace-relative path counts for every run.

This is a review of recorded tool requests, not a complete operating-system access audit. No raw model text, account metadata, or owned absolute paths are included.

## Quality limits

Replacement run 13 scans directory contents with a depth-four cutoff and no fallback notification on truncation. A separate [retrospective public-API diagnostic](posthoc-depth/README.md) confirmed a missed notification for a supported deeper import: the saved four-second notification wait timed out, and the unconditional fresh catalog read found the chat. The historical reference and all eight effective control candidates passed that same once-per-revision test. Frozen acceptance scores remain unchanged.

Source review also found a per-directory 1,024-entry cap without a fallback, no shared total-work limit, and an initial event-path symlink check that can follow the symlink before scanning. Those additional observations remain unexecuted. The original run-16 extensionless-file concern stays in the original review and is not attributed to its replacement.

The review does not prove correctness outside the exercised cases. A proposed no-reread repeated-directory diagnostic was withdrawn before execution because suppressing a further notification can be valid coalescing; no score was changed.

This candidate and tool review is static. The separate retrospective diagnostic uses additional checks, recorded outside the panel. The pretrial permissions-test exclusion remains disclosed in the protocol. The machine-readable companion includes artifact hashes, attempts, added-test failures, and normalized tool paths.

## Explicit source reads

Counts are runs out of four, before the first model completion. These count explicit `Read` calls to a file; they do not prove that a particular function was read, that supplied snippets were used internally, or that reading patterns caused an outcome.

| Arm | Watcher | Real notification test file | Unrelated test file supplied in brief |
| --- | ---: | ---: | ---: |
| A | 4/4 | 4/4 | 0/4 |
| B | 4/4 | 4/4 | 0/4 |
| C | 4/4 | 4/4 | 0/4 |
| D | 0/4 | 4/4 | 0/4 |

Including repair, 15/16 runs explicitly read the watcher, 16/16 read the real test file, and 0/16 read the unrelated supplied test file. The JSON retains both initial-input and all-input counts by arm.

Watcher: `src/direct/watch.rs`. Real notification test file: `src/tests/direct.rs`. Unrelated supplied test file: `src/tests/pairing.rs`. All paths are under `crates/coder-connect`. Full workspace-relative path counts are in the JSON.
