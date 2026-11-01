# Chapter 126: nanochat text to text, 2026-11-01

CPU only, binary64, Linux x86-64, gcc, cached Coax compiler `coax0 0.46.0`
(`~/.cache/coax-release/8925679b…3028`). No GPU.

Reference: karpathy/nanochat `92d63d4e8bb4df75c3b71618f31ddde2378b2bcd`,
`nanochat/engine.py` and `nanochat/tokenizer.py` (`render_for_completion`),
read only, never executed.

- `psionic/lib/nanochat/engine.coax`: `ne_prompt` (render for completion on
  chapter 123's `nc_render`), `ne_generate` (chapter 125 prefill and cached
  decode, chapter 53's `nx_step` over binary32 logits, one counter per pick),
  `ne_after` (stops: `assistant_end`, `bos`, `tool`, `limit`, `context`, in
  that order), `ne_text` (table bytes and special names), and
  `ne_checkpoint_weights` (chapter 125's formula with the export's F32 and
  round-to-nearest-even BF16 storage).
- `psionic/tests/nanochat_engine_oracle.py` (Python with Unicode 17.0.0):
  composes the chapter 123, 125 and 53 oracles and runs every position one
  token at a time. Five conversations: 15+12 `limit`, 15+12 `limit`, 23+12
  `limit`, 34+31 `context`, seed 5 6+21 `assistant_end`.
- `psionic/lib/nanochat/engine-checks.coax`, 6 checks, debug and release:
  prompt ids, reply ids, stop reasons and decoded bytes equal to the oracle;
  the first prompt's 329 last logits within `|a-b| <= 1e-12 (1+|b|)`; every
  97th stand-in weight exact.
- `psionic/lib/nanochat/engine.coax`, 6 own checks (85 with its closure),
  debug and release: full-pass replay of every pick for eight seeds reaching
  all five stops, refusals before any write, prompt bounds (2,048 accepted,
  2,049 refused), stop order, decoding.
- `coax mutate psionic/lib/nanochat/engine.coax --root-only --jobs 12`:
  186 mutants, 186 killed, 0 survived.
- `psionic/examples/nanochat-reply.coax`: transcript
  `psionic/fixtures/nanochat-reply.out` equal in debug and release.
- `psionic/tests/check_nanochat_engine.sh` (without `--mutate`): exit 0.

Shared changes: `nanochat-chat.coax`'s `NcStatus`/`NcResult` are renamed
`NcChatStatus`/`NcChatResult` (with its goldens, C harness, oracle and
chapter 123), and `gpt.coax` and `nanochat-chat.coax` claim modules, so one
program can load both. `psionic/tests/check_nanochat_chat.sh` passes (66
checks debug and release, native plain and ASan/UBSan).
`psionic/tests/check_nanochat.sh` stops at its first step on this host
before and after this change: `nanochat_oracle.py --coax` differs from the
committed `oracle.coax` in the last digits (this host's libm); its Coax
checks (`checkpoint.coax` 105 of 105, `gpt-checks.coax`) pass.

Remaining: a trained upstream checkpoint with its 32,768-token tokenizer and
upstream-recorded ids and greedy replies; seeded parity with
`torch.multinomial`; `engine.py`'s calculator tool (replies stop as `tool`).
