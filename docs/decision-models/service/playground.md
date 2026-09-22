# Playground and chat demo

`/playground` is a bounded, session-signed surface for trying real
decision workflows, mounted only when the `accounts` document is
configured — its sign-in is the same `sess_` session token the dashboard
uses, held in the same `HttpOnly` cookie.

## The run

`POST /playground/run` builds a real `openagents.classify.v1` envelope
from the form — workspace, model door, mode (single-label, multi-label,
binary, score), pasted items, labels or a score rubric, selection
threshold, review cut, and shared instructions — and answers one of two
lanes:

- **Live** hands the session token and workspace to the real
  `POST /v1/classify` admission path in-process. The call authenticates,
  authorizes, reserves quota, forwards to the bound backend, and leaves
  the same sealed receipt any API call would — the page renders its
  digest.
- **Simulated** (the `simulate` checkbox) never touches a backend. It
  derives deterministic answers from the request's SHA-256, marks the
  page **SIMULATED**, and bills nothing. The same envelope shape renders
  both lanes so a caller can compare.

The results page shows the outcome per input, each unit's selected
label, score, raw distribution, uncertainty and review status, refusal
causes, the served model identity, elapsed time, usage, the `x-receipt`
digest, and the equivalent request JSON for `POST /v1/classify`.

The form's **envelope override** accepts a pasted
`openagents.classify.v1` document — named dimensions, a dedicated or
shared capacity, and a review policy the form fields do not name —
bounded at 64 KiB and required to declare the schema. When set, the
document runs verbatim in either lane; the workspace select and the
simulate flag still apply, and the simulated lane derives a unit per
declared dimension in its own mode.

`POST /playground/native` is the same lane pair for
`POST /v1/systemone`: a state and the pasted questions map — `noul`,
`choice`, and `score` questions — run live under the session bearer or
simulated from the request digest. The answers page renders each
question's type, selected value, confidence, and full probability
distribution, plus the model, usage, timing, receipt digest, and the
equivalent request.

## Bounds and retention

- 100 items per run, 4,096 bytes each; 20 labels; rubrics of 2–10
  levels; uploads and pasted envelope or questions documents of 64 KiB.
- Refusals, partial batches, reviewer failures, and unavailable results
  render as their own rows — never hidden behind an aggregate.
- Nothing persists: no inputs, answers, or transcripts are stored. The
  results page is the whole record; closing it deletes everything but
  the sealed receipts the decision calls themselves produced.
- Quota feedback is the call's own admission — a workspace over its
  budget sees the same typed refusal the API returns.
- The demo does not fetch URLs or external content; everything the tool
  sees is pasted or uploaded by the caller.

## The chat demo

`/playground/chat` is a bounded conversation whose only tool is the
classify facade: each message becomes one intent classification over a
fixed label set, and the reply is composed from the tool's actual answer
— selected intent, refusal, or uncertainty — never an invented outcome.
Caps: 10 turns, 1 tool call per turn, each call bounded by the facade's
own limits. The transcript lives in the page's form state alone; closing
the page deletes it. The simulated lane runs the same shape
deterministically and unbilled.

The demo authorizes nothing: it cannot execute commands, fetch content,
or act outside the classify facade.
