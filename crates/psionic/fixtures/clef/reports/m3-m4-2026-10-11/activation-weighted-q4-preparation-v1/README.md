# Activation-weighted Q4 preparation, stopped at the owner’s request

The two-record calibration pilot and all eight weighted-Q4 CPU synthetic cases passed. The full calibration run was deliberately stopped after 70 of 96 records when the owner separated runtime correctness from BF16 artifact quality. No weighted-Q4 model was generated or tested.

`summary.json` records the results. `stop.json` explains the intentional stop without changing the original collector receipt, which still says `running`. The guard recorded signal 15 and successful cleanup of its owned process group. No desktop app or system service was stopped, and no quiet lease was requested.

The full collector exported no moments: it writes those files only after all records finish. Its completed-request receipts survive. The pilot’s 192 sum/mean pairs remain in local scratch; this bundle includes their verified hashes, widths, and row counts, without arrays or weights.

The frozen corpus contains 96 calibration records and 24 records from separate development families. No development activations or reference answers were collected. The corpus generator used no acceptance cases, recorded errors, or model answers. Its separate overlap checker used acceptance request hashes only to reject exact duplicate states or requests. The corpus is a small synthetic diagnostic set; it does not establish production coverage.

`recipe/plan.md` is the original pre-execution plan and its opening status is historical. This README and `summary.json` describe what actually ran. `portable-recipe.json` gives pinned public dependencies, the unexecuted candidate recipe, and bounds. Source templates preserve the executed logic with absolute paths replaced by named tokens. Rehydrate those tokens in persistent scratch, verify checkpoint hashes on the destination machine, and record new source hashes before any future authorized run. The old local inode/timestamp identity cannot be reused on another machine.

The CPU quantizer checks compared 270,336 decoded values bit for bit with an independent decoder. They checked zero and sparse importance blocks, packed row consistency, guard bytes, and invalid input refusal. These are format checks, not model quality results. A real-weight throughput pilot and complete development evaluation remain unperformed. The old 104-answer BF16 gate is not an activation-calibration source and must not guide recipe fitting; future artifact work follows the separately agreed routes-v5/chat evaluation scope.
