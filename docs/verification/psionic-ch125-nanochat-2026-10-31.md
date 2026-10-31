# Chapter 125: the pinned nanochat GPT and its inert checkpoint export, 2026-10-31

Task #9b3d. CPU only, binary64, Apple clang arm64, Coax 0.43.0. No GPU.

Reference: karpathy/nanochat `92d63d4e8bb4df75c3b71618f31ddde2378b2bcd`,
`nanochat/gpt.py` (sha256 8cdedc2e...04e3), `nanochat/checkpoint_manager.py`
(sha256 aa63399d...f3ba) and the pinned `flash_attention.py` (sha256
a10b0b10...2c2f, 7146 bytes). Read only, never executed. The pin's five
`planned:nanochat-forward` operations now name the unit `nanochat/gpt`
(`psionic/fixtures/models/pins/nanochat.json`, its digest in
`psionic/lib/model-pins.coax` refreshed; that check passes with 162 of 162
mutants killed).

- `psionic/lib/nanochat/gpt.coax`: the full and cached forward passes of
  gpt.py: RMS norm with eps 2^-52, smear, resid/x0 scalars, grouped
  attention with value embeddings and their 3*sigmoid gate, half-split RoPE
  (base 100000, rotated by -theta), QK norm and 1.2 scale, S/L sliding
  windows (short = ceil(seq/4/128)*128; the "2048->768" comment in gpt.py is
  stale, the code gives 512), ReLU-squared MLP, backout at layer L//2, final
  norm, untied head over the padded vocabulary, softcap 15.
- `psionic/tests/nanochat_oracle.py`: an independent pure-Python binary64
  oracle written from the reference. Losses: tiny 5.853663622595427 (15
  tensors, 10228 values), window 4.727822180934721 (3 layers, `SL`, 140
  tokens; 29 tensors, 23672 values; the window bites at position 129 only),
  checkpoint 7.379816235132483 (seq 64, vocab 329 padded to 384, `SSSL`;
  21 tensors, 53290 values). Its cached path equals its full path exactly.
- `psionic/lib/nanochat/gpt-checks.coax`, 6 checks, debug and release:
  every logit and block output at every position within
  `|a-b| <= 1e-12 (1+|b|)` of the oracle (largest observed 2.46e-14), token
  by token decoding equal to the full pass bit for bit, every enabled
  feature observable (all-long pattern over 140 positions; 11 perturbations:
  resid, x0, smear_lambda, backout, smear gate, ve gate, value embedding,
  wte, query, head, MLP), and refusals that leave outputs untouched.
- `psionic/tools/export/nanochat_export.py`: reads model_<step>.pt as a zip
  with a restricted unpickler (four globals; only tuple descriptions built),
  little-endian, contiguous and in-bounds F32/BF16 storages, a uniform
  `_orig_mod.` prefix stripped, and writes model.safetensors plus
  export.json (`psionic.nanochat-export.v1`: source, config as written,
  padding, tokenizer, checkpoint and safetensors digests).
  `psionic/tests/nanochat_export_test.py`: the committed `torch.save`
  checkpoint (`fixtures/nanochat/checkpoint/model_000000.pt`, 183467 bytes,
  sha256 192ded24...21c6; written once by torch 2.14.1 CPU in an isolated
  venv with `python -I`, no GPU) re-exports byte for byte to
  `fixtures/nanochat/export/model.safetensors` (178160 bytes, sha256
  3d15476e...d53e) and `export.json`; all 21 tensors carry the oracle's bits;
  8 crafted archives refuse and write nothing.
- `psionic/lib/nanochat/checkpoint.coax` (`#authority`): identity parsed
  against fixed key tables (unknown or repeated keys refuse), pinned source,
  the seven GPTConfig keys, the historical defaults (missing window_pattern,
  missing resid/x0 lambdas) refused as `historical`, tokenizer vocabulary
  equal to the model's, digest, safetensors names/rank/shape against the
  layout, finite values within 2^20, all or nothing. 107 checks pass in
  debug and release. `coax mutate --root-only`: 390 mutants, 390 killed, 0
  survived, no exemptions.
- Native (`psionic/tests/nanochat_native.c`): the generated C library at
  -O0 and -O3, each plain and with ASan/UBSan, loads the committed export
  from disk through `nc_load`, matches the checkpoint, tiny and window cases
  within 2.46e-14 and decodes equal to the full pass; a flipped payload byte
  refuses as `digest`, `historical.json` as `historical`, aliasing as
  `alias`; refusals leave parameters untouched.

```shell
psionic/check.sh --only nanochat
```
