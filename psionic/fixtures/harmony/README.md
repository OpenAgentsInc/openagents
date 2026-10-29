# Harmony text fixture provenance

These are DATA and owned test fixtures for `gpt-oss-harmony-text-v1`.
Nothing here installs or executes the upstream repositories.

- `o200k_base.tiktoken.gz` losslessly retains the original rank bytes from
  `https://openaipublic.blob.core.windows.net/encodings/o200k_base.tiktoken`.
  SHA-256 of the decompressed original:
  `446a9538cb6c348e3516120d7c08b09f57c36495e2acfffe59a5bf8b0cfb1a2d`.
  That identity is pinned in Harmony's `public_encodings.rs` at revision
  `abd677f7ac962629c808197caa1feb9e3e95d2b0`.
- `gptoss.hst` is an owned deterministic HST7 index over all 199,998 original
  ranks, every valid concatenated rank pair, byte-to-rank IDs, a sorted lookup
  index and Unicode-17 property ranges. No NFC tables or special-literal
  scanning are used. `manifest.json` pins every derived artifact and records
  the pattern, source revisions, counts and original hash. `pins.coax`
  supplies the runtime admission hash.
- `toy.hst`/`toy.coax` contain all 256 bytes and ten invented merged tokens,
  with the same Unicode property ranges. They make compiler checks and the
  runnable example small. Their ordinary IDs are not model input IDs.
- `goldens.coax`, `messages.coax` and `goldens.json` come from the owned
  standard-library oracle. It uses explicit Unicode regex classes and a heap
  over concatenated original byte strings, without reading the candidate's
  packed merge-pair index or outputs.
- `upstream-low-effort.txt` is the unchanged pinned Harmony fixture
  `test-data/test_simple_convo_low_effort.txt`. The oracle independently
  constructs its text and requires exact equality. This retains upstream
  format evidence without invoking its renderer.
- `notices/` retains the gpt-oss and Harmony Apache-2.0 notices from their
  pinned local revisions, and tiktoken's MIT notice, retrieved from
  `https://raw.githubusercontent.com/openai/tiktoken/main/LICENSE` on
  2026-10-29. The vocabulary URL/hash above identifies the exact rank data.

The gpt-oss tokenizer source at
`5681a6b6552635e7b29cde94a61ca6a7aee90ebd` defines special IDs 199,998–201,087,
for 201,088 total entries. Generic Harmony at the pinned revision additionally
registers ID 201,088; this model-specific profile rejects that extra ID.

Reproduce and check drift from the repository root:

```shell
python3 -I psionic/tests/harmony_fixtures.py
python3 -I psionic/tests/harmony_oracle.py
psionic/tests/check_harmony.sh
```

The first two tools require Python with Unicode 17.0.0 (tested 3.15.1).
They read the retained original data; no network, upstream runtime, service or
checkpoint weights are required. `--write` is an explicit fixture-authoring
operation and is never used by the verification gate.
