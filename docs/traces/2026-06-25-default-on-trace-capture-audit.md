# Default-ON redacted, private-by-default trace capture for all free usage — audit

> **Status: AUDIT / spec (2026-06-25).** Honest scope: this is a *design + gap
> audit + implementation plan*. It changes NO runtime behavior and ships NO
> capture code. The actual flip lives behind the GitHub issues this doc files
> (keystone #TBD and children). Every file path and symbol below was read in the
> live tree at `apps/openagents.com/workers/api/`. Where this doc says "exists"
> it cites the real code; where it says "missing / must be built" it means there
> is no implementation in the tree today.
>
> Owner directive (overrides the repo's strict-bug-only issue default): turn
> trace capture **ON by default for all free-tier traffic** — redacted,
> private-by-default, capturing everything that hits the platform **unless the
> caller is paying for privacy.** This audit specs that policy against the real
> seams.

## 0. The policy (owner intent, Episode 243)

From `docs/transcripts/243.md` (line 74), the exact data-sharing thesis:

> "you give away the core API for free and then you compensate people on the
> back end. … if you want super privacy of your data, you have to pay for that,
> and/or configure the confidential compute module. But if you're just using the
> free API, you can assume that we're using that to train the next generation of
> models."

Operationalized:

| Tier | Capture? | Visibility | Training/data-market |
| --- | --- | --- | --- |
| **Free API (default)** | **YES — auto, redacted** | **private (`unlisted`/`owner_only`)** | eligible (honest disclosed terms) |
| **Owner opts a trace public** | already captured | `public` at `/trace/{uuid}` | as above |
| **Paid-for-privacy / confidential-compute** | **NO** (not captured, or encrypted/not-retained) | n/a | excluded |

The data-market / training thesis depends on capturing the bulk of free traffic.
**Today we capture almost none of it.**

## 1. Current state — capture is OFF by default

### 1.1 The emitter and its two gates

`apps/openagents.com/workers/api/src/inference/khala-chat-trace-emitter.ts`:

- **Gate A — global flag, DEFAULT OFF.** `isKhalaChatTraceEmitEnabled(value)`
  returns `false` for `undefined`/empty/`false`; only `1|true|yes|on` enable it.
  The Worker reads `env.KHALA_CHAT_TRACE_EMIT_ENABLED`
  (`apps/openagents.com/workers/api/src/config.ts:159`).
- **Gate B — per-request opt-in, DEFAULT OFF.**
  `resolveKhalaChatTraceOptIn({ request, rawBody })` requires an explicit caller
  switch: header `x-oa-emit-trace: on` **or** body field `oa_emit_trace: true`.
  With neither present it returns `false`.
- `emitKhalaChatTrace(session, deps)` short-circuits to
  `{ emitted: false, reason: 'disabled' }` when `!deps.enabled`, and
  `{ reason: 'not_opted_in' }` when `!deps.optedIn` — **no store call, no work**.

Both gates are AND-ed at the call site in
`apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts`
(≈L2740):

```ts
if (deps.traceEmit !== undefined && deps.traceEmit.enabled && traceEmitOptIn) {
  yield* Effect.promise(() => deps.traceEmit!.emit({ … }))
}
```

Wiring in `apps/openagents.com/workers/api/src/index.ts` (≈L10974) passes
`enabled: isKhalaChatTraceEmitEnabled(env.KHALA_CHAT_TRACE_EMIT_ENABLED)` and the
emit closure that builds the owner from `agent:<id>` and persists through
`makeD1TraceStore(openAgentsDatabase(env))`.

**Net:** even with the flag ON in prod, a trace is emitted only for a request
that *explicitly* opted in. Practically, only the **qa-runner** and any
hand-opted session produce traces. The bulk of served Khala tokens generate
metering/usage receipts (`inference/metering-hook.ts`,
`inference/inference-free-tier-key.ts` zero-debit receipts) but **NO trace**.

### 1.2 What flips capture default-on (the precise change)

The keystone change is small and surgical, but must stay **fail-soft** and
**free-tier-scoped**:

1. **Decouple capture from the per-request opt-in for free-tier traffic.** The
   call site condition must become "emit when `enabled` AND
   (`traceEmitOptIn` OR caller is free-tier-and-not-paid-privacy)". The cleanest
   shape: thread a resolved `captureDefault: boolean` into `traceEmit.emit(...)`
   computed from the free-tier decision (see §4.2), and have the gate be
   `deps.traceEmit.enabled && (traceEmitOptIn || captureDefault)`.
2. **Keep the global flag as the master kill-switch** but plan to default it ON
   in prod once redaction + private-by-default land (it stays the safety valve).
3. **Inside `emitKhalaChatTrace`**, replace the hard `optedIn` reject with a
   policy input (`optedIn || captureDefault`) so the existing validator +
   tripwire path is reused unchanged.
4. **Default visibility for an auto-captured trace = `unlisted`** (already the
   default; see §2) — and we should consider `owner_only` for auto-capture so a
   captured-but-unshared trace is not even link-reachable (see §2.3).

Everything downstream (`khalaChatSessionToAtifTrajectory`, validator, tripwire,
`createTrace`, idempotency keyed on `responseId`) is reusable as-is.

## 2. Visibility model — private-by-default ALREADY EXISTS; opt-in-to-public DOES NOT

### 2.1 The tier exists and is enforced

`packages/atif/src/trace-schema.ts:25` —
`TraceVisibility = S.Literals(['public', 'unlisted', 'owner_only'])`.

Migration `apps/openagents.com/workers/api/migrations/0228_agent_traces.sql:24` —
`visibility TEXT NOT NULL DEFAULT 'unlisted' CHECK (visibility IN ('public','unlisted','owner_only'))`.

Read-gating in `apps/openagents.com/workers/api/src/trace-store-routes.ts`
(`routeRead`, ≈L745, and the blob serve `routeBlobServe`, ≈L815):

- `public` + `unlisted` → readable by anyone **with the link**, no auth.
- `owner_only` → requires the owning browser session (or admin), else **404**
  (does not even reveal existence).

The emitter defaults emitted traces to `unlisted`
(`khalaChatSessionToAtifTrajectory(..., visibility ?? 'unlisted')`).

**So private-by-default is real: an auto-captured trace is NOT in the public feed
and `/trace/{uuid}` is not discoverable.** The public discovery index
(`idx_agent_traces_public`) is scoped to `visibility = 'public'` only.

### 2.2 The gap: there is no way for an owner to opt a trace public

Enumerated routes in `trace-store-routes.ts`:

- `POST /api/traces` (+ `/api/traces/upload`) — ingest (sets visibility at
  **creation** from body/trajectory, default `unlisted`).
- `GET /api/traces/{traceRef}` — visibility-gated read.
- `POST|GET /api/traces/{uuid}/blob/{r2Key}` — media.
- `GET /api/traces` — owner-scoped list.

There is **no PATCH / set-visibility / share endpoint**, and `TraceStore`
(`trace-store-d1.ts`) exposes no `updateVisibility`. Once created `unlisted`, a
trace's tier is immutable through the API. **Owner opt-in-to-public must be
built** (a new authenticated mutation + store method; see issue 2).

### 2.3 Decision for auto-capture default tier

Recommend **`owner_only` for auto-captured free-tier traces** (not `unlisted`),
because an auto-captured trace the user never chose to share should not be
link-reachable by uuid guess/leak. `unlisted` is appropriate only once the owner
explicitly opts to "share by link." The opt-in-to-public route then walks
`owner_only → unlisted (link) → public (feed)`.

## 3. Redaction / public-safety — what exists and the gaps

### 3.1 The tripwire (value-based backstop)

`packages/atif/src/trace-schema.ts` `atifTraceTripwire(trajectory)` serializes
the trajectory and rejects on findings:

- `secret_material` — regex over `sk-…`, `sk_live_`/`sk_test_`, `rk_live_`,
  `xox[baprs]-`, `gh[pousr]_…`, `AKIA…`, `AIza…`, JWT `eyJ….….…`.
- `wallet_or_payment_material`, `local_path`, `pii_email`
  (`[A-Za-z0-9._%+-]+@…`).
- Model-id allow-listing was intentionally **removed** from the *trace* tripwire
  (a trace's model id is session content). The **gateway projection** is what
  guarantees `openagents/khala` is the only model id emitted (see §3.2).

The emitter runs the SAME `validateAtifTrajectory` + `atifTraceTripwire` the
`POST /api/traces` ingest runs (never bypassed). A finding ⇒
`{ emitted: false, reason: 'public_safety_rejected' }` (rejected, **not**
silently redacted).

### 3.2 The gateway projection (model-id safety)

The emitter maps every emitted step's `model_name` to the public
`openagents/khala` (`KHALA_TRACE_MODEL_NAME`) — never a raw Vertex / Fireworks /
Hydralisk backend id — and drops gateway-injected system scaffolding
(`GATEWAY_SYSTEM_PROMPT_MARKERS`: identity prompt, component catalog, acceptance
contract). This is the same public-safe projection the Khala gateway applies to
responses.

### 3.3 The CRITICAL gap for capture-everything

The current pipeline is **reject-on-leak, not redact-on-leak.** For the
qa-runner / opt-in surface that is correct (a leaky projection is dropped). But
for **capture-everything-by-default**, "drop the whole trace if it contains an
email or a secret-looking token" would silently lose a large fraction of real
user traffic — and real conversations *will* contain emails, file paths, and
secret-shaped strings.

The data-market plan already names the missing piece: a **redaction service**
(#6219) is the *primary scrubber*; the tripwire is the *backstop*. For
default-on capture we MUST:

1. Run a redaction pass (#6219) that **scrubs** secrets/keys/credentials,
   wallet/payment material, and PII (emails, and consider phone/SSN-shaped) into
   placeholders **before** the tripwire, so the trace is captured-and-safe
   rather than dropped.
2. Keep `atifTraceTripwire` as the post-redaction backstop: if anything still
   trips, **drop that trace** (fail-closed on safety) but never fail the
   completion.
3. No-CoT-exposure: the emitter must continue to NOT surface raw chain-of-thought
   / reasoning beyond what the public projection already includes. (ATIF has a
   `reasoning_content` field; the capture path must leave it unset / projected,
   per the workspace no-CoT-exposure invariant.)

**Gap summary:** redaction (#6219) is referenced as a dependency but is **not in
the tree**. Capture-everything is unsafe until it exists. This is the hardest and
highest-risk dependency of the whole flip.

## 4. Free-tier identification and paid-privacy exclusion

### 4.1 Free-tier is already identifiable

`apps/openagents.com/workers/api/src/inference/inference-free-tier-key.ts`:

- A free key is a normal `oa_agent_` bearer minted at `POST /api/keys/free`.
- `makeFreeTierGate(deps)` → `FreeTierGate(accountRef, model)` returns
  `{ free: boolean, reasonRef }` by reading `readAccountFreeTier(db, accountRef)`
  + quota. Wired into the chat route's `checkFreeTier` seam
  (`chat-completions-routes.ts` ≈L454), gated by `INFERENCE_FREE_TIER_ENABLED`.
- The metering decorator `withFreeTierKhala` records the zero-debit free receipt.

So at completion time the route already knows whether this account is a
free-tier account on the free Khala lane. **That same `checkFreeTier` /
`readAccountFreeTier` signal is the capture trigger** for default-on capture.

Trace ownership: the emit closure builds the owner from `accountRef` (`agent:<id>`
→ `ownerUserId`). A free key IS an `oa_agent_` credential, so it satisfies the
emitter's `owner !== undefined` gate. (Open item: a fully anonymous free key —
`POST /api/keys/free` allows anonymous mint — still has an account id to own the
trace; confirm there is always a stable `ownerUserId` so a captured trace is
never unowned. If an anonymous draw has no account ref, capture must skip, per
the existing `no_owner` no-op.)

### 4.2 Paid-privacy / confidential-compute exclusion does NOT exist

A repo-wide search for `confidential` / `ZDR` / `no-retain` / `pay…privacy` in
the inference path returns **no privacy/confidential-compute module** — the
`confidential`/`privacy` hits are unrelated (legal safe-hold workrooms, etc.).
There is **no mechanism today to mark a caller as paying-for-privacy and exclude
them from capture.** This must be built as the **opt-OUT** that mirrors the
free-tier opt-in:

- A privacy entitlement on the account (e.g. a `privacy`/`zero_retention` flag on
  the agent/account record, or the confidential-compute module being configured),
  resolved at the same seam as `checkFreeTier`.
- Capture decision: `captureDefault = freeTier.free && !paidPrivacy.enabled`.
  A paid-privacy caller is **never** captured (the emit closure is not invoked /
  short-circuits), regardless of the global flag. Fail-**closed** on privacy: if
  the privacy entitlement read errors, treat the caller as private and do NOT
  capture (the inverse of free-tier's fail-closed-to-paid).
- For paid funded (non-free) callers who are NOT paying explicitly for privacy:
  policy decision needed (default: only **free** traffic is auto-captured;
  funded paid callers are captured only on explicit opt-in, matching today). The
  owner's framing is "free = captured; pay for privacy = excluded," so the
  conservative default is **capture free only**, leave funded paid as opt-in.

## 5. Data model + flags needed

Existing (reuse):
- `agent_traces.visibility` (`public|unlisted|owner_only`, default `unlisted`).
- `agent_traces.training_consent`, `license`, `content_digest`,
  `reward_eligible`, `reward_amount_sats`, `upload_source` (migration 0229).
- `KHALA_CHAT_TRACE_EMIT_ENABLED` (master kill-switch).
- `INFERENCE_FREE_TIER_ENABLED` + `checkFreeTier` (capture trigger source).

New (to build):
- **Capture-default flag** (separate from opt-in), e.g.
  `KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT` — lets us stage the flip independently
  of the existing opt-in flag.
- **Privacy entitlement** on the account/agent record (a `privacy_tier` /
  `zero_retention` column or a confidential-compute config check) + a resolver
  at the chat seam.
- **A `demand`/`consent` tag** on auto-captured traces distinguishing
  "auto-captured under free-tier disclosed terms" from "user-uploaded with
  explicit consent" (the existing `training_consent`/`upload_source` columns can
  carry this; auto-capture sets `upload_source = 'agent'` and a new
  `capture_source = 'free_tier_auto'` marker is worth adding for honest
  provenance).
- **Visibility-mutation route + store method** (`updateVisibility`) for
  owner-opt-in-to-public (§2.2).
- Recommend auto-capture default tier `owner_only` (§2.3) — a one-line change to
  the emit default, plus the `updateVisibility` walk.

## 6. Storage / cost implications

Capturing ~all free-tier completions instead of a handful is a large volume
increase. Real considerations:

- **D1 row volume + ~1MB value cap.** Multi-MB trajectories already offload to R2
  (`makeR2TraceTrajectoryBlobStore`, `trajectory_r2_key`, migration 0230). At
  capture-everything scale, prefer R2-by-default for the trajectory JSON and keep
  D1 to the indexable row; revisit the inline-vs-R2 threshold.
- **Write amplification on the hot completion path.** Capture is fire-and-forget
  (`Effect.promise` after the priced completion) and must stay so — it can never
  block/fail/delay `/v1/chat/completions`. Consider `ctx.waitUntil` / a queue so
  the D1+R2 writes happen off the response critical path at scale.
- **Idempotency** is keyed on `responseId` (`createTrace` idempotencyKey), so a
  retried emit does not duplicate — keep this.
- **Retention policy** is currently unbounded; capture-everything needs an
  explicit retention/TTL decision (and a deletion path for redaction misses).
- **Quota interaction:** free-tier is already daily-quota-bounded
  (`FREE_TIER_MAX_REQUESTS_PER_DAY` = 2000, tokens = 2.5M/key), which caps the
  per-key capture rate as a natural backpressure.

## 7. What cannot be backfilled

Past free-tier traffic was **never captured** (only opt-in + qa-runner traces
exist). There is no stored prompt/completion corpus to reconstruct from — usage
rows carry counters/refs, not content. **Capture is forward-only**: the corpus
starts accumulating the moment default-on capture ships. This is a one-way
opportunity cost of every day the flip is delayed — the central reason the owner
wants it "in place right now."

## 8. Implementation plan (keyed to real seams)

Ordered; each maps to a filed issue (§9):

1. **Redaction service (#6219, hard dependency).** Build the scrub-before-store
   pass (secrets/keys/credentials, wallet/payment, PII) so capture-everything is
   safe-by-redaction, not lossy-by-rejection. Tripwire stays the backstop.
   *Blocks the keystone for real free traffic.*
2. **Capture-default flip (keystone).** Add `captureDefault` resolution from the
   free-tier + privacy seam; thread it through `traceEmit.emit` and
   `emitKhalaChatTrace`; gate on the new
   `KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT` flag; keep fail-soft + idempotent.
   Default emitted visibility `owner_only` (or `unlisted` per owner call).
3. **Paid-privacy opt-OUT.** Privacy entitlement column/config + resolver;
   `captureDefault = free && !privacy`; fail-closed-to-private. The
   confidential-compute module configuration is the explicit exclusion.
4. **Owner opt-in-to-public.** `updateVisibility` store method + authenticated
   mutation route (`owner_only → unlisted → public`), owner/admin-gated, scoped
   to the owning account.
5. **Disclosure / consent surface.** The honest "free API → we capture & may
   train; pay for privacy" terms, routed through `docs/promises/` and surfaced at
   free-key mint + in product copy. (Do not change user-facing copy without owner
   sign-off; this issue specs the terms text for the owner to approve.)

## 9. Issues filed

- Master/keystone: **default-ON redacted trace capture for free-tier traffic.**
- Child: **private-by-default visibility + opt-in-to-public mutation route.**
- Child: **paid-privacy / confidential-compute capture opt-OUT.**
- Child: **data-sharing terms/consent disclosure surface** (via `docs/promises/`).

(Issue numbers/URLs recorded in the commit / handoff. The repo's strict-bug-only
issue default is explicitly overridden by the owner's directive for this work.)

## 10. Invariants honored

- **Public-safe (no secrets ever):** capture path runs redaction (#6219) +
  `atifTraceTripwire` backstop + gateway model-id projection; fail-closed drop on
  any residual tripwire finding.
- **No-CoT-exposure:** the emitter projects/omits raw reasoning; the capture path
  must not start surfacing `reasoning_content` raw.
- **Fail-soft:** capture is fire-and-forget after the priced completion; it never
  blocks, fails, or alters `/v1/chat/completions`.
- **Private-by-default:** auto-captured traces are `owner_only`/`unlisted`, never
  in the public feed, public only on explicit owner opt-in.
- **No money / no authority drift:** a captured trace is evidence only; the
  data-market reward marker stays INERT (owner-gated #6221), no payout/settlement.

## References (real files)

- `apps/openagents.com/workers/api/src/inference/khala-chat-trace-emitter.ts`
- `apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts` (≈L2181, L2740)
- `apps/openagents.com/workers/api/src/index.ts` (≈L10974)
- `apps/openagents.com/workers/api/src/config.ts:159` (`KHALA_CHAT_TRACE_EMIT_ENABLED`)
- `apps/openagents.com/workers/api/src/trace-store-d1.ts`
- `apps/openagents.com/workers/api/src/trace-store-routes.ts`
- `apps/openagents.com/workers/api/src/atif-trace-schema.ts` → `packages/atif/src/trace-schema.ts`
- `apps/openagents.com/workers/api/migrations/0228_agent_traces.sql`, `0229_…data_market.sql`, `0230_…trajectory_r2.sql`
- `apps/openagents.com/workers/api/src/inference/inference-free-tier-key.ts`
- `docs/traces/README.md` (the trace primitive spec); data market #6221/#6220/#6219, EPIC #6206
- `docs/transcripts/243.md:74` (the free-API-trains / pay-for-privacy policy)
