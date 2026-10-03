# Gamma deterministic preparation preview

This is one preparation preview for qualified public issue [#9424](https://github.com/OpenAgentsInc/openagents/issues/9424) at source commit `c427943a5c84ba5938a3549f24b27de551812a37`. It uses the frozen `syntax-units-v2` preparer and existing frozen `briefing-lab` binary. No model or Cargo command ran, and no executor outcome was measured. Keep this observation separate from the original 20-preview series and the Alpha preview.

## Inputs and provenance

The [safe task manifest](inputs/public-task-manifest.json) supplies the exact normalized title and task body. The [base prompt](inputs/base-prompt.txt) keeps the original common operational envelope, with the declared edit roots and named package checks. The common check command explicitly enables `jev/blocking`; it does not enable `live`. The [normalized issue input](inputs/issue.json) is the only task input passed to the preparer. The [retrieved public issue body](inputs/original-issue-body.md) is retained as provenance and is not an additional executor instruction. [Input provenance](inputs/provenance.json) binds those bytes and the source's applicable instructions and required readings.

This is the remaining A12 slice; A22 deadlines already exist at the pinned source. This is a new native trial from a reconstructed historical pre-fix revision, not an observed clean Claude-session replay. The public issue's edit history was not reconstructed, so the retained body is not claimed to be byte-identical to the original assignment. Qualification completed separately before this preview; these preparation measurements do not repeat or replace it.

## Single observation

| Phase | Process wall time | Retained result |
| --- | ---: | --- |
| Syntax index construction | 0.514385 s | 3,830,159 bytes; 485 indexed files |
| Deterministic preview | 0.264198 s | 16,278-byte briefing; 8 selected source units |

The preparer reports 0.199479 seconds inside the preview process, including 0.197670 seconds for source selection. The process measurement also includes interpreter startup and output writes. Index construction is measured separately; no operating-system cache flush was performed. Beta and Gamma ran sequentially after qualification released the Cargo slot and before the next diagnostic build started. Their historical trees are much smaller than the original reserve snapshots; these observations do not establish a general latency bound or executor improvement.

The [receipt](preview-receipt.json) records timings and identities. The exact [briefing](preview/briefing.md), [preparation record](preview/preparation.json), and [candidate pool](preview/candidates.json) are retained. All 14 candidate file hashes and exact line spans match the pinned source, and the briefing fits the 16,384-byte limit. The full syntax index and logs are retained privately; the receipt binds the index and retained archive by SHA-256.

## Retrieval limits

The frozen policy reads 24 ranked Rust files, retains 14 candidate units, and delivers eight. Only two delivered units are within `crates/jev/`; both are tests. Six come from other packages or the historical public audit. No unit from `crates/jev/src/` is present in the pool or delivered pack, although the source index contains `SystemOneResponse::decode`, `decode_answer`, `Client::system_one`, and `BlockingClient::system_one`. A semantic reranker cannot recover those implementations from this pool. One delivered snippet is an existing live-test example; it is source evidence and does not authorize running it. The common task still forbids provider calls and the live feature. These are retrieval limits, not measured candidate failures.

The policy and input were not changed after viewing this result, and preparation was not rerun. The frozen Rust-only policy also does not supply the required Markdown readings; they remain in the common task and are available in the source snapshot.

## Reproduce preparation

Use the binary and script hashes in the receipt, an existing repository containing the exact source commit, and a new output directory outside that repository. The following paths are caller-supplied placeholders:

```sh
briefing-lab index --repo "$REPO" \
  --rev c427943a5c84ba5938a3549f24b27de551812a37 \
  --syntax --output "$OUTPUT/index.json"
python3 bench/delegation-study/prepare.py --repo "$REPO" \
  --rev c427943a5c84ba5938a3549f24b27de551812a37 \
  --index "$OUTPUT/index.json" --issue "$INPUTS/issue.json" \
  --mode deterministic --output "$OUTPUT/preview"
```
