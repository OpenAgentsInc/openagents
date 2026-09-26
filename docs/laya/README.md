# Laya

For cross-project priorities and dependencies, see the [master roadmap](../roadmap.md).

**Status:** all three checkpoints are ported and serve `POST /v1/systemone`.
On the retained four-case fixture corpus per checkpoint, sequence encoding
reproduces the Python reference exactly, and the reported answer fields match
to its four-decimal precision. This establishes scoped port conformance, not
general model accuracy or parity on every possible request. The port lives in `crates/laya`; conformance
fixtures in `crates/laya/fixtures/`; the run record in
[`measurements/`](measurements/).

## What it is

A decision model: one state document plus a map of typed questions goes
in, one probability distribution per question comes out. It generates no
text. The architecture is a bidirectional encoder — ModernBERT-large or
mmBERT-base — with a small decision head: every option of a question gets
one `[MASK]` marker in a single sequence, the head scores each marker
position, and a softmax over a question's options is its answer. An act
head beside it emits `rl_agent.act_probability`, an escalation signal the
port reports verbatim and assigns no policy meaning.

Upstream is a Python library — `rl_agent_api.py` + `rl_common.py`, GitHub
[`NandhaKishorM/laya`](https://github.com/NandhaKishorM/laya), PyPI `laya`
— with no HTTP endpoint, so the serving contract here is the TypeSafe
`POST /v1/systemone` shape every local door answers. The port was
reimplemented from the reference source; it shares no code with upstream.

## Checkpoints

Three checkpoints ship under Apache-2.0 from
[`convaiinnovations/laya`](https://huggingface.co/convaiinnovations/laya),
pinned by SHA-256 in `crates/laya/fixtures/manifest-*.json`:

| Variant id | Checkpoint | Encoder | Params | `max_len` | `head_max_len` |
| --- | --- | --- | --- | --- | --- |
| `english` | repo root | ModernBERT-large | 421M | 512 | 192 |
| `multilingual` | `multilingual/` | mmBERT-base | 322M | 1024 | 256 |
| `typed-decisions` | `typed-decisions/` | ModernBERT-large | 421M | 1024 | 256 |

Two properties the upstream reading
([`../decision-models/others/2026-09-19-laya.md`](../decision-models/others/2026-09-19-laya.md))
measured and the port preserves:

- **English means Latin script.** The English tokenizer shreds non-Latin
  input while answering confidently; route on the checkpoint, not on the
  request's language guess.
- **`multilingual` ships no fitted temperatures.** Its probabilities are
  the head's raw softmax (temperature 1.0); the other two carry fitted
  per-bucket temperatures. Neither is local validation — trust a
  probability only after measuring it on our own data.

## Serving

```text
laya-serve --bundle-dir ~/work/laya-artifacts --port 8010
```

Bundle mode loads every checkpoint directory that carries
`rl_agent_config.json` + `model.safetensors` and names each variant after
its directory. A request picks a variant explicitly with its `model`
field; an absent field or a configured `--alias` resolves to the default
variant, and an unknown name is refused with `model_unavailable` and the
list of known ids. There is no automatic language router — choosing
English versus multilingual is the caller's decision, and the served
checkpoint identity is on the model card's `artifact_identity`.

```text
laya-serve --model-dir ~/work/laya-artifacts/english --port 8010 \
    --alias laya-latest
```

serves one checkpoint; `--alias` (repeatable) puts compatibility names on
the default variant only, and an alias that collides with a variant id is
refused at startup.

### Smoke test

```text
curl -s localhost:8010/v1/models | jq '.models[].id'

curl -s localhost:8010/v1/systemone -d '{
  "state": "I was charged twice for order 4815.",
  "model": "english",
  "questions": {
    "refund": {"type": "noul", "instructions": "Should a refund be issued?"},
    "topic":  {"type": "choice", "instructions": "What is this about?",
               "criteria": ["billing", "shipping", "other"]}
  }
}'
```

These loopback calls run against local weights and incur no inference API
charge. Hardware, energy, and operator costs remain separate; serving on a
public interface requires a separately configured admission layer.

## Contract notes

Answer semantics follow `rl_agent_api.py`, not kev's door:

- Probabilities, scores, and confidences round to four decimals with
  ties-to-even; `rl_agent.act_probability` is unrounded upstream and stays
  unrounded here.
- Confidence is `1 − normalized entropy` for every question type.
- The envelope's `model` field is the constant `rl-agent` — the reference
  hardcodes it for every checkpoint, including `laya-typed-decisions`.
  Checkpoint identity for routing lives on `/v1/models`, not in answers.
- A request field the model never reads — an unknown question field, a
  `noul` criteria key outside `true`/`false`, an unsupported `type` — is
  refused rather than silently dropped. Top-level request fields stay
  open: the jev client reserves `extra_body` for fields the API adds and
  this port does not model.

## Where things live

- [`conformance.md`](conformance.md) — the fixture protocol, what is
  pinned, the transformers-version caveat that decides RoPE resolution,
  and the observed deltas.
- [`measurements/`](measurements/) — the port baseline: conformance,
  load and per-request latency, and resident memory on a declared
  workload.
- `crates/laya/` — the crate. `fixtures/gen_fixtures.py` regenerates the
  goldens inside the reference's Python environment;
  `examples/measure.rs` reproduces the latency numbers;
  `examples/dump.rs` prints raw logits and encoder hidden states for
  parity diagnosis.
- [`../decision-models/others/laya-adapter-prerequisites.md`](../decision-models/others/laya-adapter-prerequisites.md)
  — licenses, artifact sources, deployment prerequisites, and the
  upstream cautions this port inherits.
