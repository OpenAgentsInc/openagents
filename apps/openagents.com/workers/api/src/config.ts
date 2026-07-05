import { Effect, Layer, Redacted, Schema as S } from 'effect'
import * as Context from 'effect/Context'

export type OpenAgentsWorkerConfigEnv = Readonly<{
  ARTANIS_FLEET_OVERSEER_ENABLED?: string | undefined
  ARTANIS_SCHEDULED_RUNNER_ENABLED?: string | undefined
  // Compose-and-list marketplace MVP flag (EPIC #5510, #5515). Default OFF: the
  // `/api/public/marketplace/composed-products` listing surface is INERT (empty
  // store) on the live Worker until the marketplace build lands. Set
  // "true"/"1"/"on" to arm the (still planned/inert) surface.
  MARKETPLACE_COMPOSE_AND_LIST_ENABLED?: string | undefined
  // Autopilot all-in-one composed-run scaffold flag (EPIC #5510, #5519). Default
  // OFF: the `/api/public/autopilot/composed-runs` listing surface is INERT
  // (empty store) on the live Worker. Set "true"/"1"/"on" to arm the (still
  // planned/inert) surface. The composition makes no live/billable claim and the
  // autopilot.all_in_one_business_system.v1 + cloud.primitives_suite.v1 promises
  // stay planned regardless.
  AUTOPILOT_COMPOSED_RUN_ENABLED?: string | undefined
  // Agentic labor-product flow flag (promise autopilot.agentic_labor_products.v1,
  // yellow). Default OFF: the `/api/public/autopilot/labor-products` listing
  // surface is INERT (empty store) and the settlement seam never settles. Set
  // "true"/"1"/"on" to arm the (still yellow/inert) surface. The flow makes no
  // live-sale claim and the promise stays yellow regardless; a green flip stays
  // receipt-first and owner-signed with a dereferenceable settlement receipt.
  AGENTIC_LABOR_PRODUCTS_ENABLED?: string | undefined
  // Self-serve control-center fanout flag (promise
  // autopilot.control_center_fanout_marketplace.v1, yellow). Default OFF: the
  // `/api/public/autopilot/self-serve-fanout` listing surface is INERT (empty
  // store) and the dispatch seam (dispatchSelfServeFanout) lists nothing. Set
  // "true"/"1"/"on" to arm the (still yellow/inert) surface. The plan makes no
  // broad-live-marketplace claim and the promise stays yellow regardless; the
  // capability clears the self-serve planner/route blocker and the
  // plugin-marketplace-beyond-code-task planner blocker. A green flip stays
  // receipt-first and owner-signed with a dereferenceable settlement receipt.
  SELF_SERVE_FANOUT_ENABLED?: string | undefined
  // Labor self-serve earning payout flag (promise
  // provider.compliant_usage_labor.v1, yellow). Default OFF: the
  // `/api/public/labor-earnings/payout` route is INERT. When false, the
  // dispatch seam returns disabled and lists/moves nothing.
  LABOR_SELF_SERVE_PAYOUT_ENABLED?: string | undefined
  // Signature usage-metering surface flag (EPIC #5523 / DE-6 #5529; promise
  // marketplace.signature_monetization.v1, red). Default OFF: the
  // `/api/public/markets/signature-monetization/metering` surface is INERT
  // (empty store) on the live Worker. Set "true"/"1"/"on" to arm the (still
  // red/inert) surface. Metering clears only the usage-metering blocker; the
  // settlement blocker stays owner-gated and the promise stays red regardless.
  SIGNATURE_USAGE_METERING_ENABLED?: string | undefined
  // Voice-session transcript ingestion endpoint flag (EPIC #5523 / DE-7 #5530;
  // promise mobile.voice_session_evidence_transcript_ingest.v1, red). Default
  // OFF: the `/api/mobile/voice-sessions/ingest` route is INERT on the live
  // Worker (returns an honest inert/red payload and never runs the ingest
  // core). Set "true"/"1"/"on" to arm the endpoint so it decodes
  // already-transcribed, redacted, ref-only segments and returns an
  // approval-gated program-input proposal. Arming clears ONLY the
  // ingestion-endpoint blocker; STT vendor + approval UI stay owner/product-
  // gated and the promise stays red regardless.
  VOICE_PROGRAM_INGEST_ENABLED?: string | undefined
  // Site page form-capture wiring flag (#5523 / DE-9 #5532; promise
  // autopilot_sites.native_email_sequences.v1, yellow). Default OFF: the public
  // capture route (POST /api/sites/forms/:formId/submit) stays unmounted and
  // the omni dispatch chain falls through exactly as today. Set "true"/"1"/"on"
  // to mount it — the route resolves a page's FormCaptureSpec from the active
  // site version metadata via site-form-spec-registry and persists leads via
  // the native-lists addSubscriber sink. Arming clears ONLY the
  // route-unmounted blocker; the customer UI, send service, and deliverability
  // stay owner/product-gated and the promise stays yellow.
  SITE_FORM_CAPTURE_ENABLED?: string | undefined
  // Native email-sequence send-service flag (promise
  // autopilot_sites.native_email_sequences.v1, yellow). Default OFF: authored
  // sequence sends still take the dry-run/skipped path and do not call a sender.
  // Set "true"/"1"/"on" only after the Cloudflare Email binding and authenticated
  // sender domain are configured. The promise remains yellow until live
  // deliverability receipts and owner sign-off exist.
  EMAIL_SEQUENCE_SEND_ENABLED?: string | undefined
  EMAIL_SEQUENCE_FROM_EMAIL?: string | undefined
  EMAIL_SEQUENCE_REPLY_TO_EMAIL?: string | undefined
  // Mobile workroom approval projection flag (promise
  // mobile.voice_approval_companion.v1, yellow). Default OFF: the
  // `/api/mobile/workroom-approval-projection` route is INERT (empty store) on
  // the live Worker. Set "true"/"1"/"on" to arm the read-only projection over
  // an injected authorized store. It clears only the mobile-projection blocker;
  // voice-command approval receipts + cross-device sync stay open and the
  // promise stays yellow regardless.
  MOBILE_WORKROOM_APPROVAL_PROJECTION_ENABLED?: string | undefined
  // Omni client-delivery business-object projection flag (DE-9 / EPIC #5532;
  // promise workrooms.omni_client_delivery_workrooms.v1, yellow). Default OFF:
  // the `/api/public/omni/client-delivery-projection` route is INERT (empty
  // store) on the live Worker. Set "true"/"1"/"on" to arm the read-only
  // delivery-plan projection over an injected store. It clears only the missing
  // read-only delivery-projection blocker; the live integration, owner sign-off,
  // and closeout receipt stay owner-gated and the promise stays yellow.
  OMNI_CLIENT_DELIVERY_PROJECTION_ENABLED?: string | undefined
  // Pylon multi-earning-node projection flag (EPIC #5523 / DE-4 #5527; promise
  // pylon.v0_3_multi_earning_node.v1, red). Default OFF: the
  // `/api/public/pylon/multi-earning-node` surface is INERT (empty store) on
  // the live Worker. Set "true"/"1"/"on" to arm the (still red/inert) surface.
  // The projection clears only blocker.product_promises.safe_public_projection_missing;
  // the install/receipt/settlement blockers stay owner-gated and the promise
  // stays red regardless.
  PYLON_MULTI_EARNING_PROJECTION_ENABLED?: string | undefined
  // Inference gateway feature flag (EPIC #5474, #5476). Default OFF: the
  // `/v1/chat/completions` route is inert on the live Worker until the
  // inference build lands. Set "true"/"1"/"on" to enable.
  INFERENCE_GATEWAY_ENABLED?: string | undefined
  // Owner balance-gate EXEMPTION feature flag (issue #6180). Default OFF: the
  // `/v1/chat/completions` balance gate is unchanged (a zero-balance key 402s),
  // so Khala stays a paid product for the public. Set "true"/"1"/"on" to ARM the
  // exemption: an EXEMPT verified owner (the owner/admin-granted
  // `inference_operator_exemption` store) may then call our OWN non-premium /
  // own-infra lanes (e.g. `openagents/khala` on the hourly Hydralisk box) with a
  // zero balance, recorded as `operator_credit` (zero credit debit + receipt, no
  // referral). A PREMIUM model (`claude`/`unknown`) is NEVER exempt; non-exempt
  // keys still 402. Granting is an owner/admin action; no owner id/token is ever
  // logged.
  INFERENCE_OPERATOR_EXEMPTION_ENABLED?: string | undefined
  // Internal/ops account demand-attribution allowlist (#6298 follow-up). A
  // comma-separated list of account refs (e.g. `agent:user_...`) whose inference
  // traffic is auto-classified `demand_kind=internal` REGARDLESS of request
  // headers, so our own dogfood (heartbeat / canary / Terminal-Bench, all on one
  // ops key) never pollutes the external trace corpus (`agent_traces`) or the
  // demand ledger (`token_usage_events`) — without each caller having to send a
  // header. Parsed once into a set; a header-less request from a listed account
  // defaults `demand_source` to `internal_account`, while a specific internal-
  // source header (e.g. `harbor_terminal_bench`) is preserved (never
  // downgraded). Non-listed accounts are unaffected. Unset/blank => empty set =>
  // pure no-op (everything resolves exactly as before). The ops account ref is
  // NOT hardcoded in source; it lives in the worker `vars`.
  INFERENCE_INTERNAL_ACCOUNT_REFS?: string | undefined
  // AES-GCM key for owner-scoped ChatGPT/Codex refresh-token custody (#8198).
  // Set as a Worker secret/var containing exactly 32 bytes encoded as base64 or
  // base64url. Missing/malformed values fail provider-account materialization
  // closed with a typed error; raw refresh tokens must never fall back to KV.
  PROVIDER_TOKEN_CUSTODY_AES_KEY_B64?: string | undefined
  PROVIDER_TOKEN_CUSTODY_AES_KEY_ID?: string | undefined
  // Khala FREE API MODE feature flag (issue #6228, EPIC #5474). Default OFF: the
  // self-serve free-key mint endpoint (`POST /api/keys/free`) is inert and the
  // `/v1/chat/completions` balance gate is unchanged (a zero-balance key 402s),
  // so Khala stays paid for funded keys. Set "true"/"1"/"on" to ARM free mode:
  // anyone can mint a rate-limited free `oa_agent_` key, and a free-tier key may
  // then call the single public model `openagents/khala` (own-infra GPT-OSS /
  // Gemini Flash) with a zero balance, WITHIN a per-key daily quota (request +
  // token caps). Free usage is still receipt-first metered (zero-debit). Beyond
  // the quota, or for premium lanes, credits/budget are still required (the
  // existing 402 path). A PREMIUM model is NEVER free; minting is per-IP
  // rate-limited (no unbounded key minting); no token/IP is ever logged.
  INFERENCE_FREE_TIER_ENABLED?: string | undefined
  // Owner-tunable free-tier daily quota overrides (issue #6232,
  // docs/inference/2026-06-25-khala-cost-model-and-analytics.md). Each is a
  // positive-integer string; absent / non-numeric / <= 0 falls back to the
  // compiled default (FREE_TIER_MAX_REQUESTS_PER_DAY=2000,
  // FREE_TIER_MAX_TOKENS_PER_DAY=2500000 in inference-free-tier-key.ts). Read by
  // resolveFreeTierQuota and threaded through the gate, the zero-debit metering
  // wrapper, the mint response, and the public model catalog, so the owner can
  // raise or lower the quota without a code deploy.
  FREE_TIER_MAX_REQUESTS_PER_DAY?: string | undefined
  FREE_TIER_MAX_TOKENS_PER_DAY?: string | undefined
  // Owner-tunable per-IP daily free-key MINT ceiling (AAR 2026-06-25). A
  // positive-integer string; absent / non-numeric / <= 0 falls back to the
  // compiled default (FREE_KEY_MAX_MINTS_PER_IP_PER_DAY=200 in
  // inference-free-tier-key.ts). Read by resolveFreeKeyMintCap in the
  // `POST /api/keys/free` route so the cap can be raised without a code deploy if
  // ops/canaries need fresh keys during an incident.
  FREE_KEY_MAX_MINTS_PER_IP_PER_DAY?: string | undefined
  // Durable-stream resumable inference feature flag (durable-stream Rank-1,
  // #6058, EPIC #6056). Owner-armed in production/staging Wrangler config as of
  // 2026-07-05 because Khala MCP coding assignments (`khala.request`,
  // `khala.spawn`, `khala.resume`, `khala.status`) depend on the durable read
  // URL for caller-owned Pylon/Codex resume/status. If unset, the gateway remains
  // fail-safe non-durable pass-through. When set "true"/"1"/"on", the DO binding
  // `INFERENCE_DURABLE_STREAM` must also be wired; metering still settles
  // EXACTLY ONCE on the real upstream EOF and NEVER on a resume/replay read.
  INFERENCE_DURABLE_STREAM_ENABLED?: string | undefined
  // Typed component-channel feature flag (Khala, EPIC #6123, issue #6127).
  // Default OFF: the `/v1/chat/completions` `oa.component` SSE channel is inert,
  // so the gateway is byte-for-byte today's text-only stream and standard OpenAI
  // clients are unaffected. Set "true"/"1"/"on" to ARM the gateway flag; even
  // then the channel only activates for a request that explicitly opts in (the
  // `x-oa-component-channel: on` header or an `oa_component_channel: true` body
  // field) AND targets a Khala model — so the default response shape never
  // changes. The catalog is the CLOSED v1 set; props are validated with Effect
  // Schema (one bounded repair turn, then drop) and the provider-identity
  // backstop is honored on the structured channel.
  KHALA_COMPONENT_CHANNEL_ENABLED?: string | undefined
  // Cross-app trace emission feature flag (openagents #6214, epic #6206).
  // Default OFF: a completed Khala chat-completions session is NEVER emitted as
  // an ATIF trace. Set "true"/"1"/"on" to ARM emission; even then a session is
  // emitted as a shareable `/trace/{uuid}` ONLY for a request that explicitly
  // opts in (the `x-oa-emit-trace: on` header or an `oa_emit_trace: true` body
  // field). The emitter persists through the SAME D1 trace store as the
  // `POST /api/traces` ingest and reuses its validator + public-safety tripwire
  // (never bypassed); the emitted model id is the gateway projection
  // `openagents/khala`, never a raw backend.
  KHALA_CHAT_TRACE_EMIT_ENABLED?: string | undefined
  // DEFAULT-ON free-tier trace capture flag (openagents #6293, epic #6206).
  // SEPARATE from the master KHALA_CHAT_TRACE_EMIT_ENABLED kill-switch so the
  // flip can be staged independently. DEFAULT OFF: with this off, a completed
  // Khala chat session is captured ONLY for a request that explicitly opts in
  // (today's behaviour). Set "true"/"1"/"on" to ARM default-on capture: a
  // free-tier-and-not-paid-privacy completion is then captured WITHOUT an opt-in
  // — REDACTED (the primary scrubber runs before the public-safety tripwire) and
  // PRIVATE-BY-DEFAULT (stored `owner_only`, not even link-reachable until the
  // owner opts into sharing). The master flag must ALSO be on; a paid-privacy /
  // confidential-compute caller is NEVER captured (#6295). DO NOT enable in prod
  // until the redaction tests are confirmed and the migration is applied — the
  // coordinator deploys via the migrations-safe path and flips this last.
  KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT?: string | undefined
  // Confidential-compute capture opt-OUT flag (openagents #6295, child of
  // #6293). DEFAULT OFF. When "true"/"1"/"on", the deployment runs in
  // confidential-compute mode and EVERY caller is treated as paying for privacy
  // — NOTHING is auto-captured, regardless of the capture-default flag or the
  // per-account privacy entitlement. The explicit deployment-wide exclusion.
  INFERENCE_CONFIDENTIAL_COMPUTE_ENABLED?: string | undefined
  // Khala Code paid-plan purchase seam flag (khala_code.free_paid_plans.v1,
  // #7966). DEFAULT OFF, fail-closed: while unarmed the purchase route returns
  // 503 khala_code_paid_plans_not_enabled and grants nothing, and the public
  // plan catalog reports the paid plan as not purchasable. Arming is an owner
  // decision; when armed, payment still fails closed unless the owner has
  // provided Stripe price / Lightning sats config for the selected rail.
  KHALA_CODE_PAID_PLANS_ENABLED?: string | undefined
  // Owner-approved Stripe Price id for the Khala Code paid-private-data plan.
  // Absent => the Stripe/card purchase rail returns a typed 503 and grants no
  // entitlement even if KHALA_CODE_PAID_PLANS_ENABLED is armed.
  KHALA_CODE_PAID_PLAN_STRIPE_PRICE_ID?: string | undefined
  KHALA_CODE_PAID_PLAN_STRIPE_SUCCESS_URL?: string | undefined
  KHALA_CODE_PAID_PLAN_STRIPE_CANCEL_URL?: string | undefined
  // Owner-approved Lightning price for the Khala Code paid-private-data plan,
  // in integer sats. Absent/invalid => the Spark/MPP Lightning purchase rail
  // returns a typed 503 and grants no entitlement.
  KHALA_CODE_PAID_PLAN_PRICE_SATS?: string | undefined
  // Async acceptance-verification DISPATCH feature flag (Khala, EPIC #6017).
  // Default OFF: the gateway does NOT enqueue out-of-Worker verification jobs for
  // executable khala-code artifacts until a node-side runner host (Pylon /
  // oa-workroomd sandbox / Cloud Run) is deployed. With this off the honest
  // `unverified` downgrade stands. Set "true"/"1"/"on" to arm enqueue.
  KHALA_ACCEPTANCE_DISPATCH_ENABLED?: string | undefined
  // The runner-callback bearer token. A node-side runner posts its executed
  // `AcceptanceVerdict` to `/v1/inference/acceptance-verdicts` with this token;
  // the gateway rejects any verdict that does not present it. Worker secret; never
  // committed/logged. Absent => the callback is closed (every verdict rejected).
  ACCEPTANCE_VERDICT_CALLBACK_TOKEN?: string | undefined
  // Machine-payable Khala endpoint feature flag (EPIC #6049). Default OFF: the
  // `/mpp/v1/chat/completions` 402-gated endpoint is INERT on the live Worker
  // until launch. Set "true"/"1"/"on" to arm. The endpoint is ALSO fail-safe on
  // a missing Stripe key: with this off OR no STRIPE_API_KEY it returns "not
  // configured" and never constructs a charge. The discovery surfaces
  // (`/llms.txt`, `/agents.md`, `/ai.md`, `/skill.md`) are unconditional and do
  // NOT depend on this flag.
  KHALA_MPP_ENABLED?: string | undefined
  // The Stripe Directory network profile id (`profile_…`) enabling the card/SPT
  // machine-payment rail for the MPP endpoint. Absent => the MPP endpoint is
  // crypto-only (USDC via x402/MPP); the crypto rail does not need it. This is a
  // PUBLIC directory identifier (how the public Stripe Directory references the
  // business), so it ships as a committed Worker `var` in wrangler.jsonc — NOT a
  // secret. It only NAMES the card rail; it never arms charges on its own (the
  // endpoint stays inert until KHALA_MPP_ENABLED + a STRIPE_API_KEY secret).
  STRIPE_MPP_NETWORK_PROFILE_ID?: string | undefined
  // The Stripe SECRET API key (also used by the card-billing surface in
  // stripe-billing.ts via the structurally-compatible StripeBillingEnv). The MPP
  // endpoint reads it to create/verify machine-payment PaymentIntents against the
  // Stripe REST API. Worker secret; never committed/logged. Absent => the MPP
  // endpoint is fail-safe inert and never constructs a charge.
  STRIPE_API_KEY?: string | undefined
  // The MPP/Payment-Auth challenge-binding signing secret (EPIC #6049, defect B).
  // The 402 challenge `id` is HMAC-SHA256(secret, canonical challenge fields), so
  // the Worker verifies a retry credential STATELESSLY (draft-httpauth-payment-00
  // §5.1.3). Worker SECRET; never committed/logged. Set via
  //   wrangler secret put KHALA_MPP_SIGNING_SECRET
  // Absent => the MPP endpoint is fail-safe inert and never issues a challenge or
  // verifies a credential (alongside KHALA_MPP_ENABLED + STRIPE_API_KEY).
  KHALA_MPP_SIGNING_SECRET?: string | undefined
  // The Lightning rail feature flag for the MPP endpoint (EPIC #6049,
  // draft-lightning-charge-00). Default OFF: the `/mpp/v1/chat/completions` 402
  // does NOT offer a Lightning charge until this is armed. Bitcoin-first: when
  // armed AND a working BOLT11 invoice issuer is present (the MDK wallet binding
  // — MDK_CHECKOUT_ROUTE_URL + MDK_CHECKOUT_ROUTE_SECRET/MDK_ACCESS_TOKEN, or the
  // self-hosted MDK_SIDECAR), the Lightning offer is surfaced FIRST in the 402
  // and the discovery doc. HONESTY GATE: with the flag off OR no invoice issuer,
  // the Lightning rail is not advertised (we never offer a rail we cannot
  // fulfill). The crypto/card rails are unaffected. Set "1"/"true"/"yes"/"on".
  KHALA_MPP_LIGHTNING_ENABLED?: string | undefined
  // Cloud primitive scaffold feature flags (EPIC #5510, #5516/#5517). Default
  // OFF: the `/v1/fine_tuning/jobs` and `/v1/sandboxes` routes are inert on the
  // live Worker until those builds land. Set "true"/"1"/"on" to enable. The
  // related promises stay red until a dereferenceable paid receipt exists.
  CLOUD_FINE_TUNING_ENABLED?: string | undefined
  CLOUD_SANDBOX_COMPUTE_ENABLED?: string | undefined
  // Cloud coding-session surface flag (autopilot.cloud_coding_sessions.v1, red).
  // Default OFF: the `/v1/cloud-coding-sessions` launch + lifecycle routes are
  // inert on the live Worker until the managed GCE runtime is wired. Set
  // "true"/"1"/"on" to enable. Launch still fails closed unless
  // OA_CODEX_GCE_PROVISIONER=live and OA_CLOUD_CONTROL_URL/TOKEN are configured.
  // The promise stays red until a desktop-originated cloud session runs a real
  // repo-edit on GCE and produces a content-addressed artifact plus a
  // dereferenceable resource_usage_receipt with owner sign-off.
  CLOUD_CODING_SESSIONS_ENABLED?: string | undefined
  OA_CODEX_GCE_PROVISIONER?: string | undefined
  OA_CLOUD_CONTROL_URL?: string | undefined
  OA_CLOUD_CONTROL_TOKEN?: string | undefined
  // Partner passthrough adapter secrets (EPIC #5474, #5481). Worker secrets,
  // never committed/logged. Each enables the corresponding passthrough adapter
  // when the gateway flag is on; absent => that partner adapter stays inert.
  ANTHROPIC_API_KEY?: string | undefined
  OPENAI_API_KEY?: string | undefined
  // Optional partner base-URL overrides (origin, no trailing slash). Default to
  // the public Anthropic / OpenAI origins when unset.
  ANTHROPIC_BASE_URL?: string | undefined
  OPENAI_BASE_URL?: string | undefined
  // Vertex Anthropic adapter (EPIC #5474, #5480) — Claude lane on Google Cloud
  // Vertex AI. VERTEX_SA_KEY is the full service-account key JSON (a Worker
  // secret; NEVER committed) used to mint a short-lived GCP access token via a
  // JWT->token exchange. VERTEX_PROJECT_ID / VERTEX_LOCATION are optional
  // overrides (default project "openagentsgemini", default location "global").
  // The adapter is inert without VERTEX_SA_KEY; the route stays flag-gated off
  // via INFERENCE_GATEWAY_ENABLED regardless.
  VERTEX_SA_KEY?: string | undefined
  VERTEX_PROJECT_ID?: string | undefined
  VERTEX_LOCATION?: string | undefined
  // Hosted Gemini Autopilot executor arming flag (api.hosted_gemini.v1, yellow;
  // blocker.product_promises.production_hosted_gemini_executor_binding_missing).
  // Default OFF. The hosted Gemini `executeReadyWork` binding stays INERT on the
  // live Worker (no execution, no closeout) until this flag is on AND
  // VERTEX_SA_KEY is present (DOUBLE-gated). Set "1"/"true"/"yes"/"on" to arm;
  // optional HOSTED_GEMINI_MODEL overrides the requested model alias. Arming
  // does NOT flip the promise: green still needs the upstream task-ref resolver
  // and a registered-agent production smoke (see the launch worklog).
  HOSTED_GEMINI_EXECUTOR_ENABLED?: string | undefined
  HOSTED_GEMINI_MODEL?: string | undefined
  EXA_API_KEY?: string | undefined
  EXA_BASE_URL?: string | undefined
  EXA_DEFAULT_NUM_RESULTS?: string | undefined
  EXA_DEFAULT_SEARCH_TYPE?: string | undefined
  EXA_FRESHNESS_MAX_AGE_HOURS?: string | undefined
  EXA_ASSIGNMENT_REQUEST_BUDGET?: string | undefined
  EXA_CACHE_TTL_HOURS?: string | undefined
  EXA_DAILY_REQUEST_BUDGET?: string | undefined
  EXA_MAX_HIGHLIGHT_CHARACTERS?: string | undefined
  EXA_MAX_TEXT_CHARACTERS?: string | undefined
  EXA_REQUEST_TIMEOUT_MS?: string | undefined
  EXA_RETRY_LIMIT?: string | undefined
  EXA_RATE_LIMIT_BACKOFF_MS?: string | undefined
  // Fireworks AI provider adapter key (EPIC #5474, #5479). Worker secret; the
  // Fireworks open-model supply lane. Never logged. The gateway stays inert
  // under INFERENCE_GATEWAY_ENABLED, so this is only read when the adapter is
  // actually dispatched by routing (#5482).
  FIREWORKS_API_KEY?: string | undefined
  // OpenRouter Khala lane (#6313). Worker secret. This lane is a hidden Khala
  // supply tier: neither the key nor the upstream model id is published in
  // /v1/models. The upstream model is pinned in source to
  // `ibm-granite/granite-4.1-8b`; the legacy env model field is ignored by
  // registration.
  OPENROUTER_API_KEY?: string | undefined
  OPENROUTER_BASE_URL?: string | undefined
  OPENROUTER_KHALA_FALLBACK_MODEL?: string | undefined
  // Operator-only backing selector for the single public Khala model. This is a
  // non-secret routing knob, not a public model selector; supported values are
  // bounded in model-serving-policy.ts. Absent defaults to the Hydralisk-owned
  // Khala mix.
  KHALA_BACKING_MODEL?: string | undefined
  // Hydralisk GLM-5.2 504B REAP lane. The Worker registers this owned
  // OpenAI-compatible private proxy only when the enabled flag is exactly
  // "ready", the private URL/token are present, and the public-safe
  // preflight/receipt refs below are valid. The URL/token are Worker secrets
  // and must never appear in catalog/readiness payloads, public evidence, docs,
  // issues, or logs.
  HYDRALISK_GLM_52_REAP_504B_ENABLED?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_BASE_URL?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_PROFILE_REF?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_COST_PROFILE_REF?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_MAX_INFLIGHT?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_BENCHMARK_RESERVED?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_DRAINING?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_CADENCE_MINUTES?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_WARM_COMPLETION_ENABLED?:
    | string
    | undefined
  HYDRALISK_GLM_52_REAP_504B_BENCHMARK_OWNERSHIP_ACTIVE?: string | undefined
  // Optional comma-separated pool names. When absent, the five legacy GLM
  // fields above are treated as the `primary` replica. When present, each
  // named replica reads HYDRALISK_GLM_52_REAP_504B_<REPLICA>_<FIELD>, with
  // hyphens mapped to underscores and the same secret/evidence discipline.
  HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS?: string | undefined
  // Hydralisk GPT-OSS 20B lane (#6155). The Worker registers this owned
  // OpenAI-compatible vLLM adapter only when HYDRALISK_GPT_OSS_20B_ENABLED is
  // exactly "ready", HYDRALISK_BASE_URL and HYDRALISK_BEARER_TOKEN are present,
  // and the public-safe preflight/receipt refs below are valid. The URL/token
  // are Worker secrets and must never appear in catalog/readiness payloads,
  // public evidence, docs, issues, or logs.
  HYDRALISK_GPT_OSS_20B_ENABLED?: string | undefined
  HYDRALISK_BASE_URL?: string | undefined
  HYDRALISK_BEARER_TOKEN?: string | undefined
  HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF?: string | undefined
  HYDRALISK_GPT_OSS_20B_RECEIPT_REF?: string | undefined
  // Hydralisk GPT-OSS 120B high-memory lane. This is a separate adapter origin
  // from the 20B/L4 lane because 120B needs an H100/H200/B200/G4-class host and
  // its own public-safe preflight/receipt evidence before it can be advertised
  // or sold.
  HYDRALISK_GPT_OSS_120B_ENABLED?: string | undefined
  HYDRALISK_GPT_OSS_120B_BASE_URL?: string | undefined
  HYDRALISK_GPT_OSS_120B_BEARER_TOKEN?: string | undefined
  HYDRALISK_GPT_OSS_120B_PREFLIGHT_REF?: string | undefined
  HYDRALISK_GPT_OSS_120B_RECEIPT_REF?: string | undefined
  // OpenAgents/Pylon serving-fabric HTTP transport (#6089). The URL points at a
  // secret-backed gateway/proxy for an admitted Pylon `psionic-serve` compatible
  // endpoint; the bearer token authenticates Worker->proxy calls. Both are
  // Worker secrets and are presence-checked before the public openagents-network
  // lane can arm. Neither value may appear in catalog/readiness payloads,
  // public evidence, docs, issues, or logs.
  OPENAGENTS_NETWORK_FABRIC_SERVE_URL?: string | undefined
  OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN?: string | undefined
  // Public-safe admission snapshot fields for the admitted Pylon route. These
  // are refs/status strings only, never endpoint, wallet, or payment material.
  OPENAGENTS_NETWORK_PYLON_HEARTBEAT_AT?: string | undefined
  OPENAGENTS_NETWORK_PYLON_HEARTBEAT_STATUS?: string | undefined
  OPENAGENTS_NETWORK_PYLON_SERVING_CAPABILITY_REF?: string | undefined
  OPENAGENTS_NETWORK_PYLON_SERVING_LANE_REF?: string | undefined
  OPENAGENTS_NETWORK_SPARK_PAYOUT_TARGET_REF?: string | undefined
  GITHUB_CLIENT_ID?: string | undefined
  GITHUB_CLIENT_SECRET?: string | undefined
  ARTANIS_GITHUB_ISSUE_TOKEN?: string | undefined
  MDK_ACCESS_TOKEN?: string | undefined
  MDK_CHECKOUT_CONFIG_REF?: string | undefined
  MDK_CHECKOUT_CREDENTIAL_BINDING_REF?: string | undefined
  MDK_CHECKOUT_ENVIRONMENT?: string | undefined
  MDK_CHECKOUT_PATH_BASE?: string | undefined
  MDK_CHECKOUT_PROVIDER_REF?: string | undefined
  MDK_CHECKOUT_ROUTE_KIND?: string | undefined
  MDK_CHECKOUT_ROUTE_SECRET?: string | undefined
  MDK_CHECKOUT_ROUTE_URL?: string | undefined
  MDK_CHECKOUT_WEBHOOK_BINDING_REF?: string | undefined
  MDK_CHECKOUT_WEBHOOK_SECRET?: string | undefined
  MDK_CHECKOUT_WEBHOOK_SOURCE?: string | undefined
  MDK_WEBHOOK_SECRET?: string | undefined
  MDK_MNEMONIC?: string | undefined
  MDK_TIPS_BUFFER_ACCESS_TOKEN?: string | undefined
  MDK_TIPS_BUFFER_MNEMONIC?: string | undefined
  MDK_TIPS_BUFFER_SERVICE_TOKEN?: string | undefined
  MDK_TREASURY_ACCESS_TOKEN?: string | undefined
  MDK_TREASURY_MNEMONIC?: string | undefined
  MDK_TREASURY_SERVICE_TOKEN?: string | undefined
  MDK_WALLET_MNEMONIC?: string | undefined
  OPENAGENTS_SPARK_API_KEY?: string | undefined
  OPENAGENTS_ADMIN_API_TOKEN?: string | undefined
  OPENAGENTS_FORGE_CONTROL_PLANE_TOKEN?: string | undefined
  OPENAGENTS_FORGE_GITHUB_MIRROR_TOKEN?: string | undefined
  OPENAGENTS_APP_URL?: string | undefined
  OPENAUTH_CLIENT_ID?: string | undefined
  OPENAUTH_ISSUER_URL?: string | undefined
  RESEND_API_KEY?: string | undefined
  RESEND_FROM_EMAIL?: string | undefined
  RESEND_REPLY_TO_EMAIL?: string | undefined
  RESEND_WEBHOOK_SECRET?: string | undefined
  RUNNER_AUTOMATIC_FAILOVER_ENABLED?: string | undefined
  RUNNER_BACKEND_POLICY?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_ALLOWED_TRUSTS?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_CLASS_NAME?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_CONFIGURED?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_DURABLE_OBJECT_BINDING?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_ENABLED?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_IMAGE_REF?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_INSTANCE_TYPE?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_MAX_INSTANCES?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_POLICY_APPROVED?: string | undefined
  RUNNER_CLOUDFLARE_CONTAINER_STAGING_SMOKE?: string | undefined
  RUNNER_GCLOUD_REFERENCE_ENABLED?: string | undefined
  RUNNER_GCLOUD_SENSITIVE_APPROVED?: string | undefined
  SHC_CONTROL_API_BEARER_TOKEN?: string | undefined
  SHC_CONTROL_API_URL?: string | undefined
  SHC_DISPATCH_MODE?: string | undefined
  SHC_RUNNER_CALLBACK_TOKEN?: string | undefined
  SPARK_TREASURY_API_KEY?: string | undefined
  SPARK_TREASURY_MNEMONIC?: string | undefined
  SPARK_TREASURY_NETWORK?: string | undefined
  SPARK_TREASURY_STORAGE_DIR?: string | undefined
  SPARK_TREASURY_TIMEOUT_MS?: string | undefined
  TREASURY_DISPATCH_DAILY_SATS_CAP?: string | undefined
  TREASURY_DISPATCH_ENABLED?: string | undefined
  TREASURY_DISPATCH_LIQUIDITY_BUFFER_SATS?: string | undefined
  TREASURY_DISPATCH_PAYMENT_TIMEOUT_SECS?: string | undefined
  TREASURY_DISPATCH_PER_RUN_REWARD_CAP?: string | undefined
  WITHDRAWAL_DESTINATION?: string | undefined
}>

export const OpenAgentsAppUrl = S.String.pipe(S.brand('OpenAgentsAppUrl'))
export type OpenAgentsAppUrl = typeof OpenAgentsAppUrl.Type

export const OpenAgentsAppOrigin = S.String.pipe(S.brand('OpenAgentsAppOrigin'))
export type OpenAgentsAppOrigin = typeof OpenAgentsAppOrigin.Type

export const OpenAuthIssuerUrl = S.String.pipe(S.brand('OpenAuthIssuerUrl'))
export type OpenAuthIssuerUrl = typeof OpenAuthIssuerUrl.Type

export const OpenAuthIssuerOrigin = S.String.pipe(
  S.brand('OpenAuthIssuerOrigin'),
)
export type OpenAuthIssuerOrigin = typeof OpenAuthIssuerOrigin.Type

export const GitHubClientId = S.String.pipe(S.brand('GitHubClientId'))
export type GitHubClientId = typeof GitHubClientId.Type

export const OpenAuthClientId = S.String.pipe(S.brand('OpenAuthClientId'))
export type OpenAuthClientId = typeof OpenAuthClientId.Type

export const ResendEmailSender = S.String.pipe(S.brand('ResendEmailSender'))
export type ResendEmailSender = typeof ResendEmailSender.Type

export const EmailAddress = S.String.pipe(S.brand('EmailAddress'))
export type EmailAddress = typeof EmailAddress.Type

export const ShcControlApiUrl = S.String.pipe(S.brand('ShcControlApiUrl'))
export type ShcControlApiUrl = typeof ShcControlApiUrl.Type

export const WorkerSecret = S.String.pipe(S.brand('WorkerSecret'))
export type WorkerSecret = typeof WorkerSecret.Type

export type ShcDispatchMode = 'live' | 'unconfigured'

export type RunnerBackendPolicy =
  | 'shc_primary_cloudflare_container_backup_gcloud_reference'
  | 'shc_primary_only'

export type RunnerWorkloadTrust = 'low' | 'medium' | 'sensitive'

export type CloudflareContainerInstanceType =
  | 'basic'
  | 'lite'
  | 'standard-1'
  | 'standard-2'
  | 'standard-3'
  | 'standard-4'

export type ResendEmailConfig = Readonly<{
  apiKey: Redacted.Redacted<WorkerSecret>
  fromEmail: ResendEmailSender
  replyToEmail?: EmailAddress | undefined
}>

export type MdkWorkerConfig = Readonly<{
  accessToken?: Redacted.Redacted<WorkerSecret> | undefined
  checkout: Readonly<{
    checkoutPathBase: string
    configRef: string
    configured: boolean
    credentialBindingRef: string | null
    environment: 'production' | 'sandbox'
    providerRef: string
    routeKind: MdkCheckoutRouteKind
    routeSecret?: Redacted.Redacted<WorkerSecret> | undefined
    routeUrl?: string | undefined
    webhookBindingRef: string | null
    webhookSecret?: Redacted.Redacted<WorkerSecret> | undefined
    webhookSource:
      | 'daemon_invoice_hmac'
      | 'dashboard_standard_webhooks'
      | 'sdk_node_control'
  }>
  configured: boolean
  mnemonic?: Redacted.Redacted<WorkerSecret> | undefined
  walletMnemonic?: Redacted.Redacted<WorkerSecret> | undefined
}>

export type MdkCheckoutRouteKind =
  | 'fake_provider'
  | 'hosted_platform'
  | 'self_hosted_mdkd_sidecar'

export const ExaBaseUrl = S.String.pipe(S.brand('ExaBaseUrl'))
export type ExaBaseUrl = typeof ExaBaseUrl.Type

export type ExaSearchType =
  | 'auto'
  | 'deep'
  | 'deep-lite'
  | 'deep-reasoning'
  | 'fast'
  | 'instant'

export type ExaConfig = Readonly<{
  apiKey?: Redacted.Redacted<WorkerSecret> | undefined
  assignmentRequestBudget: number
  baseUrl: ExaBaseUrl
  cacheTtlHours: number
  dailyRequestBudget: number
  defaultNumResults: number
  defaultSearchType: ExaSearchType
  enabled: boolean
  freshnessMaxAgeHours: number
  maxHighlightCharacters: number
  maxTextCharacters: number
  rateLimitBackoffMs: number
  requestTimeoutMs: number
  retryLimit: number
}>

export type RunnerBackendConfig = Readonly<{
  automaticFailoverEnabled: boolean
  cloudflareContainer: Readonly<{
    allowedWorkloadTrusts: ReadonlyArray<RunnerWorkloadTrust>
    binding: Readonly<{
      className?: string | undefined
      durableObjectBinding?: string | undefined
      imageRef?: string | undefined
      instanceType?: CloudflareContainerInstanceType | undefined
      maxInstances?: number | undefined
    }>
    configured: boolean
    enabled: boolean
    policyApproved: boolean
    stagingSmokePassed: boolean
  }>
  gcloud: Readonly<{
    referenceEnabled: boolean
    sensitiveApproved: boolean
  }>
  policy: RunnerBackendPolicy
}>

export type OpenAgentsWorkerConfigShape = Readonly<{
  adminApiToken?: Redacted.Redacted<WorkerSecret> | undefined
  app: Readonly<{
    origin: OpenAgentsAppOrigin
    url: OpenAgentsAppUrl
  }>
  artanis: Readonly<{
    fleetOverseerEnabled: boolean
    scheduledRunnerEnabled: boolean
  }>
  email: Readonly<{
    resend?: ResendEmailConfig | undefined
    resendWebhookSecret?: Redacted.Redacted<WorkerSecret> | undefined
  }>
  exa: ExaConfig
  forgeControlPlaneToken?: Redacted.Redacted<WorkerSecret> | undefined
  forgeGithubMirrorToken?: Redacted.Redacted<WorkerSecret> | undefined
  github: Readonly<{
    clientId: GitHubClientId
    clientSecret: Redacted.Redacted<WorkerSecret>
  }>
  mdk: MdkWorkerConfig
  openauth: Readonly<{
    clientId: OpenAuthClientId
    issuerOrigin: OpenAuthIssuerOrigin
    issuerUrl: OpenAuthIssuerUrl
  }>
  runnerBackends: RunnerBackendConfig
  shc: Readonly<{
    controlApiBearerToken?: Redacted.Redacted<WorkerSecret> | undefined
    controlApiUrl?: ShcControlApiUrl | undefined
    dispatchMode: ShcDispatchMode
    runnerCallbackToken?: Redacted.Redacted<WorkerSecret> | undefined
  }>
}>

export class OpenAgentsWorkerConfigError extends S.TaggedErrorClass<OpenAgentsWorkerConfigError>()(
  'OpenAgentsWorkerConfigError',
  {
    field: S.String,
    reason: S.String,
  },
) {}

export class OpenAgentsWorkerConfig extends Context.Service<
  OpenAgentsWorkerConfig,
  OpenAgentsWorkerConfigShape
>()('@openagentsinc/OpenAgentsWorkerConfig') {
  static layer = (env: OpenAgentsWorkerConfigEnv) =>
    Layer.effect(OpenAgentsWorkerConfig, decodeOpenAgentsWorkerConfig(env))
}

const configCache = new WeakMap<object, OpenAgentsWorkerConfigShape>()

const trimmed = (value: string | undefined): string | undefined => {
  const next = value?.trim()

  return next === undefined || next === '' ? undefined : next
}

const requiredString = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<string, OpenAgentsWorkerConfigError> => {
  const value = trimmed(env[field])

  return value === undefined
    ? Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Required configuration value is missing.',
        }),
      )
    : Effect.succeed(value)
}

const optionalString = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): string | undefined => trimmed(env[field])

const emailAddressPattern = /^[^\s@<>]+@[^\s@<>]+\.[^\s@<>]+$/

const extractAddress = (value: string): string =>
  value.match(/<([^<>]+)>$/u)?.[1]?.trim() ?? value

const validateEmailAddress = (
  field: keyof OpenAgentsWorkerConfigEnv,
  value: string,
): Effect.Effect<string, OpenAgentsWorkerConfigError> =>
  emailAddressPattern.test(extractAddress(value))
    ? Effect.succeed(value)
    : Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Expected a valid email address.',
        }),
      )

const parseUrl = (
  field: keyof OpenAgentsWorkerConfigEnv,
  value: string,
): Effect.Effect<URL, OpenAgentsWorkerConfigError> =>
  Effect.try({
    catch: error =>
      new OpenAgentsWorkerConfigError({
        field,
        reason: error instanceof Error ? error.message : String(error),
      }),
    try: () => new URL(value),
  })

const requiredUrl = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<
  Readonly<{ origin: string; url: string }>,
  OpenAgentsWorkerConfigError
> =>
  Effect.gen(function* () {
    const value = yield* requiredString(env, field)
    const url = yield* parseUrl(field, value)

    return {
      origin: url.origin,
      url: url.toString(),
    }
  })

const optionalUrl = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<string | undefined, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  return value === undefined
    ? Effect.sync(() => undefined)
    : Effect.map(parseUrl(field, value), url => url.toString())
}

const optionalBooleanFlag = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<boolean, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  if (value === undefined) {
    return Effect.succeed(false)
  }

  const normalized = value.toLowerCase()

  if (['1', 'true', 'yes', 'on'].includes(normalized)) {
    return Effect.succeed(true)
  }

  if (['0', 'false', 'no', 'off'].includes(normalized)) {
    return Effect.succeed(false)
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field,
      reason: 'Expected a boolean flag value.',
    }),
  )
}

const redacted = (
  field: keyof OpenAgentsWorkerConfigEnv,
  value: string,
): Redacted.Redacted<WorkerSecret> =>
  Redacted.make(WorkerSecret.make(value), { label: field })

const requiredRedacted = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<
  Redacted.Redacted<WorkerSecret>,
  OpenAgentsWorkerConfigError
> => Effect.map(requiredString(env, field), value => redacted(field, value))

const optionalRedacted = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Redacted.Redacted<WorkerSecret> | undefined => {
  const value = optionalString(env, field)

  return value === undefined ? undefined : redacted(field, value)
}

const optionalPositiveInteger = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
  fallback: number,
): Effect.Effect<number, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  if (value === undefined) {
    return Effect.succeed(fallback)
  }

  const parsed = Number(value)

  return Number.isInteger(parsed) && parsed > 0
    ? Effect.succeed(parsed)
    : Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Expected a positive integer.',
        }),
      )
}

const optionalNonNegativeInteger = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
  fallback: number,
): Effect.Effect<number, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  if (value === undefined) {
    return Effect.succeed(fallback)
  }

  const parsed = Number(value)

  return Number.isInteger(parsed) && parsed >= 0
    ? Effect.succeed(parsed)
    : Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Expected a non-negative integer.',
        }),
      )
}

const optionalPositiveIntegerValue = (
  env: OpenAgentsWorkerConfigEnv,
  field: keyof OpenAgentsWorkerConfigEnv,
): Effect.Effect<number | undefined, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, field)

  if (value === undefined) {
    return Effect.sync((): number | undefined => undefined)
  }

  const parsed = Number(value)

  return Number.isInteger(parsed) && parsed > 0
    ? Effect.succeed(parsed)
    : Effect.fail(
        new OpenAgentsWorkerConfigError({
          field,
          reason: 'Expected a positive integer.',
        }),
      )
}

const exaSearchType = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<ExaSearchType, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, 'EXA_DEFAULT_SEARCH_TYPE') ?? 'auto'

  if (
    value === 'auto' ||
    value === 'fast' ||
    value === 'instant' ||
    value === 'deep-lite' ||
    value === 'deep' ||
    value === 'deep-reasoning'
  ) {
    return Effect.succeed(value)
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'EXA_DEFAULT_SEARCH_TYPE',
      reason:
        'Expected auto, fast, instant, deep-lite, deep, or deep-reasoning.',
    }),
  )
}

const exaConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<ExaConfig, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    const apiKey = optionalRedacted(env, 'EXA_API_KEY')
    const baseUrl =
      (yield* optionalUrl(env, 'EXA_BASE_URL')) ?? 'https://api.exa.ai/'
    const normalizedBaseUrl = new URL(baseUrl).origin

    return {
      apiKey,
      assignmentRequestBudget: yield* optionalPositiveInteger(
        env,
        'EXA_ASSIGNMENT_REQUEST_BUDGET',
        12,
      ),
      baseUrl: ExaBaseUrl.make(normalizedBaseUrl),
      cacheTtlHours: yield* optionalPositiveInteger(
        env,
        'EXA_CACHE_TTL_HOURS',
        24,
      ),
      dailyRequestBudget: yield* optionalPositiveInteger(
        env,
        'EXA_DAILY_REQUEST_BUDGET',
        200,
      ),
      defaultNumResults: yield* optionalPositiveInteger(
        env,
        'EXA_DEFAULT_NUM_RESULTS',
        8,
      ),
      defaultSearchType: yield* exaSearchType(env),
      enabled: apiKey !== undefined,
      freshnessMaxAgeHours: yield* optionalNonNegativeInteger(
        env,
        'EXA_FRESHNESS_MAX_AGE_HOURS',
        24,
      ),
      maxHighlightCharacters: yield* optionalPositiveInteger(
        env,
        'EXA_MAX_HIGHLIGHT_CHARACTERS',
        1_200,
      ),
      maxTextCharacters: yield* optionalPositiveInteger(
        env,
        'EXA_MAX_TEXT_CHARACTERS',
        6_000,
      ),
      rateLimitBackoffMs: yield* optionalPositiveInteger(
        env,
        'EXA_RATE_LIMIT_BACKOFF_MS',
        1_000,
      ),
      requestTimeoutMs: yield* optionalPositiveInteger(
        env,
        'EXA_REQUEST_TIMEOUT_MS',
        25_000,
      ),
      retryLimit: yield* optionalNonNegativeInteger(env, 'EXA_RETRY_LIMIT', 2),
    }
  })

const shcDispatchMode = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<ShcDispatchMode, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, 'SHC_DISPATCH_MODE')

  if (value === undefined || value === 'unconfigured') {
    return Effect.succeed('unconfigured')
  }

  if (value === 'live') {
    return Effect.succeed('live')
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'SHC_DISPATCH_MODE',
      reason: 'Expected "live" or "unconfigured".',
    }),
  )
}

const runnerBackendPolicy = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<RunnerBackendPolicy, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, 'RUNNER_BACKEND_POLICY')

  if (value === undefined || value === 'shc_primary_only') {
    return Effect.succeed('shc_primary_only')
  }

  if (value === 'shc_primary_cloudflare_container_backup_gcloud_reference') {
    return Effect.succeed(
      'shc_primary_cloudflare_container_backup_gcloud_reference',
    )
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'RUNNER_BACKEND_POLICY',
      reason:
        'Expected "shc_primary_only" or "shc_primary_cloudflare_container_backup_gcloud_reference".',
    }),
  )
}

const runnerWorkloadTrustValues: ReadonlyArray<RunnerWorkloadTrust> = [
  'low',
  'medium',
  'sensitive',
]

const isRunnerWorkloadTrust = (value: string): value is RunnerWorkloadTrust =>
  runnerWorkloadTrustValues.includes(value as RunnerWorkloadTrust)

const cloudflareContainerAllowedWorkloadTrusts = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<
  ReadonlyArray<RunnerWorkloadTrust>,
  OpenAgentsWorkerConfigError
> => {
  const value = optionalString(
    env,
    'RUNNER_CLOUDFLARE_CONTAINER_ALLOWED_TRUSTS',
  )

  if (value === undefined) {
    return Effect.succeed(['low', 'medium'])
  }

  const trusts = value
    .split(',')
    .map(part => part.trim())
    .filter(part => part !== '')
  const invalid = trusts.find(trust => !isRunnerWorkloadTrust(trust))

  if (trusts.length === 0 || invalid !== undefined) {
    return Effect.fail(
      new OpenAgentsWorkerConfigError({
        field: 'RUNNER_CLOUDFLARE_CONTAINER_ALLOWED_TRUSTS',
        reason: 'Expected comma-separated low, medium, or sensitive values.',
      }),
    )
  }

  return Effect.succeed([
    ...new Set(trusts),
  ] as ReadonlyArray<RunnerWorkloadTrust>)
}

const cloudflareContainerInstanceTypes: ReadonlyArray<CloudflareContainerInstanceType> =
  ['lite', 'basic', 'standard-1', 'standard-2', 'standard-3', 'standard-4']

const cloudflareContainerInstanceType = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<
  CloudflareContainerInstanceType | undefined,
  OpenAgentsWorkerConfigError
> => {
  const value = optionalString(env, 'RUNNER_CLOUDFLARE_CONTAINER_INSTANCE_TYPE')

  if (value === undefined) {
    return Effect.sync(
      (): CloudflareContainerInstanceType | undefined => undefined,
    )
  }

  if (
    cloudflareContainerInstanceTypes.includes(
      value as CloudflareContainerInstanceType,
    )
  ) {
    return Effect.succeed(value as CloudflareContainerInstanceType)
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'RUNNER_CLOUDFLARE_CONTAINER_INSTANCE_TYPE',
      reason:
        'Expected lite, basic, standard-1, standard-2, standard-3, or standard-4.',
    }),
  )
}

const runnerBackendConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<RunnerBackendConfig, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    return {
      automaticFailoverEnabled: yield* optionalBooleanFlag(
        env,
        'RUNNER_AUTOMATIC_FAILOVER_ENABLED',
      ),
      cloudflareContainer: {
        allowedWorkloadTrusts:
          yield* cloudflareContainerAllowedWorkloadTrusts(env),
        binding: {
          className: optionalString(
            env,
            'RUNNER_CLOUDFLARE_CONTAINER_CLASS_NAME',
          ),
          durableObjectBinding: optionalString(
            env,
            'RUNNER_CLOUDFLARE_CONTAINER_DURABLE_OBJECT_BINDING',
          ),
          imageRef: optionalString(
            env,
            'RUNNER_CLOUDFLARE_CONTAINER_IMAGE_REF',
          ),
          instanceType: yield* cloudflareContainerInstanceType(env),
          maxInstances: yield* optionalPositiveIntegerValue(
            env,
            'RUNNER_CLOUDFLARE_CONTAINER_MAX_INSTANCES',
          ),
        },
        configured: yield* optionalBooleanFlag(
          env,
          'RUNNER_CLOUDFLARE_CONTAINER_CONFIGURED',
        ),
        enabled: yield* optionalBooleanFlag(
          env,
          'RUNNER_CLOUDFLARE_CONTAINER_ENABLED',
        ),
        policyApproved: yield* optionalBooleanFlag(
          env,
          'RUNNER_CLOUDFLARE_CONTAINER_POLICY_APPROVED',
        ),
        stagingSmokePassed: yield* optionalBooleanFlag(
          env,
          'RUNNER_CLOUDFLARE_CONTAINER_STAGING_SMOKE',
        ),
      },
      gcloud: {
        referenceEnabled: yield* optionalBooleanFlag(
          env,
          'RUNNER_GCLOUD_REFERENCE_ENABLED',
        ),
        sensitiveApproved: yield* optionalBooleanFlag(
          env,
          'RUNNER_GCLOUD_SENSITIVE_APPROVED',
        ),
      },
      policy: yield* runnerBackendPolicy(env),
    }
  })

const resendConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<ResendEmailConfig | undefined, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    const hasAnyResendValue =
      optionalString(env, 'RESEND_API_KEY') !== undefined ||
      optionalString(env, 'RESEND_FROM_EMAIL') !== undefined ||
      optionalString(env, 'RESEND_REPLY_TO_EMAIL') !== undefined

    if (!hasAnyResendValue) {
      return undefined
    }

    const fromEmail = yield* Effect.flatMap(
      requiredString(env, 'RESEND_FROM_EMAIL'),
      value => validateEmailAddress('RESEND_FROM_EMAIL', value),
    )
    const replyToEmail = optionalString(env, 'RESEND_REPLY_TO_EMAIL')

    if (replyToEmail !== undefined) {
      yield* validateEmailAddress('RESEND_REPLY_TO_EMAIL', replyToEmail)
    }

    return {
      apiKey: yield* requiredRedacted(env, 'RESEND_API_KEY'),
      fromEmail: ResendEmailSender.make(fromEmail),
      replyToEmail:
        replyToEmail === undefined
          ? undefined
          : EmailAddress.make(replyToEmail),
    }
  })

const mdkCheckoutRouteKind = (
  env: OpenAgentsWorkerConfigEnv,
  routeUrl: string | undefined,
): Effect.Effect<MdkCheckoutRouteKind, OpenAgentsWorkerConfigError> => {
  const value = optionalString(env, 'MDK_CHECKOUT_ROUTE_KIND')

  if (value === undefined) {
    return Effect.succeed(
      routeUrl === undefined ? 'fake_provider' : 'hosted_platform',
    )
  }

  if (
    value === 'fake_provider' ||
    value === 'hosted_platform' ||
    value === 'self_hosted_mdkd_sidecar'
  ) {
    return Effect.succeed(value)
  }

  return Effect.fail(
    new OpenAgentsWorkerConfigError({
      field: 'MDK_CHECKOUT_ROUTE_KIND',
      reason:
        'Expected fake_provider, hosted_platform, or self_hosted_mdkd_sidecar.',
    }),
  )
}

const mdkConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<MdkWorkerConfig, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    const accessToken = optionalRedacted(env, 'MDK_ACCESS_TOKEN')
    const checkoutRouteSecret =
      optionalRedacted(env, 'MDK_CHECKOUT_ROUTE_SECRET') ?? accessToken
    const checkoutRouteUrl = optionalString(env, 'MDK_CHECKOUT_ROUTE_URL')
    const routeKind = yield* mdkCheckoutRouteKind(env, checkoutRouteUrl)
    const checkoutWebhookSecret =
      optionalRedacted(env, 'MDK_CHECKOUT_WEBHOOK_SECRET') ??
      optionalRedacted(env, 'MDK_WEBHOOK_SECRET')
    const mnemonic = optionalRedacted(env, 'MDK_MNEMONIC')
    const walletMnemonic = optionalRedacted(env, 'MDK_WALLET_MNEMONIC')
    const checkoutEnvironment =
      optionalString(env, 'MDK_CHECKOUT_ENVIRONMENT') === 'production'
        ? 'production'
        : 'sandbox'
    const checkoutWebhookSourceInput = optionalString(
      env,
      'MDK_CHECKOUT_WEBHOOK_SOURCE',
    )
    const checkoutWebhookSource =
      checkoutWebhookSourceInput === 'daemon_invoice_hmac' ||
      checkoutWebhookSourceInput === 'sdk_node_control'
        ? checkoutWebhookSourceInput
        : 'dashboard_standard_webhooks'

    return {
      accessToken,
      checkout: {
        checkoutPathBase:
          optionalString(env, 'MDK_CHECKOUT_PATH_BASE') ?? '/checkout',
        configRef:
          optionalString(env, 'MDK_CHECKOUT_CONFIG_REF') ??
          'config.openagents.hosted_mdk.route',
        configured:
          checkoutRouteSecret !== undefined && checkoutRouteUrl !== undefined,
        credentialBindingRef:
          optionalString(env, 'MDK_CHECKOUT_CREDENTIAL_BINDING_REF') ??
          (checkoutRouteSecret === undefined
            ? null
            : 'credential_binding.openagents.hosted_mdk.route_binding'),
        environment: checkoutEnvironment,
        providerRef:
          optionalString(env, 'MDK_CHECKOUT_PROVIDER_REF') ??
          'provider.openagents.hosted_mdk.route',
        routeKind,
        routeSecret: checkoutRouteSecret,
        routeUrl: checkoutRouteUrl,
        webhookBindingRef:
          optionalString(env, 'MDK_CHECKOUT_WEBHOOK_BINDING_REF') ??
          (checkoutWebhookSecret === undefined
            ? null
            : `webhook_binding.openagents.hosted_mdk.${checkoutWebhookSource}`),
        webhookSecret: checkoutWebhookSecret,
        webhookSource: checkoutWebhookSource,
      },
      configured:
        accessToken !== undefined ||
        checkoutRouteSecret !== undefined ||
        checkoutRouteUrl !== undefined ||
        checkoutWebhookSecret !== undefined ||
        mnemonic !== undefined ||
        walletMnemonic !== undefined,
      mnemonic,
      walletMnemonic,
    }
  })

const validateLiveShc = (
  env: OpenAgentsWorkerConfigEnv,
  dispatchMode: ShcDispatchMode,
  controlApiUrl: string | undefined,
  controlApiBearerToken: Redacted.Redacted<string> | undefined,
): Effect.Effect<void, OpenAgentsWorkerConfigError> => {
  if (dispatchMode !== 'live') {
    return Effect.sync(() => undefined)
  }

  if (controlApiUrl === undefined) {
    return Effect.fail(
      new OpenAgentsWorkerConfigError({
        field: 'SHC_CONTROL_API_URL',
        reason:
          'SHC_CONTROL_API_URL is required when SHC_DISPATCH_MODE is live.',
      }),
    )
  }

  if (controlApiBearerToken === undefined) {
    return Effect.fail(
      new OpenAgentsWorkerConfigError({
        field: 'SHC_CONTROL_API_BEARER_TOKEN',
        reason:
          'SHC_CONTROL_API_BEARER_TOKEN is required when SHC_DISPATCH_MODE is live.',
      }),
    )
  }

  return Effect.sync(() => undefined)
}

export const decodeOpenAgentsWorkerConfig = (
  env: OpenAgentsWorkerConfigEnv,
): Effect.Effect<OpenAgentsWorkerConfigShape, OpenAgentsWorkerConfigError> =>
  Effect.gen(function* () {
    const app = yield* requiredUrl(env, 'OPENAGENTS_APP_URL')
    const issuer = yield* requiredUrl(env, 'OPENAUTH_ISSUER_URL')
    const dispatchMode = yield* shcDispatchMode(env)
    const controlApiUrl = yield* optionalUrl(env, 'SHC_CONTROL_API_URL')
    const controlApiBearerToken = optionalRedacted(
      env,
      'SHC_CONTROL_API_BEARER_TOKEN',
    )

    yield* validateLiveShc(
      env,
      dispatchMode,
      controlApiUrl,
      controlApiBearerToken,
    )

    return {
      adminApiToken: optionalRedacted(env, 'OPENAGENTS_ADMIN_API_TOKEN'),
      app: {
        origin: OpenAgentsAppOrigin.make(app.origin),
        url: OpenAgentsAppUrl.make(app.url),
      },
      artanis: {
        fleetOverseerEnabled: yield* optionalBooleanFlag(
          env,
          'ARTANIS_FLEET_OVERSEER_ENABLED',
        ),
        scheduledRunnerEnabled: yield* optionalBooleanFlag(
          env,
          'ARTANIS_SCHEDULED_RUNNER_ENABLED',
        ),
      },
      email: {
        resend: yield* resendConfig(env),
        resendWebhookSecret: optionalRedacted(env, 'RESEND_WEBHOOK_SECRET'),
      },
      exa: yield* exaConfig(env),
      forgeControlPlaneToken: optionalRedacted(
        env,
        'OPENAGENTS_FORGE_CONTROL_PLANE_TOKEN',
      ),
      forgeGithubMirrorToken: optionalRedacted(
        env,
        'OPENAGENTS_FORGE_GITHUB_MIRROR_TOKEN',
      ),
      github: {
        clientId: GitHubClientId.make(
          yield* requiredString(env, 'GITHUB_CLIENT_ID'),
        ),
        clientSecret: yield* requiredRedacted(env, 'GITHUB_CLIENT_SECRET'),
      },
      mdk: yield* mdkConfig(env),
      openauth: {
        clientId: OpenAuthClientId.make(
          yield* requiredString(env, 'OPENAUTH_CLIENT_ID'),
        ),
        issuerOrigin: OpenAuthIssuerOrigin.make(issuer.origin),
        issuerUrl: OpenAuthIssuerUrl.make(issuer.url),
      },
      runnerBackends: yield* runnerBackendConfig(env),
      shc: {
        controlApiBearerToken,
        controlApiUrl:
          controlApiUrl === undefined
            ? undefined
            : ShcControlApiUrl.make(controlApiUrl),
        dispatchMode,
        runnerCallbackToken: optionalRedacted(env, 'SHC_RUNNER_CALLBACK_TOKEN'),
      },
    }
  })

export const getOpenAgentsWorkerConfig = (
  env: OpenAgentsWorkerConfigEnv,
): OpenAgentsWorkerConfigShape => {
  const cached = configCache.get(env)

  if (cached !== undefined) {
    return cached
  }

  const config = Effect.runSync(decodeOpenAgentsWorkerConfig(env))
  configCache.set(env, config)

  return config
}

export const redactedValue = (
  value: Redacted.Redacted<string> | undefined,
): string | undefined =>
  value === undefined ? undefined : Redacted.value(value)
