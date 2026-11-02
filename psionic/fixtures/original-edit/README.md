# Chapter 255 original-model experiment artifacts

These are inert observations and bounded overlays from the 2026-11-02
experiment in [chapter 255](../../book/ch255-edit-an-original-language-model.md).
The [protocol](protocol.md) and [measurement clarification](measurement-notes.md) record the frozen conditions and pre-held-out
amendments. This is an exploratory working-tree protocol, not independent
preregistration.

Original: `openai/gpt-oss-20b@6cee5e81ee83917806bbde320786a8fb61efebee`,
Psionic `gpt-oss-metal-f32-ff-v1`, pinned original BF16/MXFP4 weights.
The original readout windows contain 11,520 bytes of weights from that
Apache-2.0 model. Its existing [license](../gptoss/original/notices/LICENSE)
and pinned-source documentation apply. The full original checkpoint is not
redistributed here. There is no Qwen3.5 generation or reproduction evidence.

- `original-row-{0,1}.bf16`: little-endian original `analysis`/`final`
  readout rows, token IDs 35644/17196, 2880 values each.
- `parity-{0,1}.f32`: 201088 little-endian F32 logits for each original
  reference prefix. The retained independent NumPy reference predates edits.
- `channel/`: 18 development channel-prefix activation vectors (2880 F32)
  and full-vocabulary logits. Only the six harder arithmetic cases derive
  the direction and the one-scalar fit.
- `answer/`: all 54 forced-final reference prefixes, activation vectors and
  logits. These support first-answer-token NLL, not sequence log loss.
- `candidates/`: encoded replacement BF16 rows plus metadata/digests. Float
  metadata uses big-endian IEEE binary64 bit hex. Three explicit carry
  receipts reuse identical baseline observations; these are not new trials.
- `runs/`: actual sequences, prefix length, generated IDs, raw decoded
  output, completion/parser/repetition diagnostics and per-generation time.
  Later records include explicit prompt IDs and the version-2 scorer. The
  initial screen retains its original version-1 boolean; all reported
  comparisons re-score raw finals with version 2, including valid H₂O.
- `llm/`: exact text requests, full provider responses/usage and client
  timing for three bounded text-only proposer conditions. All decline edits.
- `selection.json`: development-only chosen changed diagnostic candidate;
  `quality.json`: finite worksheet and chapter-241 verdict.
- `matrix.csv`, `supplement.csv`, `ledger.csv`, `results.coax`, `rows.coax`, `example.out`:
  derived offline reports and the small public example's exact transcript.
- `live/`: saved live-phase stdout, including authentication/startup time.
- `*.command.json`: bounded live-phase wall time, including authentication.
  Initial screen/stress total wall time was not captured; per-sequence time
  remains available. Build times and proposer accelerator time are separate.
- `checks/`: source-bound final mutation receipts and historical failures.
- `builds/`: observed build hashes and exact historical model-driver source
  snapshots. See its README for provenance and the initial-build limitation.

`python3 -I psionic/tests/original_edit_oracle.py` independently reconstructs
encoded edits, identities, scalar fitting, scores, selection, loss proxies
and accounting. It decodes saved outputs with the pinned original BPE ranks.
It performs no inference, training, external request or original-model CPU
run. The Coax report reads these same inert artifacts. Neither process grants
quality, deployment or live replacement authority.
