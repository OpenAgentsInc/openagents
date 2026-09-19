# Decision models on the mesh

**Status:** proposed. Nothing on this page is implemented. It lays out the
path to serving and training kev-class decision models across the earn
mesh — Psionic owns execution and artifacts, Pylon nodes own the hardware,
Coder's fleet coordinator routes and verifies, and the service exposes a
TypeSafe-compatible `POST /v1/systemone` that fans out over that network.
Each phase names the surfaces it lands in and the evidence that proves it.

## Why this workload fits the mesh

A decision request is the cheapest useful work the mesh could carry:

- **One prefill, no decode.** A request is a single forward pass over a
  packed sequence. There is no streaming, no KV residency across requests,
  no per-token metering loop — a dispatch is one shot in, one distribution
  out. The whole `kev-0.5b` artifact is 38 MB of adapter plus head on a
  ~1 GB base; `kev-4b` serves in bf16 on a 32 GB Mac in about a second.
- **Branches are independent by construction.** The block-causal mask
  makes each question an isolated branch off a shared state. Packed and
  separate evaluation agree to `4e-6` — measured, not assumed. That
  equivalence is what makes fan-out *correct*: a coordinator can split a
  100-question request across N workers, merge the per-question
  distributions, and the merged answer equals what one worker would have
  returned packed. Replication for assurance is the same mechanism with
  the same question sent twice.
- **The verification floor gets stronger, not weaker.** Text probes check
  that a generative worker holds the right weights by string-matching one
  completion. A decision worker can be probed with typed questions whose
  expected *distributions* are pinned — and the mechanism tests themselves
  (isolation, packing invariance, delimiter forgery) become admission
  checks. A worker that fails isolation is not just wrong, it is running a
  broken mask; the floor can detect a whole class of incorrect
  implementations that a text probe would miss.
- **The artifact story already exists.** Pylon manifests pin sha256 digests
  before cache placement; the fleet catalog pins `weight_hash` per row and
  the worker downloads from source directly. A decision model is a bundle —
  base, adapter, head, tokenizer — and the existing digest discipline
  extends to a manifest of several pinned files.

## Contract: the wire shape

The service door speaks TypeSafe's contract verbatim — the same contract
`crates/jev` implements today and the same one `kev/api.py` serves:

```jsonc
// POST /v1/systemone
{
  "state": "…",                            // string | object | array
  "model": "kev-4b",                       // a catalog row id
  "questions": {
    "<id>": {
      "type": "noul" | "choice" | "score",
      "instructions": "…",                 // string | object | array
      "criteria": { "…": "…" } | ["…", …]  // per type
    }
  }
}
```

```jsonc
// 200 response
{
  "model": "kev-4b",
  "answers": {
    "<id>": { "type": "choice", "choice": "returns",
              "probabilities": { "returns": 0.89, "…": 0.11 },
              "confidence": 0.83 }
  },
  "usage": { "input_tokens": 101, "output_tokens": 161 }
}
```

Two extensions, both opt-in and additive so an unmodified `jev` client still
works:

- `extensions.provenance: true` in the request asks the response to carry
  `provenance`: the artifact digests served, the backend family, the
  verification epoch the worker cleared, and whether the answer is
  replicated. Worker identity stays off the wire — the fleet status routes
  already keep node detail behind the operator boundary.
- `extensions.replicas: k` asks the coordinator to send the same request to
  `k` verified workers and compare distributions. The response adds
  `replicas: {count, max_abs_delta}` per question group; a delta above the
  model's tolerance marks the answer `disputed` the way the verification
  floor already marks probe failures.

Refusals are explicit and typed, matching the repo's refusal posture:
`model_unknown`, `unsupported_question_type`, `too_many_options` (>255),
`branch_too_long` (state+question over the row's branch cap),
`no_workers` (no verified worker offers the model), `unverified` (workers
exist but none has cleared the floor — refuse rather than silently serve
from unverified capacity).

## Phase 0 — conformance fixtures

Before any Rust code: generate the ground truth.

1. Fetch `jaredpalmer/kev-0.5b` from the Hub (adapter, head, tokenizer,
   `eval.json`) and the Qwen2.5-0.5B base; record every file's sha256.
2. Run `python -m kev.serve --run runs/kev` locally against a fixed corpus
   of requests — the TypeSafe docs examples, the mechanism probes
   (isolation, permutation, IIA, forgery), and a packed-vs-separate set —
   and capture exact inputs and probability outputs as fixtures.
3. Commit the fixtures (inputs, expected distributions, tolerance bands)
   under `fixtures/` in psionic. They are the acceptance test for every
   later phase: a Rust implementation is done when it reproduces these
   numbers inside tolerance, not when it compiles.

Tolerance matters because precision does: kev's own notes record that TF32
alone moves probabilities ~1e-3, enough to trip the isolation gate. The
fixture band must absorb backend numerics (fp32 vs bf16 vs TF32) — expect
~1e-2 absolute for bf16 workers, tighter for fp32 — and the catalog row
below carries the band per backend family.

## Phase 1 — the decision lane in psionic

A new model kind in `psionic-models` beside the generative lanes:

- `encode()`: state + question rendering (the `render()`/`option_text()`
  semantics from `kev/api.py` ported exactly — train and serve must share
  one renderer), delimiter assignment, `seg`/`pos`/`opt` records.
- `branch_mask()`: the additive block-causal mask; `option_isolation` as a
  model-manifest flag, not a request knob.
- `PointerHead`: two linear maps + scaled dot product; readout gathers
  `</opt>` and `<decide`> hidden states.
- Backbone: the existing Qwen2.5/Qwen3 lanes in `psionic-models` /
  `psionic-serve` supply the frozen transformer; `psionic-adapters` already
  owns LoRA loading — the work is applying an adapter plus a sidecar head
  and driving it prefill-only with a custom mask.
- Serving: `psionic-serve` gains `POST /v1/systemone` next to
  `/v1/chat/completions`, plus `GET /v1/models` reporting the decision
  row's `question_types`, `max_options`, `state_tokens`, `branch_tokens`.

Validation: the phase-0 fixtures run as a `cargo test -p` suite; the
isolation and packing probes run on every backend family the row admits.

## Phase 2 — artifact manifests

`openagents.psionic.model_artifact_manifest.v0.3` describes one GGUF file.
A decision model is a bundle; the schema gains a variant (or a sibling
schema `psionic.decision_model_artifact_manifest.v1`) with:

- `base`: a reference to the backbone's own artifact manifest (reuse the
  existing schema — the base keeps its license boundary and backend rows).
- `adapter`: `{url, sha256, sizeBytes, format: "safetensors", rank, alpha,
  targets}`.
- `head`: `{url, sha256, sizeBytes, format, pointerDim}`.
- `tokenizer`: `{url, sha256}` set.
- `decision`: `{questionTypes: ["noul","choice","score"], maxOptions: 255,
  stateTokens, branchTokens, calibration: {temperature, fittedOn},
  evalRef}` — the eval record the bundle was measured against.
- `admittedEndpoints: ["/v1/models", "/v1/systemone"]`,
  `inferenceOnly: true`, `trainingClaim: "blocked"` — same posture as the
  generative rows until a training lane actually ships.

Producer lives beside `pylon_release_manifest.rs`; fixtures under
`fixtures/pylon/psionic/` as today. The eval record (`evalRef`) is a
committed JSON: source suites, partitions, the accuracy/ECE/NLL table, the
mechanism-test results, the calibration temperature. A row without a
measured `evalRef` does not admit — that is the honest-claims boundary.

## Phase 3 — the earn node

An earn node's supervisor already does the whole lifecycle; the decision
row plugs into the same machinery in `coder`:

- `fleet/catalog.toml` gains rows of a new kind:

  ```toml
  [[decision_model]]
  id = "kev-4b"
  family = "kev"
  base = "qwen3-4b"              # its own artifact row or manifest ref
  format = "kev-bundle"          # adapter + head + tokenizer manifest
  artifact_bytes = 830000000     # adapter + head + tokenizer
  min_memory_bytes = 9663676416  # base + adapter residency + prefill workspace
  weight_hash = "…"              # digest of the bundle manifest, which pins each file
  backends = ["cuda", "metal"]
  question_types = ["noul", "choice", "score"]
  max_options = 255
  branch_tokens = 8192
  ```

- The supervisor's fetch step downloads the bundle's files and verifies
  each sha256 before offering the row — the same rule `weight_hash`
  already enforces, applied per file.
- `register` offers gain a capability field (or a `decision_models[]`
  offer parallel to `models[]`) carrying the row's bounds so the
  coordinator can refuse over-sized requests at admission rather than at
  dispatch.
- The worker engine is `psionic-serve` in a decision-worker mode; until
  that lands, an `ollama`-style adapter can front a local `kev.serve`
  process for development — the catalog's `[open]` mechanism admits it the
  same way it admits Ollama models today, with the blob digest standing in
  for `weight_hash`.

## Phase 4 — the fleet work shape and verification floor

`docs/fleet/protocol-v1.md`'s dispatch `work` field gains a `systemone`
shape carrying the request body unchanged — same pattern as `chat` /
`responses`, so the worker's own door code reads every field the client
sent. `finish` carries `answers` plus `usage`.

`fleet/probes.toml` gains a decision probe kind. A probe names a pinned
typed question set over a pinned state and the expected distribution per
question within the row's tolerance — a stronger check than string match:

```toml
[[decision_probe]]
model = "kev-4b"
backend = "*"
state = "The mesh verification probe."
questions = { ok = { type = "noul", instructions = "Reply affirmative." } }
expect = { ok = { noul_min = 0.9 } }
```

And the mechanism tests become scheduled floor checks, not just admission:

- **Isolation probe**: secret planted in a sibling question must stay at
  chance; moved to state must be found. A worker failing it is masking
  wrong — quarantine, same as a hash mismatch.
- **Packing probe**: same questions sent packed and separately must agree
  inside tolerance — proves the packed path the economics depend on.
- **Forgery probe**: option text containing delimiter lookalikes must not
  change the option count — proves input sanitization.

Receipts meter `input_tokens` (packed sequence length) and `output_tokens`
(the serialized-answer count, same accounting fiction Jev uses — metered
the same way so the pricing surface stays familiar). The existing
held-receipts-until-verified machinery applies unchanged.

## Phase 5 — the service door and fan-out

`bins/coder-serve` exposes `POST /v1/systemone` behind the same consumer
auth as the other `/v1` doors. Routing:

1. Resolve `model` to a catalog row → the set of live workers offering it
   that have cleared the floor.
2. Default dispatch: whole request to one worker (cheapest, matches kev's
   semantics).
3. `extensions.replicas: k`: same request to `k` workers; coordinator
   merges by comparing distributions per question and reports
   `max_abs_delta`. Divergence past tolerance disputes the workers' floor
   records — fan-out doubles as continuous verification.
4. Optional split dispatch for oversized question counts: questions
   partition across workers (each re-encodes the state — the cost is real
   but bounded by the branch cap), merge preserves question ids. This is
   the actual scale-out lever: request-wide limits (65k packed tokens)
   stop being a per-worker constraint.

The `jev` crate needs no changes — `base_url` points at the door and the
contract is identical. Its `Question::Raw` escape hatch already tolerates
fields this design adds later.

`GET /v1/models` reports decision rows with their `question_types` and
bounds so clients can discover capability, and `/v1/fleet/status` rows
show the decision probes' pass state the way `probes` does today.

## Phase 6 — training on Pylon

Serving first; training follows the same channels once serving is proven:

- **Recipe**: port `kev/train.py`'s objective into `psionic-train` — LoRA +
  pointer head, frozen base, cross-entropy over options, the augmentation
  and the optional `--perm_kl` / `--ord_w` terms. `psionic-adapters` owns
  the LoRA mechanics; the mask/packing code is shared with the serve path
  by construction.
- **Suites**: adopt kev's frozen-suite format outright —
  train/calibration/development/locked-test partitions, checksummed, pinned
  dataset and base revisions. Compatible data means our checkpoints compare
  against kev's leaderboard and, through `kev.jev`'s method, against Jev on
  shared items.
- **Dispatch**: `qwen_legal_pylon_dispatch.rs` is the pattern — signed
  Ed25519 job envelopes, worker-verified scheduler signatures, signed
  receipts, per-job digests and verification flags, payment decisions that
  stop at payable/withheld/deferred while Treasury owns the wallet. A
  `decision_train` job kind carries the suite manifest hash, the recipe
  config (bounded allowlist, as kev's `experiment.py` does), and the code
  digest the worker must match before starting.
- **Eval receipts**: a finished job emits the suite report — accuracy,
  ECE, NLL, temperature fit, mechanism tests, transfer partition — as a
  signed artifact the scheduler verifies against the job's declared gates.
- **Promotion**: a checkpoint becomes a catalog row only through the
  manifest path of phase 2 with its eval receipt attached. Kev's own
  release screen (held-out policy pairs ≥ 0.70) is a working model of the
  gate; the previews fail it and ship honestly labelled — do the same.

## Order of work

1. **Phase 0 fixtures** — hours, unblocks everything downstream.
2. **Phase 1 serve path** — the real port; proves correctness against
   fixtures locally on CPU/Metal.
3. **Phase 2 + 3** — manifest and catalog row; a single earn node offers
   `kev-4b` and clears probes on a loopback coordinator.
4. **Phase 4 + 5** — fleet shape, floor probes, service door; a `jev`
   client pointed at the door answers through the mesh.
5. **Phase 6** — training jobs dispatch to Pylon workers; first artifact
   is a re-implementation of the kev-0.5b recipe on our own rails, gated
   by the same screens.

Each phase lands behind the repo's existing refusal posture: no catalog
row without digests, no dispatch without verification, no paid inference
claim until receipts have settled through the real path, no model named in
interface copy.

## Open questions

- **Numerical tolerance on mixed hardware.** bf16 Metal and fp32 CUDA
  workers will disagree at the third decimal; replica deltas and probe
  bands need per-backend tolerances the catalog row carries. Kev's own
  TF32 finding (~1e-3 drift) is the calibration point.
- **Question-level fan-out economics.** Splitting a request across workers
  multiplies state re-encoding; the merge is exact but the billing is not —
  price the packed-token total, not per-branch.
- **Calibration as a service property.** Per-model temperature is an
  artifact property today; whether the door applies scaling, reports raw,
  or carries both is an API decision to make before `extensions` freeze.
- **Score confidence.** kev uses a stand-in formula; TypeSafe's is
  unpublished. Ship kev's documented formula and mark it in the card.
- **Training data governance.** Suites carry dataset licenses; a paid
  training lane needs a per-source license check in the suite manifest
  before dispatch, the way model rows carry `licenseBoundary`.

## What this page is not claiming

Kev is not Jev, and this plan does not produce Jev. It produces a serving
and training path for a measured, open, small decision-model family whose
best checkpoint trails the hosted model by ~8 points out of domain. The
value is the shape: typed judgments with trained probabilities, verifiable
end to end, on hardware the network already has. Calibration on any
workflow that matters is measured per deployment — the same rule the Jev
integration program already holds.
