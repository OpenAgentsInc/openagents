# api.hosted_gemini.v1 — launch worklog

State: **yellow** (unchanged — this work does NOT flip any promise state).

Claim: OpenAgents can verify paid Autopilot delegation through a hosted Gemini
closeout bridge in the route harness, but no public paid hosted Gemini inference
product is live.

## What this change builds

A real, reusable **production hosted Gemini executor binding** for the Autopilot
route harness:

- `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-executor.ts`
  — `createHostedGeminiWorkExecutor(config)` returns an `AutopilotWorkExecutor`
  that can be wired to `dependencies.executeReadyWork`. It derives a
  deterministic, public-safe execution closeout from the work projection for a
  `hosted_gemini` placement by driving an **injected** `HostedGeminiInferenceCaller`.
- Tests added to
  `apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts`
  (3 new cases, full file 40 pass): the armed binding delivers a paid hosted
  Gemini work order end-to-end through the route harness (closeout carries the
  model + response-digest proof refs and usage verification ref); the un-armed
  binding stays INERT (no delivery, provider never called); an unsafe inference
  ref aborts delivery instead of leaking.

Prior to this, the only thing that could satisfy `executeReadyWork` for a
`hosted_gemini` placement was a hand-written test fake. This is the genuine
production binding shape the harness was missing.

### Honest / inert by construction

- **Flag-gated, INERT by default**: when `config.enabled` is false the executor
  returns `undefined` — exactly the current production behaviour (no execution,
  no closeout). Wiring it in changes nothing until an operator arms it.
- **No secrets, no raw output**: the injected caller returns public-safe REFS
  only (response digest ref, optional usage ref, model ref). Every emitted ref
  is re-validated with the same public-safe guard the route uses
  (`publicSafeExecutionCloseoutRef`, now exported); any unsafe ref aborts the
  execution rather than leaking it.
- No settlement, no spend, no payout, no implied accepted work.

## Blocker advanced

- `blocker.product_promises.production_hosted_gemini_executor_binding_missing`
  — **partially advanced, NOT cleared.** The reusable production executor
  binding now exists and is proven wireable to the route harness, but it is not
  yet bound in the live worker dependency graph and there is no real deployed
  hosted Gemini inference caller behind it, nor a registered-agent production
  smoke. The blocker therefore REMAINS listed.

### 2026-06-20 update — hosted Gemini gateway metering lane

- `blocker.product_promises.public_paid_model_gateway_missing`
  — **partially advanced, NOT cleared.** Until now the `hosted_gemini` runner
  kind had **no metered pricing lane**: `pricingLaneForRunnerKind('hosted_gemini')`
  returned `null`, so a hosted Gemini placement exposed `activeLane: null` and
  zero pricing reason refs — i.e. no metering/pricing policy for the gateway.
  This change adds the missing buyer-funded, `usd_credits`-metered gateway lane:
  - `apps/openagents.com/workers/api/src/autopilot-work-pricing-policy.ts`
    — new `lane.autopilot_work.hosted_gemini_gateway` lane
    (`buyerDebitRequired: true`, `meterKind: 'usd_credits'`, per-unit charge),
    with `pricing.autopilot_work.hosted_gemini_metered` /
    `placement.reason.hosted_gemini_gateway_metered` reason refs now surfaced in
    hosted Gemini placement decisions.
  - `apps/openagents.com/workers/api/src/autopilot-work-pricing-policy.test.ts`
    (new, 4 cases): asserts the lane is buyer-funded + metered with a positive
    unit charge, that the metering reason refs surface, that every metered lane
    is buyer-funded (and free lanes unmetered), and that lanes are unique per
    runner kind.

  **Honest scope:** this defines the meter only. It does NOT create billing,
  entitlement, quota, provider policy, or settlement, and it is inert until the
  (still-missing) armed executor delivers hosted Gemini work. The gateway
  blocker REMAINS listed.

### 2026-06-20 update — Vertex Gemini → executor public-safe inference bridge

- `blocker.product_promises.production_hosted_gemini_executor_binding_missing`
  — **further advanced, still NOT cleared.** The executor binding already
  consumed an injected `HostedGeminiInferenceCaller`, and a real Vertex Gemini
  provider adapter (`inference/vertex-gemini-adapter.ts`) already returns a
  receipt-first `InferenceResult`. The missing connective tissue was a
  public-safe projection turning a real Gemini result into the executor's
  REFS-ONLY contract without leaking raw output. This change adds it:
  - `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-inference-bridge.ts`
    — `projectGeminiResultToPublicSafeRefs(result, digestHex)` maps a real
    `InferenceResult` to `{ modelRef, responseDigestRef, usageRef }` (model id +
    SHA-256 digest of the completion + token COUNTS only); the raw completion is
    hashed (`hostedGeminiResponseDigestHex`) and never returned. Every emitted
    ref is re-validated with `publicSafeExecutionCloseoutRef`; any unsafe/empty
    ref aborts the projection. `createVertexGeminiHostedCaller(config)` wraps an
    injected runner into a `HostedGeminiInferenceCaller`, FLAG-GATED + INERT by
    default (disabled → returns `undefined`, never calls the runner).
  - `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-inference-bridge.test.ts`
    (new, 10 cases): projection is refs-only + public-safe; secret-bearing
    completion content never appears in any ref; cached-prompt split surfaces;
    empty model/digest abort; negative/NaN token counts clamp to zero; the
    caller is INERT when disabled, drives the runner + projects when armed,
    declines cleanly when the runner returns undefined; and the SHA-256 digest
    matches the known `sha256("hello world")`.

  **Honest scope:** this is the public-safe *projection + caller seam* only. It
  does NOT build the live Effect→Promise runner that constructs a Gemini request
  from a work order and drives the adapter against real Vertex quota, it is not
  wired into the worker dependency graph, and there is still no registered-agent
  production smoke. The blocker REMAINS listed.

### 2026-06-20 update — work-order → adapter Effect→Promise runner

- `blocker.product_promises.production_hosted_gemini_executor_binding_missing`
  — **further advanced, still NOT cleared.** The bridge's
  `createVertexGeminiHostedCaller` consumed an injected `runInference` seam that
  had no implementation: the chain `work order → ??? → InferenceResult` had a
  hole exactly where a request had to be built from the work order and the
  adapter Effect driven to a Promise. This change fills that hole:
  - `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-request-runner.ts`
    — `buildHostedGeminiInferenceRequest(input, options)` turns a work-order
    input into a non-streaming, REFS-ONLY `InferenceRequest` (a fixed public-safe
    system frame + a user frame carrying only the work-order/assignment/task/
    objective refs), declining (`undefined`) when the order has no work-order or
    task ref. `createHostedGeminiRequestRunner(config)` wraps an INJECTED
    `InferenceProviderAdapter` (e.g. `makeVertexGeminiAdapter(...)`) into the
    `runInference` seam: FLAG-GATED + INERT by default (disabled → returns
    `undefined`, never touches the adapter), and it folds the typed
    `InferenceAdapterError` channel into a clean `undefined` via
    `Effect.runPromiseExit` so no throw escapes the bridge's contract.
  - `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-request-runner.test.ts`
    (new, 10 cases): request is non-streaming + refs-only (no raw content),
    carries the work-order refs, omits an empty objectives line, declines on an
    empty work-order/task ref; the runner is INERT when disabled, drives the
    adapter + returns the result when armed, defaults model + max_tokens,
    declines without calling the adapter on an unframeable order, and folds a
    typed adapter failure into `undefined`.

  **Honest scope:** the runner is still INJECTED (not wired into the worker
  dependency graph behind an armed flag), the request prompt is built from REFS
  only — the upstream resolver that dereferences task/acceptance refs into the
  real content the adapter should act on is still missing — and there is no
  registered-agent production smoke. The blocker REMAINS listed.

### 2026-06-20 update — single composition root for the executor binding

- `blocker.product_promises.production_hosted_gemini_executor_binding_missing`
  — **further advanced, still NOT cleared.** All four layers of the hosted
  Gemini chain existed as separate factories (adapter → request runner → bridge
  caller → executor), but there was no single place that assembled them into one
  `AutopilotWorkExecutor` behind one arming flag — so binding hosted Gemini in
  the live worker would have required hand-wiring the chain (and four flags) at
  the call site. This change adds that composition root:
  - `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-binding.ts`
    — `createHostedGeminiExecutorBinding(config)` takes an INJECTED
    `InferenceProviderAdapter` (e.g. `makeVertexGeminiAdapter(...)`) plus a
    SINGLE `enabled` flag (and optional model/maxOutputTokens/digest) and returns
    a fully-composed `AutopilotWorkExecutor`. The one flag is propagated to every
    layer (defense in depth): disabled → the runner never touches the adapter,
    the caller never runs inference, and the executor returns `undefined`.
  - 3 new cases in
    `apps/openagents.com/workers/api/src/autopilot-work-routes.test.ts`
    (full file 43 pass): the composed binding, driven by a spy provider adapter,
    delivers a paid hosted Gemini work order end-to-end through the real route
    harness — the persisted closeout carries the served-model ref + a SHA-256
    response-digest ref PROJECTED from the real adapter result (the raw
    completion text never appears in any ref) and a token-count usage
    verification ref, and the adapter saw a non-streaming refs-only request;
    the composed binding stays INERT (no delivery, adapter never invoked) when
    the single flag is off; and a failing adapter declines to deliver (no
    closeout) instead of throwing.

  **Honest scope:** this is the composition seam only. The binding is still
  INJECTED — it is not yet bound in the live worker dependency graph behind an
  env-gated flag — the upstream ref-resolver that dereferences task/acceptance
  refs into real adapter content is still missing, and there is no
  registered-agent production smoke. The blocker REMAINS listed.

### 2026-06-20 update — env-gated executor bound into the live worker graph

- `blocker.product_promises.production_hosted_gemini_executor_binding_missing`
  — **further advanced, still NOT cleared.** The composition root existed but its
  arming was a plain boolean: nothing read it off the worker `Env`, so the live
  worker still had `executeReadyWork` UNSET (no hosted Gemini executor at all).
  This change adds the env seam and binds it in:
  - `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-executor-env.ts`
    — `hostedGeminiExecutorArmed(env)` (DOUBLE-gated: armed only when
    `HOSTED_GEMINI_EXECUTOR_ENABLED` is on AND `VERTEX_SA_KEY` is present),
    `resolveHostedGeminiExecutor(env, deps?)` (builds the real Vertex Gemini
    adapter from env + composes `createHostedGeminiExecutorBinding`, or returns
    `undefined` when not armed), and `makeHostedGeminiExecuteReadyWork(deps?)`
    (the `(env, input) => Promise<closeout | undefined>` shape the route harness's
    `dependencies.executeReadyWork` expects).
  - `apps/openagents.com/workers/api/src/config.ts` — new default-OFF
    `HOSTED_GEMINI_EXECUTOR_ENABLED` / optional `HOSTED_GEMINI_MODEL` config env.
  - `apps/openagents.com/workers/api/src/index.ts` — wires
    `executeReadyWork: makeHostedGeminiExecuteReadyWork()` into
    `autopilotWorkRouteDependencies` (generic widened `WorkerBindings` → `Env`
    so the dependency reads the config off the live env the route already gets).
  - `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-executor-env.test.ts`
    (new, 8 cases): the armed matrix (flag×secret, affirmative/negative flag
    values, blank secret), resolve returns `undefined` + never builds an adapter
    when not armed, and the INERT `executeReadyWork` resolves `undefined` without
    touching the adapter. Plus 2 route-harness cases in
    `autopilot-work-routes.test.ts` (full file 45 pass): the env seam delivers a
    paid hosted Gemini order end-to-end when the env is armed, and stays INERT
    (no delivery, adapter untouched) when the flag is off.

  **Honest scope:** the executor is now bound in the live worker graph, but it
  is INERT on prod (no `HOSTED_GEMINI_EXECUTOR_ENABLED`, no `VERTEX_SA_KEY` for
  this lane). The upstream ref-resolver that dereferences task/acceptance refs
  into real adapter content is still missing (the runner builds a refs-only
  prompt), and there is no registered-agent production smoke. The blocker
  REMAINS listed.

### 2026-06-20 update — upstream public-safe ref-resolver for the prompt

- `blocker.product_promises.production_hosted_gemini_executor_binding_missing`
  — **further advanced, still NOT cleared.** Every layer was wired, but the
  request runner framed the prompt REFS-ONLY: the user message carried only the
  work-order/task/objective refs, so a live adapter could not act on the actual
  task. The documented missing piece was the resolver that dereferences those
  refs into real content. This change adds it (and its public-safe gate):
  - `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-content-resolver.ts`
    — `HostedGeminiRefContentResolver` (an INJECTED `(ref) => Promise<string |
    undefined>` seam, so the module reaches no datastore by itself),
    `sanitizeResolvedSnippet` (strips control chars, collapses whitespace, bounds
    length to `MAX_HOSTED_GEMINI_SNIPPET_CHARS`, and DROPS whole any snippet
    matching a known secret fingerprint — PEM keys, `sk-`/`AKIA`/`ghp_`/Slack/
    Google/JWT tokens — so credentials in referenced content never reach the
    prompt), and `resolveHostedGeminiPromptContext` (dereferences task + objective
    refs into sanitized content, declining when the task ref yields no safe
    content, best-effort skipping empty/unresolvable/unsafe objective refs).
  - `apps/openagents.com/workers/api/src/autopilot-hosted-gemini-request-runner.ts`
    — `buildHostedGeminiInferenceRequest` accepts an optional `resolvedContext`
    and appends the public-safe `task_content` + `objective_content[i]` lines
    (refs retained for provenance); the runner config gains an optional
    `resolveRefContent` resolver that, when armed, enriches the prompt and
    otherwise falls back to the existing refs-only frame.
  - Tests: `autopilot-hosted-gemini-content-resolver.test.ts` (new, 11 cases:
    whitespace/control collapse, length bound, secret-fingerprint drops for PEM +
    token shapes, full dereference, decline on unresolvable/secret-only task,
    skip of empty/missing/unsafe objectives, blank-task short-circuit) and 2 new
    runner cases (resolved content embedded when armed; refs-only frame retained
    when the resolver yields nothing safe). Resolver + runner suites: 22 pass.

  **Honest scope:** the resolver remains INJECTED — no live datastore-backed
  implementation is wired into the worker dependency graph (the bound executor
  still passes no resolver, so prod stays refs-only), and there is no
  registered-agent production smoke. The blocker REMAINS listed.

### 2026-06-20 update — provider serving policy gates the public catalog

- `blocker.product_promises.public_paid_model_gateway_missing`
  — **further advanced, still NOT cleared.** The public catalog (`/v1/models`)
  published EVERY model in the pricing table regardless of whether the gateway
  could actually serve that model's supply lane: a paid gateway was advertising
  (and letting a credits customer fund a balance toward) models whose lane has no
  provisioned credential, so the request could only fail `model_unavailable` at
  dispatch. This change adds the missing provider serving policy:
  - `apps/openagents.com/workers/api/src/inference/model-serving-policy.ts`
    — `resolveSupplyLaneArming(env)` derives which supply lanes are armed from
    credential PRESENCE only (`VERTEX_SA_KEY` → both Vertex lanes incl. the
    hosted-Gemini lane; `FIREWORKS_API_KEY` → Fireworks; the openagents-network
    serving fabric is never armed — roadmap). It reads only whether a secret is a
    non-blank string — never the value — so it neither handles nor leaks a
    credential. `filterServableCatalog` / `isModelServable` / `isLaneArmed` are
    pure helpers; `ALL_LANES_UNARMED` is the safe default.
  - `apps/openagents.com/workers/api/src/inference/models-routes.ts`
    — `ModelsListDeps` gains an OPTIONAL `laneArming`; when supplied,
    `/v1/models` advertises only servable models and `/v1/models/{id}` reports
    `model_not_found` for a known model on an unarmed lane. Omitting it preserves
    the prior list-everything behaviour (no breaking change to other callers).
  - `apps/openagents.com/workers/api/src/index.ts` — the three live gateway call
    sites now pass `laneArming: resolveSupplyLaneArming(env)`, so the LIVE gateway
    only advertises models it can actually serve.
  - Tests: `model-serving-policy.test.ts` (new, 14 cases: arming matrix incl.
    blank-credential, never-armed serving fabric, per-lane servability, identity/
    empty/lane-scoped/order-preserving filtering) and 6 new route cases in
    `models-routes.test.ts` (full file 19 pass): omitted arming lists all, Vertex
    arming advertises only Vertex models incl. the hosted-Gemini lane, no
    credential advertises nothing, retrieve resolves an armed model and 404s a
    known model on an unarmed lane.

  **Honest scope:** this is provider POLICY (which models the catalog advertises)
  only. It does NOT add billing, entitlement, quota, or settlement, and arming is
  PRESENCE-derived — it does not prove a lane's credential actually authenticates
  upstream. The gateway blocker REMAINS listed.

## What remains (for green)

- Arm the bound executor on a real deployment (`HOSTED_GEMINI_EXECUTOR_ENABLED`
  + `VERTEX_SA_KEY`) and confirm a live hosted Gemini inference call serves a
  paid placement. The chain is now assemblable from one factory
  (`createHostedGeminiExecutorBinding`) over an injected Vertex adapter, env-gated
  (`makeHostedGeminiExecuteReadyWork`), and BOUND in the live worker graph
  (`index.ts`, 2026-06-20 update above) — but it is INERT until an operator both
  arms the flag and provisions the secret for this lane.
- A LIVE, datastore-backed ref-resolver implementation wired into the worker
  dependency graph. The resolver SEAM + public-safe gate now exist and the runner
  consumes them (`autopilot-hosted-gemini-content-resolver.ts`, 2026-06-20 update
  above), but no concrete `HostedGeminiRefContentResolver` is provisioned in
  `index.ts`, so prod still frames the prompt refs-only.
- A registered-agent production smoke proving a paid hosted Gemini work order
  delivered end-to-end.
- `blocker.product_promises.public_paid_model_gateway_missing` — the hosted
  Gemini metering lane and the provider serving policy that gates the public
  catalog to servable lanes now exist (2026-06-20 updates above), but billing,
  entitlement, quota, and settlement refs for a public paid model gateway still
  remain.
- Any green flip remains receipt-first and owner-signed per
  `proof.claim_upgrade_receipts.v1`.
