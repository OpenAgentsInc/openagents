# Gemini Decision 1 and our Decisions API

GitHub #13469. Google's `gemini-decision-1` answers typed questions with
calibrated probabilities and no output tokens, as our System One models do.
This note covers calling it from Coax, how its shapes map onto ours, what it
costs, what you can't send, the door order on `/api/v1/systemone`, where it
could replace a model-with-schema call, and the results of the live runs.

## Call it from Coax

The codec is `coax/lib/systemone/google.coax`, and the transport is
`coax/lib/systemone/gemini.coax`. The key comes from `GEMINI_API_KEY`, or from
a value you pass in, and reaches curl's environment and the `x-goog-api-key`
header only. It never appears in an argument.

Google's own shape, with every input kind (text, image, audio, video, PDF):

```
#load "coax/lib/systemone/gemini.coax";   // link coax/platform/process-native.c and store-file-native.c

b: [1] GdBody; buf: [8192] u8; out: [16384] u8; err: [2048] u8;
ab: [2048] u8; ar: [1] ClArena; ar[0].buf = ab[..];
gd_begin(b[..], buf[..], GD_MODEL);
gd_text(b[..], "The blue mug I ordered on Monday arrived cracked.");
gd_media(b[..], "image/jpeg", photo_bytes, "low");
gd_noul(b[..], "damaged", "Did the item arrive damaged?", "", "");
gd_choice(b[..], "queue", "Which support queue should handle this?", keys[..], labels[..]);
gd_score(b[..], "urgency", "How urgent is it?", levels[..]);
r := gd_decide(so_gemini(""), b[..], out[..], err[..], ar[..]);
// r.error.kind: none, invalid_request, safety, not_found, key, rate, server, transport, malformed
a := gd_choice_of(ar[..], r.body, "queue");   // a.choice, a.probability, a.confidence
```

Our System One shape, so a caller of `so_decide` switches doors without
changing how it reads answers:

```
r := so_decide_gemini(so_gemini(""), b[..], out[..], err[..], scratch[..]);   // b is an SoBody
e := so_choice_of(ar[..], r.body, "engine");
```

The builder refuses before anything is sent what Google would refuse:
1 to 1,000 questions, question keys of 1 to 64 `[A-Za-z0-9_-]` characters,
2 to 255 choice options of 1 to 128 characters and at most 512 bytes with no
outer spaces, control characters or non-ASCII spaces, 2 to 255 non-empty
score levels, an estimate of 262,144 tokens across input and questions,
100 MB of inline media (50 MB for a PDF), the MIME allowlist per block type,
and `ultra_high` resolution on images only.

Errors come back typed. A 400 `invalid_request` carries the rule broken (for
example `* CreateDecisionRequest.questions[k].choice.criteria: must have
between 2 and 255 options`). A safety block names its code (`safety`,
`prohibited_content`, `blocklist`, `image_safety`, `content_blocked`). A
refused key is Google's RPC shape with reason `API_KEY_INVALID`.

To check it yourself:

```
coax/bin/coax test --library coax/lib/systemone/google.coax   # golden bytes, every limit, every error kind
coax/lib/systemone/gemini-smoke.sh                            # the client against a stand-in, no network
coax/lib/systemone/gemini-smoke.sh --live                     # Google: mixed questions, the ticket, an image
```

## Mapping and what is lost

`gd_from_systemone` turns our request into Google's, and `gd_to_systemone`
turns Google's answer into ours.

| Ours | Google's | What changes |
| --- | --- | --- |
| `model` | always `gemini-decision-1` | The answer keeps the model you asked for on the server route, or `gemini-decision-1` from the client. |
| `state` as a string | one text block | Nothing. |
| `state` as `{"messages":[...]}` | one `ROLE: ` text block per message; `image_url` data URLs and HTTPS URLs become image blocks | Message boundaries become text. |
| `state` as other JSON | one text block of the JSON | Nothing. |
| noul | noul | Google's answer stays within 0.01 to 0.99. Ours can reach 0 and 1. |
| choice | choice | A choice needs 2 options here, not 1. Google sorts option keys by UTF-8 bytes. Its `confidence` is the top probability, and ours is a concentration measure; the value passes through as Google sent it. |
| score | score | The same: the probability-weighted mean of level indices. Google sends level keys in any order, and the mapping writes them `"0"` to `"n-1"` because our readers take them by position. |
| `null` criterion label | `""` | Nothing. |
| usage | `total_input_tokens`, per-modality counts | Ours is `input_tokens` and `output_tokens: 0`. The per-modality split is only in Google's shape (`gd_usage`). |

The mapped answer names its door: `"service":{"door":"gemini","identity":"gemini-decision-1","provider":"google"}`.

## Price and rate limits

As published on 2026-11-13:

- **Paid tier:** $0.10 per million input tokens for text, image, video, audio
  and PDF alike. There are no output tokens and no context caching.
- **Free tier:** free of charge.
- **Batch:** not supported.

| Tier | Requests a minute | Tokens a minute | Requests a day |
| --- | --- | --- | --- |
| Free | 200 | 50,000 | 10,000 |
| Tier 1 | 1,500 | 1,000,000 | 50,000 |
| Tier 2 | 5,000 | 4,000,000 | unlimited |
| Tier 3 | 15,000 | 10,000,000 | unlimited |

Coder's engine route is about 520 input tokens, so a route costs about
$0.00005.

## What you can't send

- Context caching, batch, flex or priority service.
- `gs://` and YouTube URIs. External media URLs must be HTTPS and at most
  15 MB.
- Conversation turns, thought blocks or tool blocks. Input is a flat list of
  content blocks, so the mapping writes our messages as text.
- Empty text blocks.
- Anything over 16 KiB on the public `/api/v1/systemone` route. That route
  takes the body limit it already had, so in practice it carries text and
  small images only. Larger media goes through the Coax client.

## The door order

On 2026-11-13 the owner made Gemini Decision 1 the primary decision model
for everything (#13473). Every caller that asks a decision model now tries
Gemini Decision 1 first, and the model it used before is the fallback:

| Caller | First | Fallback |
| --- | --- | --- |
| `/api/v1/systemone`, `"model":"auto"` | `gemini-decision-1` | `elephant-1` (`OA_DECISION_ORDER`) |
| Coder's engine route (`coax/lib/agent/engine/route.coax`) | `gemini-decision-1` with a Gemini key | `elephant-1` at the router.json URL, else `mammoth-1` |
| `coder decide` and the router's checks (`coder/tui/route.coax`, `router.coax`) | `gemini-decision-1` with a Gemini key | `mammoth-1` |
| The desktop router (`apps/openagents-desktop/router.coax`) | `gemini-decision-1` with `GEMINI_API_KEY` | `mammoth-1` |
| Everglade's guards (`everglade/npc/guard-*.coax`), through `/api/v1/systemone` with `"model":"auto"` | `gemini-decision-1` | `elephant-1`, the pinned checkpoint |

The fallback answers when there is no Gemini key, and when Google fails for
any reason other than a rule of the request or a safety block
(`so_gemini_final`). For example, it answers on a refused key, a rate limit,
a missing model, a server error or no response.

These callers still name another model first, on purpose:

- **The Lab** (`coax/lib/agent/engine/lab.coax`, `gpu.coax`) exists to
  exercise our own Elephant on this Mac's GPU.
- **Psionic and Pylon** (`psionic/`, `apps/openagents-cli/pylon.coax`) serve
  and measure Elephant and Mammoth. They are the models, not callers of a
  decision door.
- **The legacy captain** (`everglade/legacy/v1/captain.coax`) is frozen with
  the rest of v1.

### Everglade's guards

Since 2026-11-13 (#13476) both guard consumers, the ambient guard
(`guard-decision.coax`) and Directions (`guard-guide.coax`), send
`"model":"auto"`. The server answers from the first door in its order, and the
guard accepts the answer by the identity it names. Two identities pass
(`everglade/npc/guard-door.coax`):

| Door | `model` | `service.identity` | `service.provider` | `service.door` | Numbers |
| --- | --- | --- | --- | --- | --- |
| Gemini Decision 1 | `gemini-decision-1` | `gemini-decision-1` | `google` | `gemini` | Doubles in full, up to 32 bytes, with an optional `e-N` exponent |
| Elephant | `elephant-1` | `elephant-1@sha256:682f...d0ba`, the pinned checkpoint | 32-byte hex | `pylon:...`, with `service.version` `psionic-pylon-v1` | At most nine decimals |

Everything else is refused: an unknown identity, Elephant with another digest,
and one door's identity under the other door's model.

The door that answered stays with the choice:

- **Replay.** The decision ticket holds the choice and the door as one value.
  Elephant's value is the choice itself, so earlier records still replay, and
  Gemini's values follow. A record made from a Gemini answer replays as
  Gemini's.
- **Evaluation.** A scored row keeps its door, and one set of totals scores
  one door. A summary names the identity that answered and refuses rows from
  two doors. A row with no contract (a 503, a malformed body) has no door.
- **Probes.** Probe 17 (ambient) and 219 (Directions) report the door of the
  last accepted choice: 0 none, 1 Elephant, 2 Gemini Decision 1.

`everglade/npc/guard-door-server-checks.coax` runs the guards' requests through
the server's own order, rename and Gemini mapping
(`mesh-policy.coax`, `mesh-gemini.coax`) and holds the result to the answer
bytes the guard checks use. No check calls a model.

The Directions evaluation's frozen requests changed with the model field, so
they have a new version, `psionic/fixtures/guard-guide-requests-v2/`. Version 1
stays as the request set of the dated Elephant records.

Not verified: no guard request was sent to production. The recorded Google
answer (`GD_FX_TICKET_ANSWER`) writes its probabilities as plain decimals.
The exponent form is read because Google may write a small double that way,
but no recorded answer shows it.

### /api/v1/systemone

`coax/web/api/mesh-policy.coax` reads routes from `OA_MESH_ROUTES`. A row of
`{"backend":"gemini"}`, with an optional `"url"` for a stand-in, makes a
Gemini door. `mesh.c` sends the deployment's key from
`OA_GEMINI_DECISIONS_KEY`. The caller's credentials never reach Google.

A request for a named model goes to that model's door only, as before.

A request for `"model":"auto"` uses `OA_DECISION_ORDER`, a comma list of up to
eight route names:

- The first configured door answers, and the answer names it.
- A 502, 503 or 504 hands the same body, renamed, to the next door.
- A 400 (a rule of the request) or a 422 (a safety block) is final, with
  Google's reason in our error body.
- A refused key, a missing model and a rate limit are 503s, so the next door
  is tried.

The standing rule is Clef first, with Jev as backup. Clef's GPUs are paused,
so nothing answers on the Clef routes today. The recommended setting is
`OA_DECISION_ORDER=gemini-decision-1,elephant-1`: Google first by the
provider rule, and Elephant behind it. Clef and Jev routes are not removed.
When Clef's GPUs come back, put its route name first in the setting. No code
change is needed.

## Coder's engine route

Coder's engine route (`coax/lib/agent/engine/route.coax`) asks three
questions about each new task: which engine, how hard, and how much effort.
Since 2026-11-13 it asks them in this order:

1. Gemini Decision 1, when a Gemini key is available (`GEMINI_API_KEY`, or
   `~/work/.secrets/openagents-gemini-provider.env`).
2. Elephant at the System One URL in `~/.openagents/coder/router.json` or
   `PSIONIC_SYSTEMONE_URL`.
3. mammoth-1 on Baseten.

When a later door answers, the route line says why. For example:
`Engine: codex (0.96) · easy · effort low, chosen by System One (no Gemini
key: set GEMINI_API_KEY or add it to ~/work/.secrets/openagents-gemini-provider.env; mammoth-1 answers)`.
Gemini takes the task off the machine, so the consent ledger decides first.
When the ledger has no grant and a local Elephant is configured, Elephant
answers, and the note gives the command that grants consent.

To use one door only, set `"door"` in router.json, or `CODER_ROUTE_DOOR`,
which wins, to `gemini-decision-1`, `elephant-1` or `mammoth-1`.

`coder/tui/route-door-eval.coax` sends each labelled message in
`coder/tui/route-labels.tsv` to mammoth-1 (Baseten, the default without a
router.json) and to Gemini Decision 1, with the same state and questions.
Results from 2026-11-13, measured on a loaded Mac, including curl start-up:

| Split | Messages | mammoth-1 right | Gemini right | Agree | mammoth-1 median / mean | Gemini median / mean |
| --- | --- | --- | --- | --- | --- | --- |
| held | 22 | 22 | 22 | 22 | 874 / 1046 ms | 909 / 1472 ms |
| train | 22 | 22 | 22 | 22 | 1337 / 1242 ms | 1431 / 1307 ms |

Across all 44, the two agree on the effort for 42 and on the difficulty level
for 36. Gemini used 22,962 input tokens, about $0.0023.

The answers are equally right, and Gemini is a little slower. On
2026-11-13 the switch stayed off for that reason. The owner then made Gemini
Decision 1 the default for everything, accepting the latency gap (#13473), so
it is now the route's first door.

## Where it could replace a model-with-schema call

Any call that asks a chat model for JSON that picks from a fixed set, gives a
yes or no, or rates on a scale fits this model. You get calibrated
probabilities and no output tokens to parse. The candidates:

- The world-turn talk-or-edit route.
- Coder's consent and route questions, which already use System One.
- Moderation-style checks on uploaded images, where the image input matters
  and a safety block is a typed answer.
- Ranking a short list (one score question an item, up to 1,000 a request).

Calls that need free text, tool use or a conversation stay on a chat model.

## Live results, 2026-11-13

From `coax/lib/systemone/gemini-smoke.sh --live`:

- **Mixed** (noul, choice and score over a support message): `is_defect`
  0.919, `next_action` human_escalation (p 0.743), `churn_risk` 2.989
  (level 3). 315 tokens, 457 ms.
- **Ticket** through the System One mapping: route billing (p 0.9987),
  refund 0.99, urgency 0.824 (level 1). 216 tokens, 417 ms.
- **Image** (a 24,567-byte JPEG at low resolution): cold 0.962, spell
  ray_of_frost (p 0.9975), range 1.02 (level 1). 494 tokens (241 text and
  253 image), 648 ms.

## Production settings

Set on 2026-11-13 (`coax/web/api/deploy.sh`, 7a2b52e370):

- the secret `OA_GEMINI_DECISIONS_KEY`, from Secret Manager `oa-gemini-decisions-key`
- a `"gemini-decision-1":{"backend":"gemini"}` row in `OA_MESH_ROUTES` (the `oa-mesh-routes-*` secrets)
- `OA_DECISION_ORDER=gemini-decision-1,elephant-1` for `"model":"auto"`
