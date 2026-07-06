#!/usr/bin/env bun

import { existsSync, readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'

const listFiles = dir =>
  readdirSync(dir, { withFileTypes: true }).flatMap(entry => {
    const path = join(dir, entry.name)

    return entry.isDirectory() ? listFiles(path) : [path]
  })

const sourceFiles = ['workers/api/src', 'packages', 'apps/web/src']
  .flatMap(listFiles)
  .filter(path => /\.tsx?$/.test(path))
  .filter(path => !/\.test\.tsx?$/.test(path))
  .filter(path => !/\.test-support\.tsx?$/.test(path))
  .filter(path => !/\.story\.test\.tsx?$/.test(path))
  .filter(path => !/\.scene\.test\.tsx?$/.test(path))
  .filter(path => !path.includes('workers/api/src/test/'))

const khalaArchitectureScanFiles = [
  '../../clients/khala-code-desktop',
  '../../packages/khala-tools',
]
  .flatMap(listFiles)
  .filter(path => /\.tsx?$/.test(path))
  .filter(path => !/\.d\.ts$/.test(path))
  .filter(path => !/\.test\.tsx?$/.test(path))
  .filter(path => !/\.test-support\.tsx?$/.test(path))
  .filter(path => !/\.story\.test\.tsx?$/.test(path))
  .filter(path => !/\.scene\.test\.tsx?$/.test(path))

const workerFiles = sourceFiles.filter(path =>
  path.startsWith('workers/api/src/'),
)

const routeFiles = workerFiles.filter(
  path =>
    path.endsWith('-routes.ts') ||
    path.endsWith('/routes.ts') ||
    path.endsWith('/worker-routes.ts') ||
    path.endsWith('/index.ts'),
)

const workerServiceDomainFiles = workerFiles.filter(
  path =>
    !path.includes('/http/') &&
    !routeFiles.includes(path) &&
    !path.endsWith('-handlers.ts'),
)

const read = path => readFileSync(path, 'utf8')

const countMatches = (text, regex) => Array.from(text.matchAll(regex)).length

const countByFile = (files, regex) =>
  files
    .map(path => ({
      count: countMatches(read(path), regex),
      path,
    }))
    .filter(result => result.count > 0)

const totalCount = results =>
  results.reduce((total, result) => total + result.count, 0)

const convertedBrowserDomainFiles = [
  'apps/web/src/page/loggedIn/page/chat.ts',
  'apps/web/src/page/loggedIn/page/files.ts',
  'apps/web/src/page/loggedIn/run-timeline/projection.ts',
]

const browserPolicyConsumerFiles = sourceFiles.filter(
  path =>
    path.startsWith('apps/web/src/') &&
    !path.endsWith('apps/web/src/domain/session.ts') &&
    !path.endsWith('apps/web/src/product-policy.ts'),
)

const runtimePrimitiveBoundaryFiles = new Set([
  'apps/web/src/time-format.ts',
  'packages/sync-worker/src/runtime-primitives.ts',
  'workers/api/src/runtime-primitives.ts',
])

const jsonBoundaryFiles = new Set([
  'apps/web/src/json-boundary.ts',
  'packages/sync-schema/src/json-boundary.ts',
  'packages/sync-worker/src/json-boundary.ts',
  'workers/api/src/json-boundary.ts',
])

const deterministicBusinessLogicFiles = sourceFiles.filter(
  path => !runtimePrimitiveBoundaryFiles.has(path),
)

const proofReplayVisualRendererFiles = sourceFiles.filter(
  path =>
    /(?:proofReplay|proof-replay|ProofReplay)/.test(path) &&
    path !== 'apps/web/src/scene/tassadarProofReplayElement.ts' &&
    !path.startsWith('packages/proof-replay/'),
)

const lineCount = path => {
  const text = read(path)
  const lines = text.split('\n').length

  return text.endsWith('\n') ? lines - 1 : lines
}

const formatDetails = results =>
  results.map(result => `${result.path}: ${result.count}`).join('\n')

const formatRepoRootDetails = results =>
  results
    .map(
      result =>
        `${result.path.replace(/^\.\.\/\.\.\//, '')}: ${result.count}`,
    )
    .join('\n')

const budgetChecks = [
  {
    // Raised 8 -> 18 on 2026-06-14 for the wave-3 Agency Pack route landing
    // (omni-handoff/bundle/workroom, tenant-client, native-lists,
    // site-page-form). These are migration bridges to ratchet back down as
    // those route signatures move to Effect programs; do not raise further.
    budget: 18,
    description:
      'Route modules may not add Promise dependency adapters while route signatures migrate to Effect programs.',
    details: countByFile(
      routeFiles,
      /Effect\.promise\(\s*\(\)\s*=>\s*dependencies\./g,
    ),
    name: 'route dependency Effect.promise adapters',
  },
  {
    // Ratcheted 13 -> 0 on 2026-07-05 (#8371) after converting the remaining
    // live production Worker throw sites to typed TaggedError/repository errors.
    // Do not raise; expected errors should be typed at the source.
    budget: 0,
    description:
      'Production Worker modules may not add generic thrown expected errors while typed errors are introduced.',
    details: countByFile(workerFiles, /throw\s+new\s+Error\(/g),
    name: 'Worker throw new Error calls',
  },
  {
    budget: 0,
    description:
      'Provider-account routes must map ProviderAccountError tags, not English error-message substrings.',
    details: countByFile(
      workerFiles.filter(path => path.includes('provider-account')),
      /message\.includes\(\s*['"](?:credential-shaped|expired|not connected|not issued|not pending|does not match|cannot be marked|not found)['"]\s*\)/g,
    ),
    name: 'provider-account string error classifiers',
  },
  {
    budget: 0,
    description:
      'GitHub-write routes must map GitHubWriteError tags, not English error-message substrings.',
    details: countByFile(
      workerFiles.filter(
        path =>
          path.includes('github-write') ||
          path.endsWith('workers/api/src/index.ts'),
      ),
      /message\.includes\(\s*['"](?:expired|not issued|does not match|not usable|missing required scopes)['"]\s*\)/g,
    ),
    name: 'GitHub-write string error classifiers',
  },
  {
    budget: 0,
    description:
      'Domain modules must keep raw JSON.parse inside named boundary decoders.',
    details: countByFile(
      sourceFiles.filter(path => !jsonBoundaryFiles.has(path)),
      /JSON\.parse\(/g,
    ),
    name: 'raw JSON.parse outside json-boundary',
  },
  {
    budget: 0,
    description:
      'Business logic must consume runtime primitive helpers or injected services instead of raw time, UUID, or randomness primitives.',
    details: countByFile(
      deterministicBusinessLogicFiles,
      /\bDate\.now\(|new Date\(|crypto\.randomUUID\(|Math\.random\(/g,
    ),
    name: 'raw time/id/random primitives',
  },
  {
    // Raised 160 -> 162 on 2026-06-19 (#5508) for the operator
    // inference-credit grant route (`handleOmniOperatorInferenceCreditApi`,
    // the admin mirror of the #5497 self-serve bridge): one `(request, env: Env)`
    // handler in operator-billing-routes.ts and its matching dependency
    // signature in omni-routes.ts, both the same shape as the sibling credits
    // handler already counted here. Do not raise further; ratchet back down when
    // the operator billing handlers move behind the config/binding boundary.
    // Raised 162 -> 163 on 2026-06-22 (#6053) for the Khala M3 auto-settlement
    // sink factory (`makeAcceptedOutcomeSettlementSink(env: Env)` in index.ts):
    // an INERT, double-gated (loop-arming flag + owner real-settlement gate,
    // both default OFF) factory that reads the env binding to construct the
    // accepted-outcome settlement sink, the same env-reading shape as the
    // sibling index.ts handlers already counted here. Do not raise further;
    // ratchet back down when index.ts env reads move behind the config/binding
    // boundary.
    // Raised 163 -> 164 on 2026-06-24 (#6228) for the Khala FREE API mode
    // self-serve mint handler (`handleFreeKeyMint(request, env: Env)` in
    // index.ts): one `(request, env: Env)` route handler the same shape as the
    // sibling `handleProgrammaticAgentRegistration` already counted here. Do not
    // raise further; ratchet back down when index.ts env reads move behind the
    // config/binding boundary.
    // Raised 164 -> 166 on 2026-06-27 (#6370) for the admin agent-token reissue
    // handler (`handleAdminReissueAgentToken(request, env: Env, ctx, options)`
    // in index.ts): the handler `env: Env` parameter plus its injectable
    // `authorize?: (request, env: Env, ctx) => Promise<boolean>` option type,
    // both the same env-reading shape as the sibling admin handlers already
    // counted here. Do not raise further; ratchet back down when index.ts env
    // reads move behind the config/binding boundary.
    // Raised 166 -> 167 on 2026-07-05 (#8282 Promise.all landmine audit
    // follow-up) for `notifyCanceledAgentRunSyncScopesEffect(env: Env, runId:
    // string)` in index.ts: the per-run sync-scope notify isolation helper
    // that closes the severe `enforceOutOfCreditsPolicy` landmine (a
    // sync-notify failure could previously block the SHC compute-cleanup
    // dispatch and out-of-credits email for a whole batch of canceled runs).
    // Same env-reading shape as the sibling index.ts handlers already
    // counted here. Do not raise further; ratchet back down when index.ts
    // env reads move behind the config/binding boundary.
    budget: 167,
    description:
      'Worker modules may not add raw Cloudflare Env parameters outside the future config/binding boundary.',
    details: countByFile(
      workerFiles.filter(
        path =>
          !path.endsWith('workers/api/src/bindings.ts') &&
          !path.endsWith('workers/api/src/config.ts'),
      ),
      /\benv\s*:\s*Env\b/g,
    ),
    name: 'raw Env parameter annotations',
  },
  {
    budget: 0,
    description:
      'Business code must consume OpenAgentsWorkerConfig instead of reading migrated secret/config fields directly from Cloudflare Env.',
    details: countByFile(
      workerFiles.filter(path => !path.endsWith('workers/api/src/config.ts')),
      /\benv\.(GITHUB_CLIENT_ID|GITHUB_CLIENT_SECRET|OPENAGENTS_ADMIN_API_TOKEN|OPENAGENTS_APP_URL|OPENAUTH_CLIENT_ID|OPENAUTH_ISSUER_URL|RESEND_API_KEY|RESEND_FROM_EMAIL|RESEND_REPLY_TO_EMAIL|SHC_CONTROL_API_BEARER_TOKEN|SHC_CONTROL_API_URL|SHC_DISPATCH_MODE|SHC_RUNNER_CALLBACK_TOKEN)\b/g,
    ),
    name: 'direct migrated Worker config Env reads',
  },
  {
    budget: 0,
    description:
      'Worker modules must schedule background work and access sync runtime bindings through the runtime capability services.',
    details: countByFile(
      workerFiles.filter(path => !path.endsWith('workers/api/src/runtime.ts')),
      /\bctx\.waitUntil\(|\benv\.(OPENAGENTS_DB|SYNC_ROOM|RUNNER_EVENTS)\b|\bthis\.env\.OPENAGENTS_DB\b|\bscopeIdFromName\b/g,
    ),
    name: 'direct Worker runtime capability access',
  },
  {
    budget: 0,
    description:
      'Production Worker logging must go through redacted Effect observability helpers, not raw console calls.',
    // Standalone CLI entrypoints (`*/cli.ts`, e.g. the node-side Khala
    // acceptance runner harness) run OUT of the Worker and must print their
    // result to stdout/stderr directly; they are not Worker request-handling
    // modules, so the redacted-observability rule does not apply to them.
    details: countByFile(
      workerFiles.filter(path => !path.endsWith('/cli.ts')),
      /console\.(error|warn|log)\(/g,
    ),
    name: 'raw Worker console logging',
  },
  {
    budget: 0,
    description:
      'Service and domain modules must return typed values/errors, leaving HTTP response mapping to route and HTTP modules.',
    details: countByFile(
      workerServiceDomainFiles,
      /\b(noStoreJsonResponse|redirectResponse|methodNotAllowed|forbidden\(|unauthorized\(|serverError\()/g,
    ),
    name: 'service/domain HTTP response helper usage',
  },
  {
    // Raised 80 -> 83 on 2026-06-14 for the wave-3 Agency Pack route landing;
    // ratchet back down as route mappers are extracted. Do not raise further.
    // Raised 83 -> 84 on 2026-06-19 (#5508) for the operator inference-credit
    // grant route (`handleOmniOperatorInferenceCreditApi`, the admin mirror of
    // the #5497 self-serve bridge), which returns `Promise<Response>` like the
    // sibling credits handler already counted. Ratchet back down when the
    // operator billing handlers are extracted behind route mappers.
    // Raised 84 -> 85 on 2026-06-19 (#5515) for the INERT compose-and-list
    // marketplace listing handler (`handleMarketplaceCompositionApi`), which
    // returns `Effect.Effect<Response>` like the sibling public read handlers.
    // Ratchet back down when the marketplace handlers are extracted behind
    // route mappers.
    // Raised 85 -> 86 on 2026-06-19 (#5519) for the INERT Autopilot all-in-one
    // composed-run listing handler (`handleAutopilotComposedRunApi`), which
    // returns `Effect.Effect<Response>` like the sibling public read handlers.
    // Ratchet back down when the composed-run handlers are extracted behind
    // route mappers.
    // +1 (86 -> 87) for the cloud coding-session surface
    // (cloud/cloud-coding-session-routes.ts, autopilot.cloud_coding_sessions.v1,
    // red): a flag-gated INERT launch + lifecycle scaffold mirroring the
    // sandbox/fine-tuning Cloud-primitive pattern. Ratchet back down when these
    // /v1/* cloud handlers are extracted behind shared route mappers.
    // +1 (87 -> 88) for the agentic labor-product flow surface
    // (agentic-labor-product-routes.ts, autopilot.agentic_labor_products.v1,
    // yellow): a flag-gated INERT read-only listing for the post->order->
    // dispatch->deliver->settle flow scaffold, mirroring the composed-run
    // listing pattern. Ratchet back down when these public-projection handlers
    // are extracted behind shared route mappers.
    // +1 (88 -> 89) for the signature usage-metering surface
    // (signature-usage-metering-routes.ts, marketplace.signature_monetization.v1,
    // red): a flag-gated INERT read-only metering projection mirroring the
    // compose-and-list/labor-product listing pattern. It returns
    // `Effect.Effect<Response>` like the sibling public read handlers. Ratchet
    // back down when these public-projection handlers are extracted behind
    // shared route mappers.
    // +1 (89 -> 90) for the Pylon multi-earning-node surface
    // (pylon-multi-earning-node-routes.ts, pylon.v0_3_multi_earning_node.v1,
    // red): a flag-gated INERT read-only projection that distinguishes
    // modeled/observed/pending/paid/settled amounts per earning mode, clearing
    // only blocker.product_promises.safe_public_projection_missing. It returns
    // `Effect.Effect<Response>` like the sibling public read handlers. Ratchet
    // back down when these public-projection handlers are extracted behind
    // shared route mappers.
    // +1 (90 -> 91) for the enterprise claim-upgrade audit projection
    // (promise-transition-audit-routes.ts, proof.claim_upgrade_receipts.v1): a
    // read-only public projection joining the transition-receipt feed against
    // the live registry so a third party can audit every green flip. It returns
    // `Effect.Effect<Response>` like the sibling public read handlers. Ratchet
    // back down when these public-projection handlers are extracted behind
    // shared route mappers.
    // +1 (91 -> 92) for the self-serve control-center fanout surface
    // (self-serve-fanout-routes.ts,
    // autopilot.control_center_fanout_marketplace.v1, yellow): a flag-gated
    // INERT read-only projection of customer-initiated self-serve fanout plans,
    // clearing only blocker.product_promises.self_serve_fanout_missing. It
    // returns `Effect.Effect<Response>` like the sibling public read handlers.
    // Ratchet back down when these public-projection handlers are extracted
    // behind shared route mappers.
    // +1 (92 -> 93) for the Omni client-delivery business-object projection
    // surface (omni-client-delivery-projection-routes.ts,
    // workrooms.omni_client_delivery_workrooms.v1, yellow): a flag-gated INERT
    // read-only projection over the existing source-authorized
    // business-object delivery seam, clearing only
    // blocker.product_promises.omni_client_delivery_projection_missing. The
    // handler returns `Response` directly (wrapped in `Effect.succeed` at the
    // mount). Ratchet back down when these public-projection handlers are
    // extracted behind shared route mappers.
    // +1 (93 -> 94) for the marketplace work-class catalog surface
    // (marketplace-work-class-catalog-routes.ts,
    // autopilot.control_center_fanout_marketplace.v1, yellow): a read-only
    // registry projection of the listable marketplace work classes that always
    // reports the still-uncleared plugin-marketplace-beyond-code_task blocker and
    // clears nothing. It returns `Effect.Effect<Response>` like the sibling
    // public read handlers. Ratchet back down when these public-projection
    // handlers are extracted behind shared route mappers.
    // +1 (94 -> 95) for the OpenAI-compatible single-model retrieve dispatcher
    // (inference/models-routes.ts, inference.gateway_credits_business.v1, red):
    // routeModelRetrieveRequest dispatches the path-param GET /v1/models/{model}
    // (the exact-route registry is exact-match only) to the already-built
    // handleModelRetrieve, advancing but NOT clearing
    // blocker.product_promises.public_paid_model_gateway_missing. It returns
    // `Effect.Effect<Response> | undefined` like the sibling cloud-coding-session
    // dispatcher. Ratchet back down when these path-param dispatchers are
    // extracted behind a shared prefix-route mapper.
    // +1 (95 -> 96) for the Artanis labor receipt feed read handler
    // (artanis-labor-receipt-routes.ts, artanis.labor_requester.v1, yellow):
    // handlePublicArtanisLaborReceiptsApi is a GET-only public-safe read of the
    // consolidated unattended-labor-request receipt store (whole feed or a single
    // receipt by content-addressed ref), advancing but NOT clearing
    // blocker.product_promises.artanis_labor_unattended_request_receipts_missing.
    // It returns `Effect.Effect<Response>` like the sibling public read handlers.
    // Ratchet back down when these public-projection handlers are extracted behind
    // shared route mappers.
    // +1 (96 -> 97) for the public labor earnings read handler
    // +1 (97 -> 98) for the coding quick win pipeline
    // +1 (98 -> 99) on 2026-06-22 (#6049) for the crawlable discovery-surface
    // renderer in inference/discovery-surfaces.ts (`renderDiscoverySurface`
    // `Effect.Effect<Response>`, mounted from index.ts). #8387 removed the
    // separate default-off MPP/x402 chat route, Stripe client, and MPP OpenAPI
    // renderer, leaving only static keyed-Khala discovery docs.
    // +2 (103 -> 105) on 2026-06-22 (#6058, EPIC #6056) for the durable-stream
    // Rank-1 resumable-inference surfaces: the durable resume-read route
    // (inference/durable-inference-read-routes.ts `routeDurableInferenceReadRequest`
    // returning `Response | undefined`, the path-param resume surface the
    // exact-route registry cannot match — reads stored bytes only, never meters)
    // and the durable-stream DO class (index.ts `DurableInferenceStreamObject.fetch`
    // returning `Promise<Response>`, the Cloudflare DO fetch contract). Both are
    // flag-gated INERT by default. The DO `fetch` is a required Cloudflare runtime
    // signature; ratchet back down when the read route is extracted behind a shared
    // prefix-route mapper.
    // +1 (105 -> 106) on 2026-06-23 (#5531, DE-8) for the Artanis labor-requester
    // green-readiness read handler in artanis-labor-receipt-routes.ts
    // (`handlePublicArtanisLaborGreenReadinessApi` `Effect.Effect<Response>`),
    // a public read-only projection that folds the labor receipt feed onto the
    // two named green-flip blockers. It returns `Effect.Effect<Response>` like
    // the sibling `handlePublicArtanisLaborReceiptsApi` already counted here, is
    // no-store and mints no authority. Ratchet back down when these Artanis read
    // handlers are extracted behind shared route mappers.
    // +3 (107 -> 110) on 2026-06-23 (#6154 / EPIC #6056) for the durable
    // resumable-inference wiring: (a) the DO-fetch transport stub type in
    // inference/durable-inference-do-transport.ts (`DurableStreamStub.fetch():
    // Promise<Response>`, a REQUIRED structural typing of the Cloudflare DO stub
    // surface, mirroring the package's own local DO typing convention), and (b)
    // two SHARED response builders extracted in
    // inference/durable-inference-read-routes.ts (`jsonError`, `replayToResponse`)
    // that DEDUPLICATE the resume-read response shaping across the synchronous and
    // the new async DO-backed read dispatchers — a net reduction in inline
    // Response construction, just hoisted into named `: Response` helpers the
    // counter sees. Both read dispatchers read stored bytes only and NEVER meter.
    // Ratchet back down when the durable read surfaces are extracted behind a
    // shared prefix-route mapper.
    // +1 (110 -> 111) on 2026-06-24 (#6228) for the Khala FREE API mode
    // self-serve mint handler (`handleFreeKeyMint(...): Promise<Response>` in
    // index.ts): one route handler the same `Promise<Response>` shape as the
    // sibling `handleProgrammaticAgentRegistration` already counted here. It mints
    // a rate-limited free key and grants no payout/settlement authority. Ratchet
    // back down when index.ts route handlers move behind a shared route mapper.
    // +2 (111 -> 113) on 2026-06-25 (#6261) for the live Gym / Harbor run
    // progress routes (`handleOperatorGymRunProgressApi`,
    // `handlePublicGymRunProgressApi` in inference/gym/run-progress-routes.ts):
    // two read-only route handlers returning `Effect.Effect<Response>` like the
    // sibling public-projection handlers already counted here. They serve a
    // public-safe `openagents.gym.run_progress.v1` projection (counts only) and
    // mint no spend/settlement/payout/public-claim authority. Ratchet back down
    // when these route handlers move behind a shared route mapper.
    // +2 (113 -> 115) on 2026-06-25 (#6271) for the live Gym / Harbor run
    // progress PUSH-INGEST split in inference/gym/run-progress-routes.ts: two
    // extracted `: Effect.Effect<Response>` handlers
    // (`handleOperatorListRunProgress`, `handleOperatorIngestRunProgress`) that
    // separate the operator GET-list surface from the new admin-gated POST
    // ingest surface. The ingest handler REBUILDS the pushed snapshot through
    // buildGymRunProgress + checkGymRunProgressPublicSafety (rejecting any
    // prompts/responses/logs/trajectories/keys/private endpoints) and upserts it
    // by runRef into D1; it mints no spend/settlement/payout/public-claim
    // authority. Ratchet back down when these handlers move behind a shared
    // route mapper.
    // +2 (115 -> 117) on 2026-06-26 (#6309) for the recurring published Gym
    // benchmark LADDER routes in inference/gym/ladder-routes.ts: two read-only
    // route handlers returning `Effect.Effect<Response>`
    // (`handlePublicGymLeaderboardApi`, `handleOperatorGymLeaderboardApi`) like
    // the sibling public-projection handlers already counted here. They serve a
    // public-safe `openagents.gym.ladder_leaderboard.v1` projection (the three
    // rungs, decision-grade rows only) and mint no spend/settlement/payout/
    // public-claim authority. Ratchet back down when these route handlers move
    // behind a shared route mapper.
    // +2 (117 -> 119) on 2026-06-26 (#6308) for the recurring published Khala
    // external HEAD-TO-HEAD routes in inference/benchmark/head-to-head-routes.ts:
    // two read-only route handlers returning `Effect.Effect<Response>`
    // (`handlePublicKhalaHeadToHeadApi`, `handleOperatorKhalaHeadToHeadApi`) like
    // the sibling public-projection handlers already counted here. They serve a
    // public-safe `openagents.khala.head_to_head.v1` projection (Khala vs the
    // developer-default comparators, decision-grade rows only, scored on
    // solve-rate AND cost-per-accepted-outcome) and mint no spend/settlement/
    // payout/public-claim authority. Ratchet back down when these route handlers
    // move behind a shared route mapper.
    // +1 (119 -> 120) on 2026-06-27 (#6370) for the admin agent-token reissue
    // handler `handleAdminReissueAgentToken(): Promise<Response>` in index.ts:
    // one admin-gated dead-token recovery handler the same `Promise<Response>`
    // shape as the sibling `handleProgrammaticAgentRegistration` already counted
    // here. It mints a fresh credential for an EXISTING agent entity only and
    // grants no new authority. Ratchet back down when these handlers move behind
    // a shared route mapper.
    // +3 (120 -> 123) on 2026-06-27 (#6378) for the MirrorCode-as-a-service demo
    // route handlers in inference/gym/mirrorcode-routes.ts: read-only public
    // projection + owner-gated record handlers returning `Effect.Effect<Response>`
    // (`handleMirrorCodeRunsApi`, `handleMirrorCodeRunByIdApi`) like the sibling
    // gym public-projection handlers already counted here. They serve a
    // public-safe `openagents.gym.mirrorcode_runs.v1` projection (Khala runs +
    // labeled illustrative paper-reference comparators, never task contents) and
    // mint no spend/settlement/payout/public-claim authority. Ratchet back down
    // when these route handlers move behind a shared route mapper.
    // +9 (123 -> 132) on 2026-07-01 for the Mutalisk Khala-delegation Gym
    // route handlers in inference/gym/mutalisk-khala-delegation-routes.ts
    // (public run projection + owner-gated ingest handlers returning
    // Response, same shape as the sibling gym handlers above). The lane that
    // added the file never bumped this ratchet, leaving check:deploy red on
    // main; this records the actual count. They mint no spend/settlement/
    // payout/public-claim authority. Ratchet back down when these handlers
    // move behind a shared route mapper.
    // -5 (134 -> 129) on 2026-07-05 (#8387) for retiring the standalone
    // default-off MPP/x402 chat endpoint and root MPP discovery document instead
    // of arming them. The keyed Khala gateway and Khala Code plan-purchase
    // Lightning helper remain.
    // +1 (129 -> 130) on 2026-07-05 (#8414) for the new public settled-feed
    // khala-sync projection read route in public-settled-feed-routes.ts
    // (`handlePublicSettledFeedApi`) — one Effect-returning handler, same
    // shape as the sibling public-projection routes already counted here.
    // Mints no spend/settlement/payout/public-claim authority.
    // +2 (130 -> 132) on 2026-07-06 for the Khala Code mobile-only MVP push
    // notification lane (#8485/#8486): `push/push-sender.ts`'s
    // `FetchLike = (url, init) => Promise<Response>` injectable-fetch type
    // (a dependency-injection seam for tests, matching this codebase's
    // existing injectable-fetch convention, not a route handler) and one
    // index.ts route-table handler added by a concurrent lane in the same
    // parallel dispatch. Neither mints spend/settlement/payout/public-claim
    // authority. Ratchet back down when route mappers are extracted.
    budget: 132,
    description:
      'Worker domain and route modules may not grow Response-returning surfaces while route mappers are extracted.',
    details: countByFile(
      workerFiles.filter(path => !path.includes('/http/')),
      /Promise<Response>|:\s*Response\b|Effect\.Effect<Response/g,
    ),
    name: 'Worker Response return surfaces',
  },
  {
    budget: 0,
    description:
      'Converted browser domains must branch on tagged model state or Option, not raw DTO nulls.',
    details: countByFile(
      convertedBrowserDomainFiles,
      /\.agentRunId\s*(?:===|!==)\s*null|\.durationSeconds\s*(?:===|!==)\s*null|detail\.file\.teamId|\.teamId\s*(?:===|!==)\s*null/g,
    ),
    name: 'converted browser domain raw null branches',
  },
  {
    budget: 0,
    description:
      'Browser product permission checks must go through product-policy.ts instead of calling session helpers directly.',
    details: countByFile(
      browserPolicyConsumerFiles,
      /authHasCoreTeamAccess\(/g,
    ),
    name: 'direct browser Core Team permission checks',
  },
  {
    budget: 0,
    description:
      'Project workroom visibility must be owned by product-policy.ts, not reintroduced as a local UI flag.',
    details: countByFile(sourceFiles, /\bPROJECT_WORKROOMS_ENABLED\b/g),
    name: 'legacy project workroom flag',
  },
  {
    budget: 0,
    description:
      'The deleted SIMULATED auth flow must not return. The real /login page (LoginRoute/loginRouter) is allowed — it is a branded launcher into the real OpenAuth flow (/login/github + /login/email one-time code), never a fake/in-app session. These symbols belonged to the removed simulated flow and must stay gone.',
    details: countByFile(
      sourceFiles,
      /\b(StartupRedirectToLogin|RedirectToLogin|SimulateAuthRequest|SaveSession|SucceededSaveSession|FailedSaveSession)\b/g,
    ),
    name: 'deleted simulated login auth symbols',
  },
  {
    budget: 0,
    description:
      'The deleted personal /chat alias must not be reintroduced as a Worker redirect to root.',
    details: countByFile(
      workerFiles,
      /path:\s*['"]\/chat['"](?:(?!path:\s*['"])[\s\S])*redirectResponse\(\s*['"]\/['"]\s*\)/g,
    ),
    name: 'legacy Worker /chat redirect alias',
  },
  {
    budget: 0,
    description:
      'Proof replay visual renderers must be promoted through @openagentsinc/three-effect and the /animations visual taxonomy. App code may adapt bundles, HUD, inspector, and accessibility mirrors, but must not add new DOM/canvas/WebGL replay stages outside the named legacy bridge.',
    details: countByFile(
      proofReplayVisualRendererFiles,
      /document\.createElement\(\s*['"](?:canvas|div|section|button|span|footer|aside)['"]|CanvasRenderingContext2D|new\s+Three\./g,
    ),
    name: 'app-local proof replay visual renderers outside three-effect',
  },
]

const runPromiseAllowlist = new Map([
  // Added 2026-07-01: the Mutalisk Khala-delegation Gym routes bridge their
  // Effect-returning store/read programs once from Promise-shaped route
  // handlers (`const run = (effect) => Effect.runPromise(effect)`), the same
  // named-bridge shape as the billing/operator routes below. Ratchet down if
  // the gym route handlers move to an Effect program.
  ['workers/api/src/inference/gym/mutalisk-khala-delegation-routes.ts', 1],
  // index.ts raised 6 -> 7 on 2026-06-14 for the wave-3 tenant-client
  // integration bridge; ratchet back down with the Effect-program migration.
  // Raised 7 -> 8 on 2026-07-05 (#8282 Promise.all landmine audit follow-up):
  // `notifyCanceledAgentRunSyncScopesEffect` bridges the isolated,
  // per-run sync-scope notify effect into `enforceOutOfCreditsPolicy`'s
  // Promise-shaped body, so one canceled run's notify failure can never
  // again block the SHC compute-cleanup dispatch or the out-of-credits
  // email for the rest of the batch. Named bridge; ratchet down if
  // `enforceOutOfCreditsPolicy` becomes an Effect program end-to-end.
  // Raised 8 -> 9 on 2026-07-06 for the Khala Code mobile-only MVP $10
  // GitHub-account-keyed signup credit grant (#8478): the GitHub
  // sign-in callback bridges the Effect-returning idempotent grant call
  // (`grantGithubSignupCredit`) once from the Promise-shaped OAuth
  // callback body, the same named-bridge shape as the sibling grant
  // above. Never blocks or fails sign-in on error (fail-soft, logged).
  // This lane never bumped the ratchet, leaving check:deploy red on
  // main; this records the actual count. Ratchet down if the sign-in
  // callback becomes an Effect program end-to-end.
  ['workers/api/src/index.ts', 9],
  ['workers/api/src/observability.ts', 1],
  ['workers/api/src/omni-handlers.ts', 7],
  ['workers/api/src/thread-access.ts', 1],
  ['workers/api/src/onboarding/repository.ts', 1],
  ['packages/sync-worker/src/index.ts', 1],
  // Added 2026-06-17: the homepage pylon-stats boot-payload injector runs the
  // Effect-returning public-stats handler once from the asset-shell path to
  // SSR-seed the snapshot. Named bridge; ratchet down if the asset path moves
  // to an Effect program.
  ['workers/api/src/http/pylon-stats-boot-payload.ts', 1],
  // Added 2026-06-18: the forum thread document SSR path runs the
  // Effect-returning topic-detail read once to derive per-thread Open Graph /
  // Twitter Card metadata. Named bridge; ratchet down if the forum read path
  // moves to an Effect program. Keeping it here (not in index.ts) holds the
  // index.ts bridge budget flat.
  ['workers/api/src/http/forum-social-preview.ts', 1],
  // Added 2026-06-19 (#5497): the billing route runs the Effect-returning
  // USD->msat credit bridge (`fundInferenceFromCredit`) once from the
  // Promise-based `/api/billing/inference-credit` handler. Named bridge; ratchet
  // down if the billing route handlers move to an Effect program.
  ['workers/api/src/billing-routes.ts', 1],
  // Added 2026-06-19 (#5508): the operator inference-credit route runs the same
  // Effect-returning USD->msat credit bridge (`fundInferenceFromCredit`) once
  // from the Promise-based `/api/omni/operator/billing/inference-credit` admin
  // handler — the operator mirror of the #5497 self-serve bridge above. Named
  // bridge; ratchet down if the operator billing handlers move to an Effect
  // program.
  ['workers/api/src/operator-billing-routes.ts', 1],
  // Added 2026-06-22 (#6035 / refs #6027): the inference gateway TRUE
  // pass-through SSE stream (the khala-code 524 fix) bridges the Effect-returning
  // metering hook into the Web Streams `ReadableStream.start` controller callback
  // ONCE, after the upstream stream drains, to settle metering receipt-first from
  // the terminal usage frame. The Web Streams controller API is not Effect-native
  // and streaming must flow incrementally (no server-side buffering, so the edge
  // idle-timer resets and long generations never 524), so the metering Effect is
  // run at that boundary. Named bridge; ratchet down if the streaming response is
  // expressed as an Effect Stream program end-to-end.
  ['workers/api/src/inference/chat-completions-routes.ts', 1],
  // Added 2026-06-23 (#6123 UI follow-up): the /autopilot onboarding streaming
  // turn route bridges the Effect-returning finalize step (append + persist) into
  // the Web Streams `ReadableStream.start` controller callback ONCE, after the
  // prose deltas drain, to commit the turn receipt-first. Same boundary as the
  // chat-completions SSE bridge above (the controller API is not Effect-native and
  // streaming must flow incrementally). Named bridge; ratchet down if the
  // onboarding stream is expressed as an Effect Stream program end-to-end.
  ['workers/api/src/autopilot-onboarding-routes.ts', 1],
  // Added 2026-06-26: the public Khala chat SSE route bridges
  // the Effect-returning served-token recorder into the Web Streams
  // `ReadableStream.start` controller callback once, after the upstream stream
  // drains and terminal usage metadata is available. Same receipt-first
  // streaming boundary as chat-completions and onboarding; ratchet down if the
  // public Khala chat stream becomes an Effect Stream program end-to-end.
  ['workers/api/src/khala-chat-routes.ts', 1],
  // Added 2026-06-27 (#6359): the Artanis network-stats D1 reader runs the
  // combined token-usage ledger reads once to assemble the live token-pace
  // snapshot for the operator agent (situational awareness + the
  // get_network_stats tool). The openagents.com Worker cannot reliably
  // HTTP-fetch its OWN public /stats zone (a same-zone loopback subrequest
  // returns empty), so it reads the ledger from D1 directly; the awareness
  // reader and the tool are Promise-shaped while the ledger reads are Effects.
  // Named bridge; ratchet down if these move to an Effect program.
  ['workers/api/src/artanis-network-stats-d1.ts', 1],
  // Added 2026-07-05 (KS-6.7, #8417): the tokens-served aggregates refresh
  // bridges the Effect-returning ledger reads (readPublicTokensServedModelMix
  // / ...DemandMix / ...ChannelMix / ...History) into the Promise-shaped
  // khala-sync projection refresh sweep, via ONE shared named helper
  // (`runEffect`) reused for all four ledger calls per window. Named bridge;
  // ratchet down if the refresh sweep becomes an Effect program end-to-end.
  ['workers/api/src/khala-sync-public-tokens-served-mix.ts', 1],
  // Added 2026-07-05 (#8282 Promise.all landmine audit follow-up): each of
  // these Promise-shaped functions previously fanned out an independent,
  // unrelated batch of async work through a bare `Promise.all`, where one
  // item's rejection silently discarded visibility into (or delivery for)
  // every sibling item — the same failure class as the #8409 severe
  // write-loss incident. Each now runs its per-item-isolated fan-out
  // (`Effect.forEach` + `Effect.result`/`Effect.catch`, one item's failure
  // logged and skipped rather than aborting its siblings) via ONE named
  // `Effect.runPromise` bridge at the existing Promise-shaped boundary.
  // Named bridges; ratchet down as these call sites migrate to Effect
  // programs end-to-end. See
  // docs/2026-07-05-promise-all-cron-landmine-audit.md for the full list.
  ['workers/api/src/runtime.ts', 1],
  ['workers/api/src/tassadar-settled-feed-sync.ts', 1],
  ['workers/api/src/treasury-routes.ts', 1],
  ['workers/api/src/relay-health.ts', 1],
  ['workers/api/src/pylon-capacity-funnel-live-routes.ts', 2],
  ['workers/api/src/forge-control-plane-routes.ts', 1],
  ['workers/api/src/operator-provider-account-routes.ts', 1],
  ['workers/api/src/agent-definition-run-routes.ts', 2],
])

const runPromiseDetails = countByFile(sourceFiles, /Effect\.runPromise\(/g)

const runPromiseProblems = runPromiseDetails.flatMap(result => {
  const budget = runPromiseAllowlist.get(result.path)

  if (budget === undefined) {
    return [
      `${result.path}: ${result.count} Effect.runPromise call(s) are outside the named temporary bridge allowlist.`,
    ]
  }

  return result.count > budget
    ? [
        `${result.path}: ${result.count} Effect.runPromise call(s), budget ${budget}.`,
      ]
    : []
})

const lineBudgetChecks = [
  {
    budget: 228,
    count: lineCount('apps/web/src/page/loggedIn/update.ts'),
    description:
      'The parent logged-in update must remain a dispatcher over domain command and transition modules.',
    name: 'loggedIn/update.ts lines',
    path: 'apps/web/src/page/loggedIn/update.ts',
  },
  {
    budget: 0,
    count: countMatches(
      read('apps/web/src/page/loggedIn/update.ts'),
      /Command\.define\(/g,
    ),
    description:
      'The parent logged-in update may not define request commands; command definitions belong in domain command modules.',
    name: 'loggedIn/update.ts Command.define count',
    path: 'apps/web/src/page/loggedIn/update.ts',
  },
  {
    budget: 0,
    count: countMatches(
      read('apps/web/src/page/loggedIn/update.ts'),
      /\bevo\(/g,
    ),
    description:
      'The parent logged-in update may not perform direct struct mutations; pure transitions belong in domain transition modules.',
    name: 'loggedIn/update.ts evo count',
    path: 'apps/web/src/page/loggedIn/update.ts',
  },
  {
    budget: 606,
    count: lineCount('apps/web/src/page/loggedIn/page/chat.ts'),
    description:
      'The chat page must stay focused on rendering; run timeline projection belongs in run-timeline/projection.ts.',
    name: 'loggedIn/page/chat.ts lines',
    path: 'apps/web/src/page/loggedIn/page/chat.ts',
  },
  {
    budget: 0,
    count: countMatches(
      read('apps/web/src/page/loggedIn/page/chat.ts'),
      /JSON\.parse\(/g,
    ),
    description:
      'The chat page may not parse runner payload JSON directly; projection parsing belongs in the typed run-timeline module.',
    name: 'loggedIn/page/chat.ts JSON.parse count',
    path: 'apps/web/src/page/loggedIn/page/chat.ts',
  },
]

const deletedFileChecks = [
  {
    description:
      'UI implementations must live in typed family modules rather than the deleted compatibility registry.',
    name: 'ui/registry.ts deleted',
    path: 'apps/web/src/ui/registry.ts',
  },
  {
    description:
      'The deleted local login demo must not return; root plus /login/github are the public auth surface.',
    name: 'loggedOut/page/login.ts deleted',
    path: 'apps/web/src/page/loggedOut/page/login.ts',
  },
  {
    description:
      'The deleted local login demo story must not return with the removed simulated auth flow.',
    name: 'loggedOut/page/login.story.test.ts deleted',
    path: 'apps/web/src/page/loggedOut/page/login.story.test.ts',
  },
]

const reportOnlyArchitectureChecks = [
  {
    description:
      'Khala desktop/tools modules should parse untyped JSON through Schema or named boundaries before casting.',
    details: countByFile(
      khalaArchitectureScanFiles,
      /JSON\.parse\([\s\S]{0,240}?\)\s+as\b/g,
    ),
    name: 'Khala JSON.parse casts',
  },
  {
    description:
      'Khala desktop/tools modules should preserve failure information instead of swallowing empty catch blocks.',
    details: countByFile(
      khalaArchitectureScanFiles,
      /\bcatch\s*(?:\([^)]*\)\s*)?\{\s*\}/g,
    ),
    name: 'Khala bare catch blocks',
  },
  {
    description:
      'Khala desktop/tools modules should route environment reads through config services or explicit boundaries.',
    details: countByFile(
      khalaArchitectureScanFiles,
      /\b(?:process|Bun)\.env\b|\bimport\.meta\.env\b/g,
    ),
    name: 'Khala direct env reads',
  },
  {
    description:
      'Khala desktop/tools logic should use injected Clock/time services instead of Date.now().',
    details: countByFile(khalaArchitectureScanFiles, /\bDate\.now\(\)/g),
    name: 'Khala Date.now calls',
  },
  {
    description:
      'Khala desktop/tools modules should keep Effect.runPromise at named executable edges.',
    details: countByFile(
      khalaArchitectureScanFiles,
      /\bEffect\.runPromise\(/g,
    ),
    name: 'Khala Effect.runPromise calls',
  },
  {
    description:
      'Khala desktop/tools process cleanup should use supervised lifecycles instead of setTimeout kill paths.',
    details: countByFile(
      khalaArchitectureScanFiles,
      /\bsetTimeout\s*\([\s\S]{0,400}\b(?:killTree|process\.kill|kill|\.kill)\b/g,
    ),
    name: 'Khala setTimeout process kills',
  },
]

// Public landing pages may use a little local layout glue, but they must not
// drift back into page-local Tailwind-only composition. Keep route-specific
// shared component symbols here; DOM-level marker assertions live with the
// route tests where Foldkit rendering is available.
const publicLandingCompositionChecks = [
  {
    // 2026-07-02 owner-directed redesign: /business is dark-only on the
    // DESIGN.md operational surface, so the light/system theme selector and
    // theme script are intentionally gone. The local-class budget covers the
    // page-local Khala intake console (KHALA · INTAKE strip, transcript,
    // composer); ratchet it back down if/when the console is extracted into
    // @openagentsinc/ui as a shared component.
    maxLocalClassCalls: 14,
    module: 'apps/web/src/page/business.ts',
    requiredSymbols: [
      'Ui.publicLandingThemeShell',
      'Ui.businessLandingHero',
      'Ui.businessOfferingMenu',
      'Ui.quickWinLadder',
      'Ui.businessProjectInvite',
      'Ui.businessIntakeForm',
      "DataAttribute('business-intake-chat'",
    ],
    route: '/business',
  },
]

// ---------------------------------------------------------------------------
// Public projection staleness ratchet (epic #4751).
//
// Invariant: every public projection carries generatedAt (or
// generatedAtUnixMs) plus a declared staleness contract from
// workers/api/src/public-projection-staleness.ts. The inventory below
// is the projection-surface ledger; the enforced rules are:
//   1. Every `/api/public/...` route literal discovered in route
//      modules must be covered by a ledger row — a NEW public
//      projection route fails this check until it is added here, and
//      it can only be added as `staleness_declared`; the former legacy
//      budget is now zero.
//   2. Every `staleness_declared` row's payload module must actually
//      reference the shared staleness contract (the grep token is
//      `maxStalenessSeconds` or the module import path).
//   3. Every row must be either `staleness_declared` or
//      `static_contract_exempt`; no legacy status remains available.
//
// The check is module-granular: a shared route module with one
// declared projection does not prove every route in it is compliant.
// Route-level truth lives in the inventory in
// apps/openagents.com/INVARIANTS.md ("Public Projection Staleness
// Declaration"); rows below mirror it.
// ---------------------------------------------------------------------------

const publicProjectionComplianceToken =
  /maxStalenessSeconds|public-projection-staleness/
const publicProjectionAllowedStatuses = new Set([
  'staleness_declared',
  'static_contract_exempt',
])

const publicProjectionSurfaces = [
  // Declared surfaces (payload carries generatedAt + staleness contract).
  {
    module: 'workers/api/src/public-forum-activity-routes.ts',
    route: '/api/public/forum-activity',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/labor-earnings-routes.ts',
    route: '/api/public/labor-earnings',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/public-khala-tokens-served-routes.ts',
    route: '/api/public/khala-tokens-served',
    status: 'staleness_declared',
  },
  {
    // KS-6.4 (#8414): the live settled-feed khala-sync projection's new
    // public, unauthenticated read route — serves the scope.public.
    // settled-feed projection (rebuilt_on_transition) with a fail-open
    // fallback to the legacy D1 sync-outbox snapshot (live_at_read); both
    // branches carry generatedAt + the shared staleness contract.
    module: 'workers/api/src/public-settled-feed-routes.ts',
    route: '/api/public/settled-feed',
    status: 'staleness_declared',
  },
  // Added 2026-07-01: the Mutalisk Khala-delegation Gym run projection was
  // introduced by the gym bridge lane with the staleness contract wired
  // (liveAtReadStaleness) but never registered here; registering closes the
  // pre-existing check:deploy red on main.
  {
    module: 'workers/api/src/inference/gym/mutalisk-khala-delegation-routes.ts',
    route: '/api/public/gym/mutalisk-khala-delegation/runs',
    status: 'staleness_declared',
  },
  {
    // Khala Code plan catalog (khala_code.free_paid_plans.v1, #7966): static
    // catalog text plus one deployment-config input (the fail-closed
    // KHALA_CODE_PAID_PLANS_ENABLED read), recomputed on every read —
    // live_at_read with generatedAt + the shared staleness contract.
    module: 'workers/api/src/inference/khala-code-plan-routes.ts',
    route: '/api/public/khala-code/plans',
    status: 'staleness_declared',
  },
  {
    // Khala Code public install-truth counter (#8246): exact grouped rows from
    // khala_code_download_events only, or an empty response with blocker refs.
    // It is live_at_read with generatedAt + the shared staleness contract.
    module: 'workers/api/src/khala-code-download-counts-routes.ts',
    route: '/api/public/khala-code/download-counts',
    status: 'staleness_declared',
  },
  {
    // Khala Code outside-user run receipt readback (#8247): one public-safe
    // receipt by receiptRef from khala_code_outside_user_run_receipts. It is
    // live_at_read with generatedAt + the shared staleness contract and stores
    // no paths, prompts, tokens, logs, account ids, or machine ids.
    module: 'workers/api/src/khala-code-outside-user-run-routes.ts',
    route: '/api/public/khala-code/outside-user-runs/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    // Khala Code trace->plugin->revenue-share precedent readback (#8251): one
    // public-safe receipt by receiptRef from the settled-precedent ledger.
    // It is live_at_read with generatedAt + the shared staleness contract.
    module:
      'workers/api/src/khala-code-trace-plugin-revenue-share-routes.ts',
    route:
      '/api/public/khala-code/trace-plugin-revenue-share-precedents/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    // QA Swarm first-engagement receipt readback (#8252): one public-safe
    // operator-assisted Swarm Audit commitment by receiptRef from the
    // first-engagement, workspace, service-promise, and commitment-ledger rows.
    // It is live_at_read with generatedAt + the shared staleness contract.
    module: 'workers/api/src/qa-swarm-first-engagement-routes.ts',
    route: '/api/public/qa-swarm/first-engagements/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    // RL-9 first-dollar evidence bundle readback (#8253): one public-safe
    // revenue event by bundleRef from the revenue_event_provenance ledger.
    // It is live_at_read with generatedAt + the shared staleness contract.
    module: 'workers/api/src/revenue-event-provenance.ts',
    route: '/api/public/revenue-loop/first-dollar-evidence/{bundleRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/marketing-agency-receipt-public-routes.ts',
    route: '/api/public/marketing-agency/receipts',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/marketing-agency-self-serve-public-routes.ts',
    route: '/api/public/marketing-agency/self-serve/deliverability',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/coding-quick-win-pipeline-routes.ts',
    route: '/api/public/business/coding-quick-win-pipeline',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/coding-quick-win-receipt-public-routes.ts',
    route: '/api/public/business/coding-quick-win-receipts',
    status: 'staleness_declared',
  },
  {
    module:
      'workers/api/src/business-already-sold-engagement-receipt-routes.ts',
    route: '/api/public/business/already-sold-engagement-receipts',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/artanis-public-report.ts',
    route: '/api/public/artanis/report',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/artanis-activity-routes.ts',
    route: '/api/public/artanis/activity',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/artanis-tick-streak.ts',
    route: '/api/public/artanis/tick-streak',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/artanis-distillation-dataset-receipt.ts',
    route: '/api/public/artanis/tassadar-distillation-dataset',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/artanis-responder-provenance.ts',
    route: '/api/public/artanis/responder-support',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/artanis-labor-receipt-routes.ts',
    route: '/api/public/artanis/labor-receipts',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/artanis-labor-green-readiness.ts',
    route: '/api/public/artanis/labor-green-readiness',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/artanis-activity-routes.ts',
    route: '/api/public/artanis/activity',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/forum/tip-earnings.ts',
    route: '/api/forum/tip-leaderboards',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/forum/tip-earnings.ts',
    route: '/api/forum/moderation/tip-earnings',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/forum/tip-earnings.ts',
    route: '/api/forum/actors/{actorRef}/tip-earnings',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/forum/repository.ts',
    route: 'forum post tipStats blocks (topic/post/list payloads)',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/forum-routes.ts',
    route: '/api/agents/profiles/{profileRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/x-claim-reward-eligibility-routes.ts',
    route: '/api/agents/claims/rewards',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/provider-account-pool-routes.ts',
    route: '/api/provider-accounts/pool',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/provider-account-usage-routes.ts',
    route: '/api/admin/provider-accounts/usage',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/relay-health-routes.ts',
    route: '/api/public/relay-health',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/accepted-outcomes-per-kwh.ts',
    route: '/api/public/metrics/accepted-outcomes-per-kwh',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/verified-outcome-reputation.ts',
    route: '/api/public/reputation/verified-outcomes',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/inference/gym/run-progress-routes.ts',
    route: '/api/public/gym/run-progress',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/inference/gym/ladder-routes.ts',
    route: '/api/public/gym/leaderboard',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/inference/benchmark/head-to-head-routes.ts',
    route: '/api/public/khala/head-to-head',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/omni-contributor-accrual-bundle-routes.ts',
    route: '/api/public/payments/contributor-accrual-bundle',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/demand-provenance.ts',
    route: '/api/public/demand-provenance',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/open-markets-surface.ts',
    route: '/api/public/markets/open-markets',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/open-markets-skeletons.ts',
    route: '/api/public/markets/liquidity/skeleton',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/open-markets-skeletons.ts',
    route: '/api/public/markets/risk/skeleton',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/marketplace-product-composition.ts',
    route: '/api/public/marketplace/composed-products',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/wasm-plugin-marketplace.ts',
    route: '/api/public/marketplace/wasm-plugins',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/autopilot-composed-run.ts',
    route: '/api/public/autopilot/composed-runs',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/agentic-labor-product.ts',
    route: '/api/public/autopilot/labor-products',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/self-serve-fanout.ts',
    route: '/api/public/autopilot/self-serve-fanout',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/marketplace-work-class-catalog.ts',
    route: '/api/public/autopilot/marketplace-work-classes',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/signature-usage-metering.ts',
    route: '/api/public/markets/signature-monetization/metering',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/pylon-multi-earning-node.ts',
    route: '/api/public/pylon/multi-earning-node',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/omni-client-delivery-projection-routes.ts',
    route: '/api/public/omni/client-delivery-projection',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/customer-one-cohort-projection.ts',
    route: '/api/public/customer-one-cohort',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/business-signup-routes.ts',
    route: '/api/public/business-signup',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/business-funnel-dashboard-routes.ts',
    route: '/api/public/business/funnel-dashboard',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/business-intake-chat-routes.ts',
    route: '/api/public/business-intake-chat',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/training-run-window-routes.ts',
    route: '/api/public/training/runs/{trainingRunRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/training-run-window-routes.ts',
    route: '/api/public/training/runs/{trainingRunRef}/settlements',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/training-verification-routes.ts',
    route: '/api/public/training/verification-challenges/{challengeRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/public-tassadar-run-summary-routes.ts',
    route: '/api/public/tassadar-run-summary',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/public-activity-timeline.ts',
    route: '/api/public/activity-timeline',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/tassadar-compiled-module-marketplace.ts',
    route: '/api/public/tassadar/compiled-module-marketplace',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/public-proof-replay-routes.ts',
    route: '/api/public/proof-replays',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/public-proof-replay-routes.ts',
    route: '/api/public/tassadar-replays/first-real-settlement',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/replay-clip-job-routes.ts',
    route: '/api/public/replay-clips',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/replay-clip-job-routes.ts',
    route: '/api/public/replay-clips/{jobRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/site-referral-payout-public-projection.ts',
    route: '/api/public/site-referral-payouts',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/site-referral-payout-receipts.ts',
    route: '/api/public/site-referral-payout-receipts/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/partner-payout-public-projection.ts',
    route: '/api/public/partner-payouts',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/partner-payout-receipts.ts',
    route: '/api/public/partner-payout-receipts/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/promise-transition-audit-routes.ts',
    route: '/api/public/product-promises/audit',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/promise-transition-receipt-routes.ts',
    route: '/api/public/product-promises/transitions',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/product-promises.ts',
    route: '/api/public/product-promises',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/training-ablation-derisking-ledger.ts',
    route: '/api/public/training/ablation-derisking-ledger',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/training-post-training-instruct-sft.ts',
    route: '/api/public/training/post-training-arc/instruct-sft-lane',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/public-accepted-outcome-settlement-routes.ts',
    route: '/api/public/accepted-outcome/settlement/{economicsId}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/inference-receipts.ts',
    route: '/api/public/inference/receipts/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/cloud/cloud-primitive-receipts.ts',
    route: '/api/public/cloud/receipts/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/stripe-checkout-receipts.ts',
    route: '/api/public/billing/stripe-checkout-receipts/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/inference/card-credit-spend-receipt-store.ts',
    route: '/api/public/inference/card-credit-spend-receipts/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/inference/inference-privacy-receipt-routes.ts',
    route: '/api/public/inference/privacy-receipts',
    status: 'staleness_declared',
  },
  // Static contract documents, not state projections.
  {
    module: 'workers/api/src/index.ts',
    route: '/api/public/home',
    status: 'static_contract_exempt',
  },
  {
    // Free-tier data-sharing disclosure (#6296): a static terms/policy contract
    // (version + ordered terms + bounded policy facts), not a live state
    // projection, so it carries no generatedAt/staleness contract.
    module: 'workers/api/src/inference/free-tier-data-sharing-routes.ts',
    route: '/api/public/free-tier-data-sharing',
    status: 'static_contract_exempt',
  },
  {
    module: 'workers/api/src/ecommerce-campaign-receipt-routes.ts',
    route: '/api/public/ecommerce-campaign/receipts',
    status: 'static_contract_exempt',
  },
  {
    module: 'workers/api/src/ecommerce-campaign-self-serve-routes.ts',
    route: '/api/public/ecommerce-campaign/workspaces',
    status: 'staleness_declared',
  },
  // Wave 1 retrofit complete: the former legacy surfaces now declare
  // generatedAt plus the shared projection_staleness.v1 contract.
  {
    module: 'workers/api/src/public-otec-proof.ts',
    route: '/api/public/proof/otec',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/public-pylon-stats.ts',
    route: '/api/public/pylon-stats',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/pylon-capacity-funnel-live-routes.ts',
    route: '/api/public/pylon-capacity-funnel',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/pylon-capacity-funnel-live-routes.ts',
    route: '/api/public/pylon-capacity-funnel/history',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/public-launch-dashboard.ts',
    route: '/api/public/launch-dashboard',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/treasury-routes.ts',
    route: '/api/public/treasury/launch-status',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/treasury-page-routes.ts',
    route: '/api/public/treasury',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/artanis-tick-monitor.ts',
    route: '/api/public/artanis/admin-ticks',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/nexus-pylon-visibility.ts',
    route: '/api/public/nexus-pylon/receipts/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/nip90-market-receipts.ts',
    route: '/api/public/nip90-market/receipts/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/adjutant-public-activity.ts',
    route: '/api/public/adjutant/activity',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/agent-goal-public-projection.ts',
    route: '/api/public/goals/{goalId}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/agent-goal-public-projection.ts',
    route: '/api/public/agents/{agentRef}/goal',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/forum-routes.ts',
    route: '/api/forum/launch-status',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/forum-routes.ts',
    route: '/api/forum/receipts/{receiptRef}',
    status: 'staleness_declared',
  },
  {
    module: 'workers/api/src/training-run-window-routes.ts',
    route: '/api/training/runs (window/leaderboard/eval surfaces)',
    status: 'staleness_declared',
  },
]

const PUBLIC_PROJECTION_LEGACY_BUDGET = 0

const normalizedPublicRoute = value =>
  value.replaceAll('\\', '').replace(/\/+$/, '')

const discoveredPublicRoutes = [
  ...new Set(
    routeFiles.flatMap(path =>
      Array.from(
        read(path).matchAll(/\/api\\?\/public(?:\\?\/[A-Za-z0-9_.$-]+)*/g),
      ).map(match => normalizedPublicRoute(match[0])),
    ),
  ),
].sort()

const ledgerRoutePaths = publicProjectionSurfaces
  .map(surface => surface.route.split(' ')[0].replace(/\{[^}]+\}.*$/, ''))
  .map(route => route.replace(/\/+$/, ''))
  .filter(route => route.startsWith('/api/'))

const publicRouteIsKnown = route =>
  ledgerRoutePaths.some(
    known => route === known || route.startsWith(`${known}/`),
  )

const publicProjectionProblems = [
  ...discoveredPublicRoutes
    .filter(route => route !== '/api/public' && !publicRouteIsKnown(route))
    .map(
      route =>
        `public projection route ${route} is not in the projection-surface ledger. ` +
        'New public projections must declare generatedAt plus the staleness contract from ' +
        'workers/api/src/public-projection-staleness.ts, be added to this ledger as ' +
        "'staleness_declared', and be added to the inventory in INVARIANTS.md (epic #4751).",
    ),
  ...publicProjectionSurfaces
    .filter(surface => surface.status === 'staleness_declared')
    .filter(
      surface =>
        !existsSync(surface.module) ||
        !publicProjectionComplianceToken.test(read(surface.module)),
    )
    .map(
      surface =>
        `public projection module ${surface.module} (${surface.route}) is marked ` +
        'staleness_declared but does not reference the shared staleness contract.',
    ),
  ...publicProjectionSurfaces
    .filter(surface => !publicProjectionAllowedStatuses.has(surface.status))
    .map(
      surface =>
        `public projection module ${surface.module} (${surface.route}) has retired status ` +
        `${surface.status}. Rows must be 'staleness_declared' or 'static_contract_exempt'; ` +
        `legacy budget is ${PUBLIC_PROJECTION_LEGACY_BUDGET}.`,
    ),
]

const budgetProblems = budgetChecks.flatMap(check => {
  const count = totalCount(check.details)

  return count > check.budget
    ? [
        `${check.name}: count ${count}, budget ${check.budget}.\n${formatDetails(check.details)}`,
      ]
    : []
})

const lineBudgetProblems = lineBudgetChecks.flatMap(check =>
  check.count > check.budget
    ? [
        `${check.name}: ${check.path} has ${check.count}, budget ${check.budget}.`,
      ]
    : [],
)

const deletedFileProblems = deletedFileChecks.flatMap(check =>
  existsSync(check.path)
    ? [`${check.name}: ${check.path} must not exist.`]
    : [],
)

const publicLandingCompositionResults = publicLandingCompositionChecks.map(
  check => {
    const text = existsSync(check.module) ? read(check.module) : ''

    return {
      ...check,
      localClassCalls: countMatches(text, /Ui\.className(?:<|\()/g),
      missingSymbols: check.requiredSymbols.filter(
        symbol => !text.includes(symbol),
      ),
      present: existsSync(check.module),
    }
  },
)

const publicLandingCompositionProblems =
  publicLandingCompositionResults.flatMap(result => [
    ...(result.present
      ? []
      : [
          `public landing route ${result.route}: ${result.module} is missing.`,
        ]),
    ...result.missingSymbols.map(
      symbol =>
        `public landing route ${result.route}: ${result.module} is missing shared component ${symbol}. ` +
        'Public landing pages must compose through @openagentsinc/ui families, not only local Ui.className blocks.',
    ),
    ...(result.localClassCalls > result.maxLocalClassCalls
      ? [
          `public landing route ${result.route}: ${result.module} has ${result.localClassCalls} ` +
            `Ui.className calls, budget ${result.maxLocalClassCalls}. Keep local classes to layout glue and move repeated sections into @openagentsinc/ui.`,
        ]
      : []),
  ])

console.log('Zero-debt architecture budget report')
console.log('')

budgetChecks.forEach(check => {
  const count = totalCount(check.details)
  console.log(`${check.name}: ${count}/${check.budget}`)
  console.log(`  ${check.description}`)
  console.log(formatDetails(check.details) || '  none')
  console.log('')
})

console.log('Effect.runPromise temporary bridge allowlist:')
runPromiseDetails.forEach(result => {
  const budget = runPromiseAllowlist.get(result.path)
  const budgetText = budget === undefined ? 'not allowed' : `budget ${budget}`
  console.log(`${result.path}: ${result.count} (${budgetText})`)
})
console.log('')

lineBudgetChecks.forEach(check => {
  console.log(`${check.name}: ${check.count}/${check.budget}`)
  console.log(`  ${check.description}`)
})
console.log('')

deletedFileChecks.forEach(check => {
  console.log(
    `${check.name}: ${existsSync(check.path) ? 'present' : 'deleted'}`,
  )
  console.log(`  ${check.description}`)
})
console.log('')

console.log('public landing shared component composition:')
publicLandingCompositionResults.forEach(result => {
  console.log(
    `${result.route}: ${result.module} ` +
      `${result.localClassCalls}/${result.maxLocalClassCalls} local Ui.className calls`,
  )
  console.log(
    `  required shared symbols: ${
      result.missingSymbols.length === 0
        ? 'all present'
        : `missing ${result.missingSymbols.join(', ')}`
    }`,
  )
})
console.log('')

console.log(
  `public projection staleness ledger (epic #4751): ${publicProjectionSurfaces.length} surfaces, ` +
    `legacy budget ${PUBLIC_PROJECTION_LEGACY_BUDGET}`,
)
publicProjectionSurfaces.forEach(surface => {
  console.log(`  ${surface.status}: ${surface.route} (${surface.module})`)
})
console.log('')

console.log(
  'Khala architecture report-only scan (clients/khala-code-desktop + packages/khala-tools):',
)
reportOnlyArchitectureChecks.forEach(check => {
  const count = totalCount(check.details)
  console.log(`${check.name}: ${count} finding(s)`)
  console.log(`  ${check.description}`)
  console.log(formatRepoRootDetails(check.details) || '  none')
  console.log('')
})

const problems = [
  ...budgetProblems,
  ...runPromiseProblems,
  ...lineBudgetProblems,
  ...deletedFileProblems,
  ...publicLandingCompositionProblems,
  ...publicProjectionProblems,
]

if (problems.length > 0) {
  console.error('Zero-debt architecture check failed:')
  problems.forEach(problem => console.error(`- ${problem}`))
  process.exit(1)
}

console.log('Zero-debt architecture check passed.')
