# Triage: the authority, money, privacy, and safety subset of the 171 unmasked symbols

Date: 2026-08-03. Base commit: `e861903bc42b56eb36d3bc7c92eca4bd078dceed`.

## Why this document exists

`scripts/uncalled-production-symbol-guard.mjs` fails the build when a production
symbol's only caller is a test. Until `e861903bc4` it decided "does production
reference this?" by matching raw file text, so a TSDoc `{@link foo}`, a
`// TODO: call foo` note, or the string `"foo"` counted as a production caller.
The worst shape was `Effect.fn("Svc.member")` span names, which exempted service
members wholesale.

Blanking comments and string / template / regular-expression literals before
matching revealed **171 symbols the guard had been blind to**. They were
recorded in `inheritedDebt` in
`scripts/uncalled-production-symbol-baseline.json` — a dated record of a known
defect, explicitly never an approval — and not in `allowed`. See
[`uncalled-production-symbols.md`](uncalled-production-symbols.md) for the rule
itself.

This document triages the subset that touches **authority, money, custody,
privacy, or safety**, because that is the subset where "this function is never
called" can mean "a control the owner believes is enforcing is inert". Every
symbol below is classified:

- **(a)** a control that is supposed to be enforcing right now and is not — a
  live defect;
- **(b)** a control that is dormant because the surface it guards is not
  shipped, not flag-enabled, or structurally unreachable;
- **(c)** superseded or dead code — an independent implementation already
  enforces the property, or the property no longer exists.

The rule that decided most verdicts was: **before calling anything a live hole,
check whether the behaviour is enforced somewhere else.** It nearly always was.
The `(a)` list is short on purpose.

## Method, and what it cannot tell you

For each symbol: read the definition, read every reference site, and classify
each site as definition / test / comment / string literal / real call. Grep
alone was treated as insufficient — that is exactly the failure mode being
triaged. Route factories were checked for an actual mount in the Worker
entrypoint and for the production Cloud Run env flag. Effect service members
were checked for a layer that is actually constructed. Barrel re-exports were
followed to see whether the re-export itself has a consumer.

Live production evidence uses **read-only public GETs only**. No mutating
request was made, so anything that would need a POST to confirm is marked as
source-level. Where a 404 had to be distinguished from "mounted but wrong
method", a control request was used instead of a write:

```
GET /api/public/business-signup        -> 405 method_not_allowed   (mounted, POST-only)
GET /api/public/product-promises       -> 200                      (mounted)
GET /api/public/free-tier-data-sharing -> 404 not_found            (not mounted)
```

Three tooling caveats worth carrying forward:

- **Two files under `packages/local-secret-store/` contain literal NUL bytes**
  (a documented key separator in `src/locator.ts`, and
  `src/platform-secret-store-adapters.test.ts`). BSD `grep` treats them as
  binary and skips them **silently** — no "Binary file matches" notice. A
  grep-only audit under-reports references there and can misread an unwired
  symbol as ordinary dead code. Use `grep -a` or an AST tool. The guard reads
  them correctly.
- Some names collide with unrelated local functions elsewhere.
  `authorizeMutation` is the clearest: naive grep hits live, unrelated
  definitions in `turn-checkpoint-host.ts` and `workspace-service.ts` and would
  clear the symbol wrongly.
- Source comments are not evidence of wiring. Several comments in this set
  assert a live wiring that does not exist, and one names a helper that was
  deleted. Comments were the thing masking these symbols; they cannot also be
  the thing that clears them.

## Summary

**64 symbols reviewed**: 61 of the 171 newly unmasked, plus 3 pre-existing
`inheritedDebt` entries that the same investigation surfaced
(`decideFiatKycGate`, `decideAbuseResponse`, and
`handleFreeTierDataSharingDisclosureApi` — the last of which is the only `(a)`).

| Class | Count | Meaning |
| --- | --- | --- |
| (a) live defect | 1 | a documented, promise-referenced public endpoint that is not mounted |
| (b) dormant control for a dormant surface | 44 | |
| (c) superseded / dead, property enforced elsewhere | 19 | |

The remaining ~110 of the 171 fall outside the authority / money / privacy /
safety class (renderers, backfills, feed builders, conformance suites, training
receipt projections). They were scanned by name and path, not read.

The dominant pattern is **not** missing enforcement. It is **orphaned
evidence**: a control was written, tested, and named in a document, a behavior
contract, a service topology, or an assurance spec as the enforcing mechanism —
and then the integrator implemented an equivalent control inline, or in a
different module, or the surface was retired outright. The safety property
survived; the documentation now points at code that cannot run.

That is a real defect class and it is what a guard against test-only code is
for. It is not the same defect as an unguarded surface, and this document does
not conflate the two.

## The single fact that resolves most of the money set

On 2026-07-14 the VP-1 retirement excised the whole money graph
(`apps/openagents.com/workers/api/src/money-surface-retirement.ts`,
`MONEY_SURFACE_RETIRED_AT = '2026-07-14'`, excision commit `b022f72748`), and
`scripts/vp1-retired-money-surface-guard.mjs` in `check:fast` keeps it out.
Payments, checkout, billing, credits, treasury, partner payouts, Sites, and
tipping return `410 money_surface_retired`. Verified live:

```
GET /api/public/partner-payouts             -> 410 {"schemaVersion":"openagents.money_surface_retired.v1", ...}
GET /api/operator/partners/payout-ledger/x  -> 410 {"schemaVersion":"openagents.money_surface_retired.v1", ...}
```

The inference gateway kept the same posture. `inference/metering-hook.ts`
types `MeteringOutcome.metered` as the literal `false` with
`paymentMode: 'no-spend'`; `chat-completions-routes.ts` returns
`503 platform_funding_unavailable` for any request that is not caller-funded
BYOK, the authenticated org-runtime no-meter channel, or an OpenAuth/internal
hosted lane.

**Consequence for triage: a money control that is not wired cannot be losing
money on a surface that has no charging capability.** Every "uncalled spend
control" below inherits this, and none of them is `(a)`.

## Ranked (a) findings

### A1 — `handleFreeTierDataSharingDisclosureApi` is exported and never mounted, so a live promise's evidence route 404s

`apps/openagents.com/workers/api/src/inference/free-tier-data-sharing-routes.ts#handleFreeTierDataSharingDisclosureApi`

This is **pre-existing `inheritedDebt`, not one of the new 171** — it entered
the ledger when the guard first landed (`ee71183d3b`). It is reported here
because it was found by the "is this enforced elsewhere?" check on the
inference set, and because it is the one symbol in the whole reviewed set whose
absence has a live, externally observable consequence.

Evidence:

- The handler is exported at `free-tier-data-sharing-routes.ts:13` and is
  imported by **no** file. `index.ts` never references it.
- `GET https://openagents.com/api/public/free-tier-data-sharing` returns `404`
  with `{"error":"not_found"}`. The 405 control above proves that means
  "unregistered path", not "wrong method".
- The live OpenAPI at `GET /api/openapi.json` declares the path with
  `operationId: getFreeTierDataSharingDisclosure`, `security: []`, described as
  "Read-only, no auth, no secrets" with no arming flag.
- The live registry at `GET /api/public/product-promises` records
  `data.free_tier_capture_disclosure.v1` in state **`yellow`** — an active
  record, not withdrawn — carrying the evidence refs
  `route:/api/public/free-tier-data-sharing` and `route:/api/keys/free`. **Both
  return 404.** Its verification text begins "GET
  /api/public/free-tier-data-sharing returns the canonical disclosure…".

Consequence, stated precisely and no further: **a public, agent-readable
privacy disclosure that an active promise names as its verification method is
unreachable.** An agent or user following the documented API surface to read
the free-API data-sharing terms gets a 404.

**What this is not.** The capture the disclosure describes is not currently
occurring, so this is not an undisclosed privacy capture:

- `KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT` is `"true"` in
  `scripts/cloudrun/env-production.yaml`, and `traceEmit.captureDefaultEnabled`
  is wired from it at `index.ts:16512`;
- but the branch is only taken when `traceEmit.resolveCaptureDefault` is
  supplied (`chat-completions-routes.ts:810`, consumed at `:3806`), and
  `index.ts` **never supplies it** — the identifier appears in exactly three
  places, all inside the route module;
- so `captureDefault` is permanently `false`, and only an explicit
  `x-oa-emit-trace` opt-in emits a trace.

Severity: **S2**. A supported agent-discovery path is broken and an active
promise's stated verification cannot be performed. Filed as a strict bug.

### Adjacent, deliberately not filed as an issue

The same 404 investigation showed the free-tier product surface is retired in
code while every discovery surface still advertises it:

- `POST /api/keys/free` returns `404`. The route module
  (`free-key-mint-routes.ts`), the quota module (`inference-free-tier-key.ts`,
  `FREE_TIER_MAX_REQUESTS_PER_DAY=2000` / `FREE_TIER_MAX_TOKENS_PER_DAY=2500000`),
  `inference-free-allowance.ts`, and `inference-operator-exemption.ts` were all
  deleted by `6ab684d63a` ("retire inference credit entitlements", 2026-07-14).
- `INFERENCE_FREE_TIER_ENABLED` is still `"true"` in the production env and
  still declared in `config.ts`, but **no code reads it** — every remaining
  occurrence is a documentation string.
- `GET /api/v1/models` returns `openagents/khala` **without** the
  `oa_free_tier_eligible` / `oa_free_tier` fields the OpenAPI describes;
  `model-catalog.ts` no longer emits them.
- The live registry still records `inference.khala_free_openai_compatible_api.v1`
  as **green**, and its own verification text requires
  "POST /api/keys/free to return an `oa_agent_` bearer with the published
  free-tier limits".

This is a product-promise accuracy gap. `CLAUDE.md` routes promise gaps to the
Product Promises Forum rather than to GitHub issues, so it is recorded here and
belongs in the Forum, not in the issue tracker.

## (b) Dormant controls for dormant surfaces

### Inference abuse controls — the whole module is unreachable

`apps/openagents.com/workers/api/src/inference/inference-abuse-controls.ts`

Every exported decider is test-only. `decideFairShare`, `decideSpendCap`, and
`clawbackInferenceCredits` are in the new 171; `decideAbuseResponse` and
`decideFiatKycGate` were already in the ledger. The module's only non-test
mentions outside itself are two comment blocks — `index.ts:16428-16436` and
`chat-completions-routes.ts:668-671` — which is precisely the masking shape.

- **`decideFairShare`** — the route declares a `checkFairShare` seam
  (`chat-completions-routes.ts:678`) and genuinely consumes it (`:2609-2611`),
  but `index.ts` never supplies it, so the gate is open. `index.ts:16430`
  states this outright: "deliberately LEFT UNWIRED here (=> the gate is OPEN /
  no-op) until a per-account rolling-window counter store … lands".
  **Enforced elsewhere for the only exposed lane:** the sole platform-funded
  path is the OpenAuth hosted lanes (`gemini-3.6-flash`, `kimi-k3`,
  `gpt-5.6-luna`), and `checkSelfProvisionedDailyCeiling` **is** wired
  (`index.ts:16247`, `makeSelfProvisionedDailyCeilingGate`), reading the same
  `token_usage_events` UTC-day window as the google-gemini proxy so one
  identity cannot get two allowances. Everything else is caller-funded BYOK or
  503. Honest residue: there is no per-account request-rate limit on
  `/v1/chat/completions`, so a funded caller can burst; no OpenAgents spend is
  at stake.
- **`decideSpendCap`** — worse than unwired: the route has **no spend-cap seam
  at all**. `checkSpendCap` exists only inside the `index.ts` comment. Its own
  default is `maxSpendMsatPerWindow: null` (gate open), and there is no surface
  anywhere for an account or owner to configure a cap, so nothing is silently
  failing to honour a configured value. The msat spend it caps no longer
  exists.
- **`clawbackInferenceCredits`** — the comment says it "hangs off the Stripe
  dispute/refund webhook path". **There is no Stripe webhook route in the
  Worker at all**; `STRIPE_API_KEY` / `STRIPE_WEBHOOK_SECRET` are in the VP-1
  retired-secret pattern. No chargeback event can arrive, and the credits it
  would reverse are retired.
- **`decideFiatKycGate`**, **`decideAbuseResponse`** — same: card top-up and
  the abuse-signal source were retired with the money graph.

### Cloud primitive metering hooks

All three route factories **are** mounted (`index.ts:16851`, `:17043`,
`:17067`; dispatched from `worker-routes.ts:263/305/321`), and **none** of the
three mounts passes a `meteringHook`, so each falls to its no-op stub
(`… deps.meteringHook ?? stub…`). `settleCloudPrimitiveCharge` — the function
all three hooks call — has zero production callers.

- **`makeLedgerFineTuningMeteringHook`**, **`makeLedgerSandboxMeteringHook`** —
  `CLOUD_FINE_TUNING_ENABLED` and `CLOUD_SANDBOX_COMPUTE_ENABLED` are **absent
  from `env-production.yaml`** (present only in `env-staging.yaml`), and
  `deploy-cloudrun.sh` uses `--env-vars-file`, a full replacement, so absence
  is authoritative. Both routes 404 in production. Money consequence: none.
  Promises `cloud.fine_tuning_service.v1` and `cloud.sandbox_compute_service.v1`
  are live-registry **`planned`**.
- **`makeLedgerCloudCodingMeteringHook`** — this one's surface **is** armed
  (`CLOUD_CODING_SESSIONS_ENABLED: "true"`, `OA_CODEX_GCE_PROVISIONER: "live"`),
  so Agent Computer runs really do launch Firecracker microVMs on OpenAgents
  GCE. But wiring the hook would still charge nothing: it is invoked once at
  launch with **no `usage` field** (`cloud-coding-session-routes.ts:2043-2047`),
  and the hook returns `{metered:false, receiptRef:null}` immediately when
  `context.usage === undefined` (`:1645-1650`). There is no end-of-session
  metering call anywhere in the file. The missing piece is a usage-bearing
  settlement callback that was never built, not a wiring line.
  What *is* charged: inference tokens consumed inside the microVM are recorded
  to `token_usage_events` via `khala-cloud-runtime-usage-routes.ts:737`. Only
  VM rental is unbilled, the promise `autopilot.cloud_coding_sessions.v1` is
  live-registry **`red`**, and the only live control is the admission gate's
  402 on `availableBalanceMsat <= 0` (a presence check, never a debit) plus
  `maxConcurrentSessions: 1`.

Two comments in that file are themselves false and should be corrected:
`:1622-1623` claims the fine-tuning and sandbox hooks "already are" wired in
`index.ts` (neither is), and `:1624` instructs a future implementer to pass
`creditBalanceProjectionRecorderForEnv(env)`, **a function that no longer
exists anywhere in the repository** — its only occurrence is that comment.

### Other money and settlement symbols

- **`carryLaborProductOrderToSettlement`** — no route;
  `/api/public/autopilot/labor-products` 410s. Doubly inert: even if called,
  `settleLaborProductOrder` returns `{_tag:'disabled'}` without `deps.enabled`
  and requires an owner sign-off ref nothing supplies.
- **`makePublicAcceptedOutcomeSettlementRoutes`** — never mounted, from
  introduction (`a6ba45068a`) to HEAD; the file's own header says "This lane
  does NOT edit router.ts or index.ts", handing wiring to a coordinator that
  never did it. `GET /api/public/accepted-outcome/settlement/{id}` returns 404.
  Its promise `payments.accepted_outcome_economics.v1` does carry that route as
  an evidence ref — but that promise is **`withdrawn`** in the live registry, so
  this is not a live non-dereferenceable claim.
- **`projectPylonSettlementManifest`**, **`recordModeWorkReceipt`** — the
  entire `pylon-multi-earning-receipts.ts` module is imported by exactly one
  file: its own test. Pure functions; they mint nothing.
- **`assertXClaimRewardSmokeCandidate`**, **`isKhalaCodePaidPlansEnabled`**
  (`/v1/khala-code/plans/purchases` is a retired 410 prefix),
  **`buildOpenAgentsExternalRepoStudyPilotAdmission`**,
  **`planCustomerPrivateValidationDelivery`** (its own promise text records it
  as "HONEST/INERT by construction … `deliverable` is ALWAYS false") — all
  test-only, all dormant by design.

### Sovereign identity and local secret store — dormant together

`packages/sovereign-identity` is depended on by `apps/openagents-desktop` and
`packages/pylon-core`, but only a thin pure-derivation slice is actually
imported: `resolveLocalIdentityPublic` / `LocalSignerPort`
(`pylon-core/src/shared/nostr-identity.ts:9-12`) and
`DERIVATION_PROFILE_ID` / `deriveSovereignIdentityPublic` / `normalizeMnemonic`
(`pylon-core/src/wallet/spark-status.ts:32-41`). **No app composes a
`LocalSecretStore` layer, and the `SovereignIdentity` service tag is resolved
only inside the package's own tests.** `@openagentsinc/local-secret-store` has
no consumer outside `sovereign-identity` at all.

All fourteen reviewed symbols are therefore `(b)`:
`retirePlaintextCompatibility`, `verifyRemainingBackupRestore`,
`makeReadOnlyLegacyPlaintextGuard`, `SovereignIdentityInterface.hasRootSecret`,
`SovereignIdentityInterface.rootCustody`, `createIdentityRoot`, `openIdentity`,
`importConfirmedIdentity`, `ManifestStoreInterface.readRetirementReceipt`,
`deriveAndAttachPublicIdentity`, `reconcileIdentities`,
`makeBreezSparkComparisonAdapter`, `macosKeychainOwnerAttendedLayer`, and
`unimplementedPlatformSecretStoreLayer` (the last is really `(c)`: its own
docblock says the IDR-05 adapters superseded it, and each real adapter now
carries its own fail-closed default).

On the privacy question specifically, stated precisely:
`makeReadOnlyLegacyPlaintextGuard` guards **writes and deletes** during a
verification period — its `readMetadata` is "never refused" (`retire.ts:104`).
An uninstalled guard therefore cannot cause a plaintext *read* that would
otherwise be blocked. What can read the plaintext today is `readMnemonic`
(`pylon-core/src/shared/nostr-identity.ts:133`), reachable from Pylon presence
and the Desktop main process, plus any process running as that user — because
the file **is** the canonical store. The write/delete property the guard would
enforce is already true of the shipped path by construction: `writeMnemonic`
uses the exclusive-create flag `"wx"`, `createNostrIdentity` refuses over an
existing candidate, `inspectIdentityFile` refuses symlinks and group/other
readable files via `lstat` without reading bytes, and nothing in shipped code
deletes the file.

`makeBreezSparkComparisonAdapter` is the textbook masked case:
`pylon-core/src/wallet/spark-status.ts:135` is a **comment** mentioning it, and
that comment is what made it look production-called. There is no Breez SDK
dependency anywhere in the repository, and
`packages/sovereign-identity/src/boundary.test.ts` fails the build if one
appears.

**The finding worth escalating here is one level up and is not what the guard
flags**: the IDR-05 → IDR-09 platform-custody migration merged on 2026-07-20
and was never wired to a composition root, while shipped Pylon and Desktop
still hold the BIP-39 root as a `0600` plaintext file at
`~/.openagents/pylon/identity.mnemonic` — the arrangement
`docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`
explicitly rejects as a canonical store ("File mode `0600` does not give
platform secret-store protection"). Classifying the fourteen symbols
individually as dormant is accurate but understates it: they are dormant
*together*, and the thing they were built to replace is still running. I did
not check live GitHub issue state, so I cannot say whether a roadmap slice
currently tracks that wiring.

### Full Auto capacity and initiative

- **`admitConcurrentRun`** (`packages/omega-effectd/src/engine/full-auto-capacity.ts:90`)
  — **the eight-active-run cap does not depend on it and is independently
  enforced twice**: `FULL_AUTO_RUN_ACTIVE_LIMIT = 8` refuses with
  `active_run_limit_reached` at `full-auto-run-registry.ts:875` and `:898`
  (real production callers), and `full-auto-run-actions.ts:243` (same package) pre-checks with
  an HTTP 409 before minting a thread. What is inert is the *proactive per-lane
  spreading* — "never admit a run onto a `busy`, `cooling`, or `exhausted`
  lane". Consequence: up to 8 concurrent runs can land on one provider account,
  oversubscribing an exhausted or rate-limited lane. Bounded by the total cap
  and recovered reactively by `full-auto-reconcile.ts:292-311`, which rotates a
  lane after an `account_exhausted` / `rate_limited` dispatch failure. So: a
  capacity-efficiency defect, not an unbounded spend hole. It was born uncalled
  — the introducing commit `19ac593e84` adds definition, tests, and contract
  prose with no call site — and its sibling `projectFullAutoCapacityLedger` *is*
  live (`protocol/server.ts:951`), so the ledger is computed and displayed while
  the decision that would act on it is never invoked.
- **`decideFullAutoInitiative`** — absence is restrictive, not permissive:
  every branch except the last returns `hold`. But three real honesty defects
  hang off it: `selfClaimRef` is permanently `null`
  (`full-auto-mission.ts:131`), so `:243` renders "the host will record the
  self-claim" on **every** autonomy run and the host never does;
  `packages/behavior-contracts/src/openagents-apps.ts:1774` marks
  `openagents_desktop.full_auto_autonomy_initiative.v1` `state: "enforced"`
  with `blockerRefs: []` for a statement no production path implements; and
  `packages/omega-effectd/src/engine/full-auto-run-actions.ts:309` tells
  the user the run starts with
  "objective selection, host verification, plan, churn, initiative" when two of
  those five have no production implementation.
- **`IdePortableCoordinatorShape.authorizeMutation`** — the owning layer
  `makeIdePortableCoordinatorLayer` is itself never constructed, so no service
  lookup can reach the method. **Enforced elsewhere, heavily:**
  `IdePortableMutationAuthority`
  (`packages/omega-effectd/src/support/ide/portable-mutation-authority.ts`) is
  constructed at `main.ts:2495` and its `authorize`/`reauthorize` are called
  from roughly eighteen production sites, implementing the same stale-writer
  fence (permit key embeds `sessionRef`/`workContextRef`/`attachmentRef`/
  `generation`; `reauthorize` refuses on any byte difference). Cost: two
  divergent implementations of one invariant, and the model-checked one is not
  the shipped one; plus
  `specs/desktop/desktop-trust-complete-workbench.assurance-spec.md:698-712`
  lists the unreachable module as `disposition: "required"` evidence.

## (c) Superseded or dead — the property is enforced elsewhere

### `decideDesktopMediaPermission` — the shipped posture is *stricter*

`apps/openagents-desktop/src/voice-permission-policy.ts:4`. Zero real calls;
the module is imported by nothing but its own test. This was flagged to me as
the high-priority case and it is the safest one in the whole set.

The shipped handler (`apps/openagents-desktop/src/main.ts:1388-1392`, registered
at `:6620`) is:

```ts
target.setPermissionRequestHandler((_webContents, _permission, callback) => {
  callback(false)
})
```

Unconditional `false` for `media`, `microphone`, `camera`, `display-capture`,
and everything else. Navigation, `will-attach-webview`, and `setWindowOpenHandler`
are all denied at `main.ts:6552-6559`; `webPreferences` is
`contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`,
`webviewTag: false`, `webSecurity: true`. The `CLAUDE.md` deny-by-default
requirement is met.

**Wiring `decideDesktopMediaPermission` in would be a loosening, not a
tightening** — it returns `true` for microphone under three conditions. There is
no reading in which the uncalled symbol is a missing guard.

Microphone still works because it never touches Chromium: voice I/O runs in the
signed Rust helper `oa-desktop-audio` (signature + sha256 pinned,
`voice-native-helper.ts:8,42`), gated on the macOS TCC grant via
`systemPreferences.getMediaAccessStatus("microphone")` (`main.ts:2064-2070`),
failing closed to `phase:"denied"` and returning `"denied"` outright off-darwin.
The `getUserMedia` calls under `components/ai-elements/` are unimported
electron-shadcn template leftovers — no file outside that directory imports from
it — and would be denied anyway.

OpenAgents Desktop **is** shipped and auto-updating on the `rc` channel
(`package.json` `0.1.1-rc.2`; signed manifests through
`apps/oa-updates/openagents-desktop-dist/`), so this is a real binary, not a
dormant app — which is why the "stricter, not missing" conclusion matters.

Two genuine minor gaps found while checking, reported without inflation:
`setPermissionCheckHandler` is never registered anywhere (Electron's own
typings warn that the pair is needed for complete handling; practical exposure
is near-zero because the request half denies all), and the two env-gated
offscreen probe `BrowserWindow`s (`main.ts:9444`, `:9878`) do not get
`hardenSession`.

**A blind oracle worth fixing.** `apps/openagents-desktop/src/service-topology.ts:674`
asserts the live `desktop-voice-host` service composes
`decideDesktopMediaPermission`. The validator checks
`source.includes(construction) && compositionSource.includes(construction)` —
but here the composition module *is* the defining module, so the check is
satisfied by the declaration's own name. It is vacuous, and it is the **same
blind spot as the original guard bug**. Exactly three topology evidence entries
are self-referential; the other two happen to be genuinely composed, so this is
the only one where the vacuous check hides a false claim.

### `verifyNip98Authorization` — the server deliberately refuses NIP-98

`packages/pylon-core/src/shared/nostr-identity.ts:384`. Real calls: zero. Its
test call sites are inside `fakePresenceServer` mocks — it exists so tests can
assert the *client's* NIP-98 output.

The production Pylon presence API **deliberately rejects** NIP-98:
`pylon-api-routes.ts:544` detects the `Nostr` scheme and `:503-510` returns
`401 pylon_api_presence_requires_agent_token` — "a self-signed Nostr (NIP-98)
signature proves Nostr identity but is not accepted as presence authority,
because Pylon registrations are bound to the owning agent token" (commit
`9edbd9b0a1`). So there is no route this function should be protecting.

**Enforced elsewhere for the surfaces that do use NIP-98:** two independent
implementations on `nostr-effect/nip98` —
`auth/omega-nostr-session.ts:286-302` (wired at `index.ts:3783` and `:14410`,
covering the `POST /api/omega/auth/session` invariant) and
`forge-invite-policy.ts:272 verifyForgeNip98Proof` (consumed by five real route
sites). Both perform the same checks: `verifyHttpAuthEvent`, empty content,
exact `u` / `method` / `payload` tag binding, integer `created_at`, 60 s skew.

**Stale doc claim to fix:** `apps/pylon/docs/presence-registration-heartbeat.md:19-24`
states in the present tense that "Presence requests now use strict NIP-98 HTTP
auth". It describes only the client sending, and the server contradicts it.
Two later documents already say so
(`apps/openagents.com/docs/2026-06-12-provider-discovery-fields.md:42-48`,
`apps/openagents.com/AGENTS.md:241-256`).

Caveat: `@openagentsinc/pylon-core` is publishable and re-exports this through
`export *` barrels, so an out-of-repo consumer cannot be ruled out from a
worktree.

### `gateSparkLiveBalance` — vacuous; there is no balance to gate

`packages/pylon-core/src/wallet/spark-status.ts:275`. The function **has no
success path**: without both flags it returns `online_action_gated`; with both
flags it returns `deferred`. Its own test asserts exactly that.

No Spark balance is read anywhere in production. No Breez/Spark SDK appears in
any `package.json` or in `pnpm-lock.yaml`. The one live Spark path
(`openRecoveredSparkWalletStatus`) is offline identity-fingerprint matching:
`mode: "status_only"`, `sendEnabled: false`. The optional wallet control
actions in `apps/pylon/src/node/control-server.ts:75-152` are never supplied —
production spreads `retiredMoneyControlActions`, returning
`money_capability_retired`. The heartbeat `walletProbe` has no production
injection site and its type carries no balance field. Consistent with
`INVARIANTS.md:603-610`: wallet custody, payout, and settlement are not part of
the accepted MVP.

### The rest

- **`degradeStalePresence`** — the only production code that can set
  `PylonPresenceState.stale = true`; every other write sets it `false`, so
  `if (presence.stale)` at `apps/pylon/src/assignment.ts:1304` is a provably
  dead branch. Enforced elsewhere twice: the three lines immediately below it
  do the same job from `lastHeartbeatAt`, and — correctly, since a client
  self-reporting staleness is not a control — the server enforces it
  authoritatively in `inference/coding-workflow-delegation.ts:660-679`
  (5-minute window, surfacing as `target_pylon_unavailable`),
  `autopilot-work-placement-selector.ts:61-78`, and
  `operator-fleet-status-routes.ts:144-146`.
- **`withPresenceRetry`** — never wraps anything; the daemon loop has its own
  inline retry for the 401 case only. A transient 5xx costs one heartbeat
  against a 30 s interval and a 5-minute server TTL. No authority, money,
  privacy, or safety consequence. Worth noting that
  `apps/pylon/tests/presence.test.ts:784` proves a resilience property
  production does not have.
- **`evaluateLaunchReceipt`** — superseded by
  `openagents.desktop.launch_health.v1`, which ships and is wired
  (`main.ts:1892`, `:9992`, `:9999`, `:9415`) and is **stricter** in three
  ways: a 32-hex per-apply `transactionRef` that defeats stale-receipt replay;
  two-step health (renderer+provider ready **and** a full child-runtime drain);
  and an out-of-process watchdog that survives a build that never starts, with
  macOS re-verifying `codesign --verify --deep --strict`,
  `TeamIdentifier=HQWSG26L43`, `stapler validate`, and `spctl` before atomic
  restore. **But `apps/openagents-desktop/GUARANTEES.md:1084-1097` attributes
  the guarantee to `launch_receipt.v1` and "the clock-free
  `evaluateLaunchReceipt`", citing a test that proves an unreached function.**
  The guarantee holds; the named evidence does not support it. Since
  GUARANTEES.md is release-quotable, this is the highest-value documentation
  fix in the desktop set. `docs/DEPLOYMENT.md:59` repeats the attribution.
- **`adviseFullAutoRoute`** — Apple FM is structurally not an admissible Full
  Auto lane (`FULL_AUTO_LANE_POLICIES` in `full-auto-lane.ts:40-72`;
  `validateFullAutoRoutingPolicy` refuses an unknown lane as
  `lane_not_full_auto_eligible`; `main.ts:6265-6269` refuses independently),
  and the live dispatch selector takes no recommendation parameter at all.
- **`startMetaAgentAcpServerIfEnabled`** — `main.ts:6467-6483` inlines the
  identical gate using the same `isMetaAgentAcpServerEnabled` (strict `=== "1"`)
  and the same `startMetaAgentAcpServer`. Fail-safe: the uncalled wrapper means
  a listener that never binds, not one that binds unguarded. Containment when
  enabled is wrapper-independent (`assertLoopbackHost` throws on any
  non-loopback host; the default decider denies every gated tool call; the env
  override selects only a port). Cost: the gate now lives in two places, so a
  future change to the wrapper would silently not reach production.
- **`projectPartnerPayout`**, **`transitionPartnerPayout`** — sole caller
  `partner-payout-ledger-routes.ts` deleted in `b022f72748`; both routes 410
  (verified live above). No other writer of `partner_payout_ledger_entries`
  exists, and the ingress `recordPartnerPayoutForPaidEvent` is itself orphaned,
  so no row can exist to transition.
- **`treasuryRead`** — reads moved Postgres-authoritative in CFG-4; last caller
  `treasury-page-routes.ts:179` deleted in `b022f72748`. **Ops trap worth
  fixing:** `KHALA_SYNC_TREASURY_READS` is still parsed and still documented as
  a cutover control, but with zero readers a `compare` → `postgres` cutover now
  shows a clean mismatch-free result that means nothing.
- **`recordUserCreditBalanceDeltaBestEffort`** — its sole injector
  `creditBalanceProjectionRecorderForEnv` was deleted, as was every producer
  named in its header. Users still see correct balances: `GET
  /api/agents/me/balance` reads `agent_balances` directly, and nothing
  subscribes to the projection (`credit_balance` has zero hits in the mobile and
  desktop apps).
- **`sellPricePerMtok`** — production pricing is `priceRequest`
  (`pricing.ts:563`), live via `served-tokens-recorder.ts:163` and
  `cost-estimate.ts:116`. `buildModelCatalog` recomputes the same formula
  inline; the only "reference" is that inline comment. Duplicated-but-agreeing
  math, drift risk only.
- **`cardCreditGrantContextRef`**, **`parseCardCreditGrantContextRef`** — not
  "provenance lost": the card → credit grant path no longer exists. All three
  real callers were deleted in `b022f72748`; `/checkout` and `/api/billing/`
  410.
- **`omegaNostrSelfProvisionAbuseBound`** — reads alarming, is not. It is a
  pure arithmetic **summarizer** that exists so a test can assert the
  documented worst-case figures do not drift. The real abuse bound is armed and
  enforced: `env-production.yaml:243-246` sets `ENABLED=1`,
  `GLOBAL_DAILY_LIMIT=20`, `IP_HOURLY_LIMIT=30`,
  `DAILY_TOKEN_CEILING=50000000`; the route is mounted at `index.ts:14410`; and
  the handler calls `reserveOmegaNostrSelfProvision` (real caller
  `index.ts:3800`), returning 429. The spend ceiling is separately enforced at
  `provider-account-service-routes.ts:414-419`.
- **`makePremiumAccessGate`** — superseded, with a **factually wrong header
  comment**: `inference-premium-allowlist.ts:21` and `:286-287` claim the
  chat-completions route calls it. `ChatCompletionsDeps` has no premium field
  at all. Enforced by stricter controls instead — the only public model is
  `openagents/khala` and the route rejects anything else at
  `chat-completions-routes.ts:2739-2749`.
- **`ALL_LANES_UNARMED`** — a test fixture that happens to equal the empty-env
  result. Production computes the equivalent independently in
  `resolveSupplyLaneArming` (`model-serving-policy.ts:673-698`), field by field
  from `isPresent(env.X)`, and never references the constant.
- **`canReadScopeV1`** — see the privacy section below.
- **`adjudicateDoneCondition`**, **`canResolveDecision`**,
  **`createShipReceiptLedger`**, **`verifyQualifiedContributorMethodologyDocument`**,
  **`ARTANIS_OWNER_PROMOTION_AUTHORITY_RECEIPT_REF`**,
  **`solClaimLedgerRepositoryFilter`** — checked and confirmed test-only, all
  low-stakes; no live control depends on any of them.

## Privacy, read-scope, and prompt-injection defenses

Grouped by subject rather than by class, because the question they answer
together — "can one owner see another's rows, and can untrusted text reach a
model prompt?" — is the same question. Each entry states its own class. **The
answer to both is no.**

### `canReadScopeV1` — superseded by a broader, stricter, wired gate

`packages/khala-sync-server/src/fleet-projection.ts:140`. Zero real callers;
the only non-test mention is the docstring at `:58`.

The serving path was traced end to end rather than trusted. The successor
`resolveScopeRead` (`packages/khala-sync-server/src/scope-auth.ts:169`) **is**
wired into every Sync read surface:
`apps/openagents.com/workers/api/src/khala-sync-route-wiring.ts:139` builds it
and injects it at `:160`, `:166`, `:171`, `:182`; that module is imported by
`index.ts:923`; and each route runs the gate **before** any data read
(`khala-sync-log-routes.ts:393`, `khala-sync-bootstrap-routes.ts:314`,
`khala-sync-cvr-routes.ts:218`).

Its `fleet_run` arm is logically identical to the flagged function — both
require `owner !== null && owner === userId`. The successor is strictly
broader and stricter: full scope taxonomy, `scope.public.*` the only
anonymous-readable kind and checked first, every other kind denied before any
capability callback when `userId` is undefined, and an unknown kind failing
closed with `unknown_scope`.

The write side is genuinely live (`khala-sync-fleet-projection.ts:120`), so
real `fleet_run` scopes exist in production — and they are gated.
`read-service.ts` is a raw substrate with no auth by design; authorization sits
at the route layer above it. **Consequence: none.**

### `quoteUntrustedCommunityContent` and `admitIndependentVerification` — a silent regression, currently latent

These two are the most interesting finding in the set, because they are the
only ones that **had** real production callers and lost them.

`apps/openagents-mobile/src/workroom/issue31-community-read-model.ts` called
`quoteUntrustedCommunityContent` at `:624` and `:626` and imported
`admitIndependentVerification` at `:29`. Commit `017ae3dedb` ("Rebuild the
mobile app on arcade patterns, in plain React Native", 2026-07-27) deleted that
file. Verified directly:

```
$ git show 017ae3dedb^:apps/openagents-mobile/src/workroom/issue31-community-read-model.ts | grep -n "quoteUntrustedCommunityContent"
40:  quoteUntrustedCommunityContent,
624:    return quoteUntrustedCommunityContent(input);
626:    return quoteUntrustedCommunityContent({
$ git show 017ae3dedb:apps/openagents-mobile/src/workroom/issue31-community-read-model.ts
fatal: path ... does not exist in '017ae3dedb'
```

`quoteUntrustedCommunityContent` is the prompt-injection fence: the only way to
obtain a quoted untrusted-community value. `admitIndependentVerification`
refuses self-dealing verification — same key, burned key, and two agent keys
resolving to the same operator.

**Does untrusted community content reach a model prompt today? No.**
`packages/sarah` contains no prompt assembly and makes no model calls; mobile
`src/` has no community code left; and the one real model prompt
(`apps/sarah-livekit-agent/src/agent.ts:286`) uses the **static**
`COMMUNITY_INSTRUCTIONS` at `:74-80`, with community members reaching the model
as realtime audio rather than as text records. The defended path does not
currently exist, so this is **latent, not live** — classified `(b)`.

Two things follow. The comment at `community/untrusted.ts:173-175` — "Until the
mobile community room subscribed, nothing could reach Sarah from this room and
the fence had zero call sites. **That is no longer true**" — was accurate when
written and is now false. And if the community workroom is rebuilt, both
defenses must be re-wired; nothing in the tree will notice if they are not,
beyond this guard.

One commit orphaned the whole cluster: these two plus
`buildCommunitySarahContext` (described in-source as "the single admitted
door", now with zero references anywhere including its own test), the community
ledger fold, LBR decoding, and the XP surfaces. The guard's comment-blindness
is exactly why it went unnoticed — the deleted read model left *comments*
naming these functions in sibling files.

### `approveBacklogFaucetForPublication` — the "not bypassable" claim is false

`apps/openagents.com/docs/2026-06-12-backlog-faucet-contract.md:82-88` states:

> `approved_for_publication` exists only through
> `approveBacklogFaucetForPublication`, which requires a
> `BacklogFaucetOperatorApproval`: a typed `operator.*` ref, an ISO approval
> instant, and an integer spend cap covering the filing budget.

The live publisher is `apps/openagents.com/scripts/backlog-faucet-list.ts`,
which calls `buildBacklogWorkRequestFiling` (`:91`) and POSTs straight to
`/api/forum/work-requests` (`:97`), never touching the state machine.
`docs/labor/2026-06-14-p5-backlog-faucet-closeout.md` records three real
listings through that path, one settled with real sats.

Stated without inflation: this is **not a regression** — the gate appears never
to have guarded the live path, and the contract document itself notes no
operator HTTP route was added. So the classification is `(b)`, a dormant state
machine, not `(a)`, an inert control. The residue is that the doc sentence is
untrue, and that the live generic route authorizes any active registered agent
token with no operator role and no budget check while publishing to the public
Forum and the live Nostr relay. Money is separately backstopped at
quote-acceptance by `labor-escrow.ts`, so the exposure is unapproved public
listings, not fund loss. Whether that route needs an operator/spend gate is a
separate product decision, not a finding of this triage.

### `stripToolMetadata` — the claim is vacuous rather than violated

`CLAUDE.md` says "`tool_metadata` is stripped on public export". There is no
public-export path: `packages/product-spec/src/cli.ts` has only `validate`,
`validate-trace`, `digest`, and `init`, and no build script calls the function.
The two apparent "public" references (`observatory-public-trace.ts:241`,
`observer-routes.ts:236`) are hardcoded GitHub URL strings that never read the
file bytes.

All 21 specs do carry `tool_metadata` blocks; they were reviewed and none
contains a secret, PII, or a price figure. The decisive point is that `specs/`
lives in a public repository, so those blocks are already public on commit,
with or without this function. The present-tense claim implies an active
control that has no caller and should be reworded to a conditional.

### `makeDpopCapabilityProofVerifier` — dormant and fails closed

No DPoP surface is served. The harness MCP pilot uses a plain bearer with an
explicit `TODO(ENV-2 #8780)` at `harness-mcp-server.ts:28-31`, matching what
`CLAUDE.md` already says. The broker fails **closed**: a thumbprint-bound lease
with no verifier throws `proof_required`
(`capability-broker.ts:850-857`), and `clientKeyThumbprint` has zero
occurrences under `apps/`, so no lease is ever key-bound. The gate is
unreachable rather than bypassed. Docs are honest here.

### `resolveModelPreference` — INVARIANTS names an orphaned first draft

`apps/openagents.com/INVARIANTS.md:809` states "The resolution
(`resolveModelPreference`) is typed and never silently substitutes…" and cites
`model-preference-store.test.ts` as its regression coverage — a unit test, not
a route. The live `/api/mobile/model-preference` handler
(`index.ts:5511-5573`, registered at `:14704`) calls
**`resolveExecutionTargetPreference`** at `:5570`, a near-identical sibling
generalized to execution targets. The named behaviour is enforced; the
invariant names the wrong function. No defect; the reference should be
repointed.

## Documentation and contract claims to correct

None of these is an enforcement gap. Each is a place where a document, comment,
contract, invariant, or oracle names an artifact that cannot run, which is the
residue this triage actually found. Ordered by how quotable the claim is.

1. `apps/openagents-desktop/GUARANTEES.md:1084-1097` credits `launch_receipt.v1`
   and "the clock-free `evaluateLaunchReceipt`" for a guarantee that
   `launch_health.v1` plus the native watchdog delivers. Release-quotable.
   `docs/DEPLOYMENT.md:59` repeats it.
2. `packages/behavior-contracts/src/openagents-apps.ts:1774` marks
   `openagents_desktop.full_auto_autonomy_initiative.v1` `state: "enforced"`
   with `blockerRefs: []` for a self-claim no production path writes; the Full
   Auto mission prompt renders "the host will record the self-claim" on every
   autonomy run.
3. `apps/openagents.com/docs/2026-06-12-backlog-faucet-contract.md:82-88` — the
   spend gate is described as "not bypassable"; the live publisher bypasses it.
4. `apps/pylon/docs/presence-registration-heartbeat.md:19-24` — "Presence
   requests now use strict NIP-98 HTTP auth", contradicted by the server, which
   returns `401 pylon_api_presence_requires_agent_token`. Same document's
   "stale presence degrades to explicit blocker refs" names two ref strings
   production never emits.
5. `apps/openagents.com/workers/api/src/cloud/cloud-coding-session-routes.ts:1622-1624`
   claims two sibling metering hooks are wired (neither is) and instructs a
   future implementer to call `creditBalanceProjectionRecorderForEnv`, a
   function deleted by `b022f72748` whose only remaining occurrence is that
   comment.
6. `apps/openagents-desktop/src/service-topology.ts:674` asserts the live
   `desktop-voice-host` service composes `decideDesktopMediaPermission`. The
   validator's check is satisfied by the declaration's own name, so it is
   vacuous — **the same blind spot as the guard bug this triage came from.**
   Three topology evidence entries are self-referential; this is the only one
   where the vacuous check hides a false claim.
7. `packages/sarah/src/community/untrusted.ts:173-175` — "the fence had zero
   call sites. That is no longer true" was accurate when written and is false
   since `017ae3dedb`.
8. `apps/openagents.com/INVARIANTS.md:809` names `resolveModelPreference` where
   the live handler calls `resolveExecutionTargetPreference`.
9. `apps/openagents.com/workers/api/src/inference/inference-premium-allowlist.ts:21,286-287`
   claims the chat-completions route calls `makePremiumAccessGate`; there is no
   premium field on `ChatCompletionsDeps`.
10. `apps/openagents.com/workers/api/src/inference/served-tokens-recorder.ts:154`
    — "the metering hook already charges the customer"; it does not.
11. `apps/openagents.com/workers/api/src/khala-sync-user-credit-balance.ts`
    header (`:14-18`) describes a
    producer chain whose every member was deleted.
12. `CLAUDE.md`'s "`tool_metadata` is stripped on public export" — vacuous; no
    export path exists and `specs/` is already public.
13. `KHALA_SYNC_TREASURY_READS` is still parsed and still documented as a
    cutover control, but has zero readers, so a `compare` → `postgres` cutover
    now shows a clean mismatch-free result that means nothing.
14. `packages/omega-effectd/src/engine/full-auto-run-actions.ts:309` tells the user a
    run starts with "objective selection, host verification, plan, churn,
    initiative"; two of those five have no production implementation.

## Two gaps found in passing, neither a triage finding

- **Electron permission handling is half a pair.**
  `setPermissionCheckHandler` is not registered anywhere in
  `apps/openagents-desktop`; Electron's own typings warn that both halves are
  needed for complete handling. Practical exposure is near-zero because the
  request half denies everything. Separately, the two env-gated offscreen probe
  `BrowserWindow`s (`main.ts:9444`, `:9878`) do not receive `hardenSession`.
- **Agent Computer VM rental is unbilled**, with the promise
  `autopilot.cloud_coding_sessions.v1` correctly `red` and the inference tokens
  inside the microVM correctly recorded. Making it billable needs an
  end-of-session usage callback that was never built, not a wiring line.

## What I could not determine

- **Out-of-repo consumers of published packages.**
  `@openagentsinc/pylon-core` and `@openagentsinc/sovereign-identity` are
  publishable and re-export several of these symbols through `export *`
  barrels. "No in-repo caller" is established; "no caller anywhere" is not.
- **Whether any roadmap slice tracks wiring the IDR-05 custody migration** to a
  composition root. Live GitHub issue state and `docs/sol/MASTER_ROADMAP.md`
  were outside what was read.
- **Whether the Sync-projected IDE mutation authority has a lag window** versus
  a relay-side generation bump — that needs relay code outside this worktree.
- **The live behaviour of `POST /api/keys/free` and
  `POST /v1/chat/completions`.** Both would need a mutating or authenticated
  request. The `/api/keys/free` conclusion rests on the GET/OPTIONS 404 plus the
  405 control plus the deleted route module; the `503
  platform_funding_unavailable` conclusion is source-level only.
- **Whether the remaining ~110 symbols of the 171** outside the authority /
  money / privacy / safety class contain anything of this severity. They were
  scanned by name and path, not read.
- **Whether `POST /api/forum/work-requests` should carry an operator or budget
  gate.** The finding here is only that the backlog-faucet document describes a
  gate the live path does not use. What the route's authorization *ought* to be
  is a product decision outside this triage.
- **Whether the env comment's claim that the Omega self-provision token ceiling
  is also enforced on `/v1/chat/completions` holds.** It is consistent with the
  code found (`checkSelfProvisionedDailyCeiling` is wired at `index.ts:16247`
  and reads the same UTC-day `token_usage_events` window), but was not
  independently proven against a live request.

## Recommended disposition

Nothing here needs an emergency fix. In rough order of value:

1. Correct the fourteen claims listed above. That is the actual damage.
2. Fix the vacuous self-referential check in `service-topology.ts` — it is the
   guard bug wearing a different hat, and it will keep laundering false
   composition claims until it is fixed.
3. Decide **wire or delete** for the small number with a real (bounded) runtime
   consequence: `admitConcurrentRun` (its ledger is already computed and shown,
   only the decision is missing) and the community fence pair
   (`quoteUntrustedCommunityContent`, `admitIndependentVerification`), which
   must be re-wired before any community-workroom rebuild.
4. Escalate the sovereign-identity cluster as one item, not fourteen: the
   IDR-05 → IDR-09 custody migration is complete, tested, fail-closed, and
   connected to nothing, while shipped Pylon and Desktop still hold the BIP-39
   root as a `0600` plaintext file.
5. Delete the rest as VP-1 and supersession cleanup. Per
   [`uncalled-production-symbols.md`](uncalled-production-symbols.md), deleting
   a symbol together with its test is a legitimate resolution — behaviour
   nothing reaches is not an asset, and Git keeps it. Each deletion shrinks the
   ledger, which is the only direction it may move.
