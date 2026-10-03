# Beta deterministic preparation preview

This is one preparation preview for qualified public issue [#9425](https://github.com/OpenAgentsInc/openagents/issues/9425) at source commit `7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6`. It uses the frozen `syntax-units-v2` preparer and existing frozen `briefing-lab` binary. No model or Cargo command ran, and no executor outcome was measured. Keep this observation separate from the original 20-preview series and the Alpha preview.

## Inputs and provenance

The [safe task manifest](inputs/public-task-manifest.json) supplies the exact normalized title and task body. The [base prompt](inputs/base-prompt.txt) keeps the original common operational envelope, with the declared edit roots and named package checks. The [normalized issue input](inputs/issue.json) is the only task input passed to the preparer. The [retrieved public issue body](inputs/original-issue-body.md) is retained as provenance and is not an additional executor instruction. [Input provenance](inputs/provenance.json) binds those bytes and the source's applicable instructions and required readings.

This is the complete A13 task, including the strict CoderBench consumer. This is a new native trial from a reconstructed historical pre-fix revision, not an observed clean Claude-session replay. The public issue's edit history was not reconstructed, so the retained body is not claimed to be byte-identical to the original assignment. Qualification completed separately before this preview; these preparation measurements do not repeat or replace it.

## Single observation

| Phase | Process wall time | Retained result |
| --- | ---: | --- |
| Syntax index construction | 0.564690 s | 3,845,754 bytes; 486 indexed files |
| Deterministic preview | 0.264200 s | 16,239-byte briefing; 9 selected source units |

The preparer reports 0.208595 seconds inside the preview process, including 0.206773 seconds for source selection. The process measurement also includes interpreter startup and output writes. Index construction is measured separately; no operating-system cache flush was performed. Beta and Gamma ran sequentially after qualification released the Cargo slot and before the next diagnostic build started. Their historical trees are much smaller than the original reserve snapshots; these observations do not establish a general latency bound or executor improvement.

The [receipt](preview-receipt.json) records timings and identities. The exact [briefing](preview/briefing.md), [preparation record](preview/preparation.json), and [candidate pool](preview/candidates.json) are retained. All 16 candidate file hashes and exact line spans match the pinned source, and the briefing fits the 16,384-byte limit. The full syntax index and logs are retained privately; the receipt binds the index and retained archive by SHA-256.

## Retrieval limits

The frozen policy reads 24 ranked Rust files, retains 16 candidate units, and delivers nine. Five delivered units are within the allowed `atif` and `coderbench` edit roots; four are outside them. It includes an existing torn-trace consumer test and the ATIF half-written-line test. However, `atif`'s `read`, `Log::append`, and `Log::finish` are absent from both the candidate pool and the briefing, even though their declarations exist in the source index. The pool also contains `EXIT_UNVERIFIABLE`, which does not fit in the delivered pack. A semantic reranker can change selection only within this retained pool; it cannot restore the absent reader and writer implementations. These are retrieval limits, not measured candidate failures.

The policy and input were not changed after viewing this result, and preparation was not rerun. The frozen Rust-only policy also does not supply the required Markdown readings; they remain in the common task and are available in the source snapshot.

## Reproduce preparation

Use the binary and script hashes in the receipt, an existing repository containing the exact source commit, and a new output directory outside that repository. The following paths are caller-supplied placeholders:

```sh
briefing-lab index --repo "$REPO" \
  --rev 7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6 \
  --syntax --output "$OUTPUT/index.json"
python3 bench/delegation-study/prepare.py --repo "$REPO" \
  --rev 7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6 \
  --index "$OUTPUT/index.json" --issue "$INPUTS/issue.json" \
  --mode deterministic --output "$OUTPUT/preview"
```
