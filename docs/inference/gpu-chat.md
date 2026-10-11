# Connecting a GPU to a chat (spec, 2026-10-12)

How a person, or OpenAgents, attaches GPU capacity to a chat so that the
model answering the chat runs on that GPU: our GCE H100 capacity, a Modal
function, a Pylon someone else runs, or the GPU in the person's own
computer. The chat does not learn about GPUs; it learns about one more
**door** behind the gateway, and the person picks it where they pick a
model today.

Status: design. Everything it builds on exists and is named; the new pieces
are marked **new**. Sibling documents: [the gateway](gateway.md) (the
`Upstream` trait, routing, the rate card), [providers.md](providers.md)
(which door each call site uses now), [private inference](../security/private-inference.md)
(the `a3-highgpu-1g` H100 lane), [the GCE pool](../cloud/gce-pool.md)
(`openagents cloud up`, grants, route records), [Pylon](../compute/pylon.md)
and [Clef self-hosting](clef-self-host.md) (what fits on which GPU).

## 1. What the person sees

In any chat surface (Coder, the web chat, the phone), the model picker
(`/models` in Coder, "Where it runs" and the model menu on the web) gains
a **Your GPUs** group next to the hosted models. Each row is a GPU door
that is online now, with the model it serves and its price:

```text
Models
  openagents/auto            Vertex first, our credit         free allowance, then $…
  anthropic/claude-sonnet    …
Your GPUs
  gpu/h100-east5  Qwen3.8-27B    1×H100 80 GB, us-east5, spot   $6.40/h while up · warm
  gpu/modal-a100  Qwen3.8-27B    Modal A100 40 GB, scales to 0  $0.0011/s while running · cold (≈45 s)
  gpu/mac         clef-flash     This Mac, Metal                free · warm
Pylons
  studio-4080     qwen3.5-0.8b   someone's RTX 4080             12 sat/1k tokens
```

Picking a row pins the chat to that door. Everything else about the chat is
unchanged: the transcript, tools, sync, cost rows. The chat rail shows
`gpu/h100-east5 · warm` where it shows the model today, and `starting…`
with a countdown while a cold door boots.

Commands, all with JSON output (`--json`), none of them UI-only:

```sh
openagents gpu list                               # doors, state, model, price, who may use them
openagents gpu up h100-east5                      # start a stopped door (GCE) or warm it (Modal)
openagents gpu down h100-east5                    # stop it; the chat falls back to its next door
openagents gpu add modal --app oa-chat --model qwen3.8-27b --price-from-modal
openagents gpu add gce --machine a3-highgpu-1g --zone us-east5-a --spot --model qwen3.8-27b
openagents gpu add local                          # this computer's psionic-serve
openagents gpu share h100-east5 --with acct_…     # let another account's chats pick it
coder --model gpu/h100-east5                      # pin a Coder run to it
```

Nothing is bought, started, or shared without one of these commands or the
picker, and every one of them appends to `~/.openagents/gpu/runs.jsonl`
the way the GCE pool appends to `cloud/runs.jsonl`.

## 2. The one new concept: a GPU door

A **GPU door** is a gateway upstream (section 4 of [gateway.md](gateway.md))
whose compute we or the person control, that can be **up** or **down**,
costs money by the hour or second while up rather than per token, and
serves an open-weight model through `psionic-serve` (or an OpenAI-compatible
server for models Psionic does not run yet).

| Field | Meaning |
| --- | --- |
| `id` | `gpu/<name>`; what the picker and `--model` use |
| `kind` | `gce`, `modal`, `local`, or `pylon` (a Pylon is already a door; it is listed here only so the picker shows one list) |
| `model` | the model id the server loads (`qwen3.8-27b`, `clef-flash`, …) and its context limit |
| `owner` | the account that pays for it; the only one who can `up`, `down`, or `share` it |
| `shared_with` | accounts whose chats may pick it (default: owner only) |
| `state` | `down`, `starting` (with an estimate), `warm`, `draining`, `failed` (with the reason) |
| `price` | what it costs while up (`usd_micros_per_second`), from GCE pricing or Modal's rate; shown, and charged to the owner's balance, never per token |
| `privacy` | `strict` for `local`, `gce` (our project, no retention), and CC-mode H100s; `standard` for Modal unless its zero-retention terms are confirmed, like Z.ai and the Pro door today |
| `endpoint` | where the gateway reaches it: a loopback or tailnet URL for `local` and `gce`, Modal's web endpoint URL for `modal` |
| `grant` | for `gce`: the pool grant and epoch, as `chat work --on gce` records |

The record lives in the gateway's config next to `inference.pylons`
(**new** `inference.gpus`), and the owner's copy at `~/.openagents/gpu/`.
A door is an `Upstream` implementation (**new** `upstream/gpu.rs`) that
wraps the existing Psionic adapter (`upstream/psionic.rs`) or the
Chat-Completions adapter (`upstream/chat.rs`) and adds the state machine.

## 3. Routing

- **Pinned.** A chat that picked `gpu/<name>` sends
  `openagents.route.only: ["gpu/<name>"]`, the same field local Psionic
  uses today. The router sends there and nowhere else. If the door is
  `down` or `failed`, the request gets a typed `503 door_down` with the
  state and estimate, and the surface asks: start it (owner), wait, or use
  `openagents/auto` for this turn. It never silently switches doors,
  because the person picked a GPU on purpose (privacy or cost).
- **Auto.** `openagents/auto` may add a `warm` GPU door the caller owns or
  is shared to its candidate list when the task class fits the model (text
  only, no tools, under its context limit) and the privacy level allows
  it. It never starts a door: a cold start costs money and 45 s to a few
  minutes, so only the person starts one.
- **Fallback before the first token** works as for every upstream: a door
  that fails mid-request is marked `failed`, the router falls back within
  the chat's allowed doors, and the owner sees the reason in `gpu list`.

## 4. The three kinds

### 4.1 `gce`: our H100 capacity

Reuses the GCE pool. `gpu add gce` is `cloud up` with a GPU machine type
and a different image: [`oa-coder-host`](../deployment/coder-host-image.md)
plus the CUDA 13 libraries and `psionic-serve` that
`scripts/pylon-psionic.sh setup` installs, baked as **new**
`oa-gpu-host`. The host runs `psionic-serve` on loopback and the Pylon
provider's health loop (`draining` when the model stops answering), and
reaches the gateway over the project VPC, so no model port is public.

| Choice | Default | Why |
| --- | --- | --- |
| Machine | `a3-highgpu-1g` (1×H100 80 GB) spot | The one GPU type we have run (private-inference.md); about $6.40/h spot in us-east5 |
| Zones | `us-east5-a`, `us-central1-a`, `europe-west4-c` | Where `a3-highgpu-1g` exists |
| Model | `qwen3.8-27b` BF16 | Fits in 80 GB with room for context; `clef` (27B) for decision-only doors |
| Idle | stop after 10 minutes with no request (`idle_minutes`, like the pool) | A chat that goes quiet should not run a $6/h machine overnight |
| Spot preemption | state `failed: preempted`, auto-restart once if the owner asked (`--restart`) | Spot is the only way to get these at this price |
| Confidential (CC mode) | off by default; `--confidential` picks the `tee-cloud` image from private-inference.md | The sealed lane is a separate, slower boot; vault chats pick it on purpose |

Cold start measured budget: 3–5 minutes (VM boot, driver, 54 GB of weights
from a regional GCS bucket). The door shows `starting (≈4 min)` from the
first measurement and refines from `runs.jsonl`.

### 4.2 `modal`: scale-to-zero GPUs

Modal runs a Python function on a GPU and gives it an HTTPS web endpoint.
We keep **one** small Modal app (Python is an infrastructure exception,
like the training tooling): a class with a `@modal.enter` that loads the
model into a Modal Volume-backed cache, an OpenAI-compatible
`/v1/chat/completions` served by vLLM (Psionic has no Modal build yet; when
it does, the same door switches to the Psionic adapter), `keep_warm` 0, and
a `scaledown_window` matching our idle minutes. The token the gateway sends
is a Modal proxy-auth token held in Secret Manager; the door is reachable
by nothing else.

`gpu add modal --app oa-chat --gpu A100-40GB --model qwen3.8-27b` deploys
that app with the chosen GPU and model (`modal deploy`, run from the
operator's computer, which only orchestrates), records the endpoint URL,
and reads the price from Modal's published per-second rate for that GPU.
`gpu up` sends one warm-up request; `gpu down` sets `keep_warm` 0 and lets
it scale down. State comes from Modal's container count for the app.

Why Modal next to GCE: no VM to keep, a cold start of 30–60 s against our
3–5 minutes, per-second billing, and GPUs we have no quota for (H200,
B200). Why not only Modal: `standard` privacy until its terms are
confirmed, no CC mode, and our Google credit does not pay for it.

### 4.3 `local`: the GPU in the person's computer

Already built as the `local` upstream: `psionic-serve` on the person's
machine, `route.only: ["local"]`. This spec only lists it in the same
picker as `gpu/mac` (or `gpu/coderos-4080`) and lets `gpu add local`
install the `psionic-serve` user unit from `scripts/pylon-psionic.sh`,
so the three kinds look the same to the person. A local door is free,
`strict`, and never leaves the machine; a chat synced to the account still
syncs its transcript, not the model traffic.

## 5. Money

- A `gce` or `modal` door charges its owner's OpenAgents balance per
  second while `starting` or `warm`, settled every minute by the gateway's
  meter (`inference::meter`) into the same ledger as token spend, resource
  **new** `openagents.inference.gpu.v1`, so the usage summary shows "GPU
  time" next to "model cost". Tokens through the door cost nothing extra.
- The owner's balance must cover at least 10 minutes before `gpu up`
  succeeds; at zero the door is `down`-ed and the chat told why.
- A shared door bills its owner only. Charging the guests is a later
  slice; the ledger split already exists for Pylons if we want it.
- Our Google credit pays for `gce` doors on the `openagentsgemini` project
  when the owner is OpenAgents; a person's own door runs in their own GCP
  project with their own credential (`gpu add gce --project`).

## 6. What is new, in order

| Slice | Code | Check |
| --- | --- | --- |
| S1 `inference.gpus` records, `upstream/gpu.rs` state machine over the Psionic and Chat adapters, `route.only: ["gpu/<name>"]`, typed `503 door_down` | `crates/inference`, `crates/gateway` | gateway test: a pinned chat answered by a stub door; a `down` door returns `door_down`; auto never starts one |
| S2 `openagents gpu list/up/down/add local/share` with `--json`, `~/.openagents/gpu/runs.jsonl` | `crates/openagents-cli` | CLI tests over a fake gateway |
| S3 Picker group **Your GPUs** in Coder `/models` and the web model menu; rail shows state | `crates/coder-new` (`models.rs`, `model_catalog.rs`), `crates/openagents-web` | existing picker tests plus one per state |
| S4 `gce` doors: `oa-gpu-host` image, `gpu add gce`, idle stop, preemption | `crates/openagents-cli/src/cloud.rs`, `deploy/` | pool tests with a fake `gcloud`; one real spot H100 boot recorded in `runs.jsonl` (owner, costs about $1) |
| S5 `modal` doors: the app, `gpu add modal`, state from Modal | `infra/modal/` (**new**), CLI | deploy and warm once on a Modal account the owner creates (owner step) |
| S6 GPU-time metering and the usage row | `crates/inference/src/meter`, `crates/gateway` | ledger test: 90 s warm bills 90 s at the rate |

Each slice is one issue and one PR; S1–S3 need no GPU and no owner step.

## 7. Owner steps

- Choose the first real door: the existing `a3-highgpu-1g` spot lane in
  us-east5 (no new account) or a Modal account (new account, card).
- Confirm Modal's data terms before any Modal door serves a `strict`
  chat.
- Decide whether guests of a shared door pay (section 5), or stay free on
  the owner's balance.

## 8. Not in this spec

Training or fine-tuning on these GPUs; selling our doors to other people
(that is Pylon, which already has beacons, receipts, and payouts);
multi-GPU tensor parallel (one model per door); image, audio, or video
models.
