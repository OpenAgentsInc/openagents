# Alpha deterministic preparation preview

This is one preparation preview for the qualified historical task [#9908](https://github.com/OpenAgentsInc/openagents/issues/9908). It uses source commit `09f4e0915150f503e332c2b213d24f793c433f3b`, the frozen `syntax-units-v2` preparer, and the existing frozen `briefing-lab` binary. No model or Cargo command ran, and no executor outcome was measured. Keep this observation separate from the original 20-preview series.

## Inputs and provenance

The [safe task manifest](inputs/public-task-manifest.json) supplies the normalized title and task body. The [base prompt](inputs/base-prompt.txt) uses the original common operational envelope with `coder-access` as the package. The [normalized issue input](inputs/issue.json) is the only task input passed to the preparer. The [original public issue body](inputs/original-issue-body.md) is retained as provenance; it is not an additional executor instruction. [Input provenance](inputs/provenance.json) binds these files and the applicable instructions and readings to the historical source.

Alpha's prior Linux qualification used helper `f2c40fe470ccd92dbdcf4e9a72cfc03310583026a677114638e8355cdaa498ee` under the default-feature policy. This preview does not repeat or replace that qualification. It does not alter the original task inputs, preview measurements, or draft schedule.

## Single observation

| Phase | Process wall time | Retained result |
| --- | ---: | --- |
| Syntax index construction | 4.771843 s | 31,589,070 bytes; 3,961 indexed files |
| Deterministic preview | 0.665204 s | 15,449-byte briefing; 11 selected source units |

The preparer reports 0.543656 seconds inside the preview process, including 0.531367 seconds for source selection. The process measurement also includes interpreter startup and output writes. Index construction is measured separately; no operating-system cache flush was performed. These are single observations, not a latency distribution or evidence of faster task completion.

The [receipt](preview-receipt.json) records timings and identities. The exact [briefing](preview/briefing.md), [preparation record](preview/preparation.json), and [candidate pool](preview/candidates.json) are retained. All 13 candidate file hashes and exact line spans were checked against the pinned source. The full syntax index and execution logs are retained privately; the receipt binds the index and retained archive by SHA-256.

## Retrieval limits

The frozen policy read 24 ranked Rust files and retained 13 candidate units. The briefing includes `Host::request_enrollment`, `MAX_GRANTS`, `GrantRecord`, and an invitation retry test. Two selected units come from `coder-computers`, outside the allowed edit root. `Host::redeem` and `Host::handle_with_clock` are present in the candidate pool but not in the final briefing; `Host::execute` exceeds the 6,000-byte unit limit and is excluded from the pool. The preview therefore does not establish complete implementation or test coverage. These omissions were retained without changing the policy or rerunning selection.

## Reproduce preparation

Use the binary and script hashes in the receipt, an existing repository containing the exact source commit, and a new output directory outside that repository. The following paths are caller-supplied placeholders:

```sh
briefing-lab index --repo "$REPO" \
  --rev 09f4e0915150f503e332c2b213d24f793c433f3b \
  --syntax --output "$OUTPUT/index.json"
python3 bench/delegation-study/prepare.py --repo "$REPO" \
  --rev 09f4e0915150f503e332c2b213d24f793c433f3b \
  --index "$OUTPUT/index.json" --issue "$INPUTS/issue.json" \
  --mode deterministic --output "$OUTPUT/preview"
```
