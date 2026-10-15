# Task index edit pilot

Date: 2026-10-15. [Retained results](evaluation-2026-10-15.json).

All twelve model-produced edits passed independent acceptance checks, and
all twelve patches passed a separate replay. The cheaper baseline already
solved both tasks. Recipes reduced navigation calls but increased reported
serving-model cost by about 12%. This pilot does not establish an edit-quality
or total-cost improvement. The feature stays off by default.

## Setup and scope

Two small, frozen Rust repositories exercise different companion obligations:
adding an enum variant and its exhaustive registry mapping, and adding a new
module with its export and registry entry. The stronger model builds typed
recipes from the unchanged source before receiving the task's specific new
variant, module, or value. Separate source-reviewed guides provide a diagnostic
comparison. These guides were reviewed by the implementing agent, not a human.

The requested aliases resolved to `claude-opus-5-5` and `claude-haiku-5-5` in
the recorded calls. Every edit arm has the same restricted Read, Grep, Glob,
Edit, and Write tools. Current arms receive a file list; this is not a replay
of Coder's full production finder and graph toolset. Cases have one trial per
arm, no retry of a failed edit, and independent tests installed after the
model finishes. The test runner checks required companion changes and new
files, records the patch and resulting Git tree, and replays them separately.

The frozen gate requires at least four independent task families, no quality
loss within its paired family bootstrap interval, and complete cost per
accepted task at most 90% of the cheaper baseline. This pilot has two families.
Machine, verification, and refresh costs remain unknown. Both limitations
prevent promotion; no threshold was changed after seeing the results.

## Results

Costs below are provider-reported model costs, not an invoice or a complete
deployment cost. Tokens include cached input where reported. Navigation counts
are observed Read, Grep, and Glob calls.

| Arm | Accepted | Total tokens | Navigation calls | Reported model cost |
| --- | ---: | ---: | ---: | ---: |
| Stronger model, current tools | 2/2 | 41,657 | 5 | $0.103751 |
| Cheaper model, current tools | 2/2 | 69,462 | 6 | $0.003256 |
| Cheaper model, summaries | 2/2 | 73,538 | 6 | $0.003503 |
| Cheaper model, capability map | 2/2 | 60,912 | 5 | $0.002584 |
| Cheaper model, model-built recipes | 2/2 | 72,267 | 4 | $0.003645 |
| Cheaper model, source-reviewed recipes | 2/2 | 77,080 | 4 | $0.003271 |

Recipe construction reported $0.118247. An earlier free-form teacher attempt
returned incorrect field types and was rejected, reporting another $0.054447.
The total retained construction cost is $0.172694. That failure led to explicit
JSON schema enforcement and prompt version `task-recipes-2` before the edit
plan was frozen. It did not become an indexed recipe or an unreported retry.

The observed paired quality difference is zero. With two easy tasks and no
failures, that says little about harder work. No production claim follows from
the degenerate bootstrap interval. The next useful evaluation needs larger
tasks where missing companion code causes real failures, more task families,
repeated trials, historical cutoffs, and complete cost accounting.

## Repeat the process

Use a new private scratch directory. Commands below make real model calls
through the selected Claude login; no saved session, MCP server, or private
history is included. Keep the generated plans, build records, and raw model
logs private. Retain failed preparation attempts and include their reported
cost before freezing a replacement plan.

```sh
cargo run -p code-index -- prepare-evaluation \
  --out ABSOLUTE_SCRATCH_DIR --strong-model MODEL --cheap-model MODEL
cargo run -p code-index -- evaluate \
  --plan ABSOLUTE_SCRATCH_DIR/plan.json \
  --out ABSOLUTE_SCRATCH_DIR/run --target EXISTING_CARGO_TARGET_DIR
cargo run -p code-index -- replay-evaluation \
  --plan ABSOLUTE_SCRATCH_DIR/plan.json \
  --report ABSOLUTE_SCRATCH_DIR/run/report.json \
  --run-dir ABSOLUTE_SCRATCH_DIR/run \
  --out ABSOLUTE_SCRATCH_DIR/replay --target EXISTING_CARGO_TARGET_DIR
```

The general runner accepts larger frozen plans. Parent tasks and their
delegates must share one family and partition; evaluation families cannot
appear in the index's training families. Completed trials resume without a
new model call. A different plan needs a new run directory. Engine failures,
timeouts, incomplete costs, and fixture-only runs remain visible to the gate.
