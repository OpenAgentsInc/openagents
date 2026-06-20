# 2026-06-19 Autopilot / Autopilot Sites Yellow→Green Readiness Receipt

This is a dereferenceable green-readiness receipt for the non-green
`autopilot.*` and `autopilot_sites.*` product-promise records. For each promise
it records the current state, the runnable verification command and its observed
output, what was built/assembled in this pass, the exact dereferenceable receipt
still needed to go green, and whether the final flip is owner-gated.

It exists so the registry can cite a single independently re-runnable proof per
promise instead of prose, and so an owner can dereference the evidence before any
state flip.

## Honest scope (read first)

- **No promise is flipped to green by this receipt.** Green requires a
  dereferenceable receipt **plus owner sign-off**; this document assembles the
  evidence and builds the last-mile code/tests, leaving the flip owner-gated.
- All values below are refs, test counts, and exit/verify outcomes only. No raw
  prompts, transcripts, credentials, wallet material, or raw local paths.
- "Software-complete, tested" means the source and unit/route tests for the
  yellow scope pass; it does **not** mean the live runtime, settlement, or
  customer self-serve path is wired unless explicitly stated.

## Environment

- Repo worktree base commit: `f6e1f9ac4` (clean `origin/main`), branch
  `assault-autopilot`.
- Runtime: `bun 1.3.11` on macOS arm64.
- `workers/api` tests run under `vitest run` (Cloudflare Workers pool). Raw
  `bun test` cannot resolve the `cloudflare:workers` virtual module and is not
  the API test runner.
- `apps/web` tests run under `vitest run`.
- `apps/autopilot-desktop` tests run per-file via `scripts/run-tests.sh`
  (the #5026 bun load-hang means co-loading all files fails; each file passes
  on its own).

## Consolidated test run (this pass)

`workers/api` (`npx vitest run` over the autopilot/sites modules):

| Module test files | Tests |
| --- | --- |
| site-form-spec-registry, site-page-form-routes, site-page-kinds | included |
| email-sequence-authoring(+routes), list-sequence-enrollment | included |
| tenant-custom-hostnames, cloudflare-custom-hostname-client | included |
| partner-payout-ledger(+routes), site-referral-payout-{wire,adapter,ledger,ledger-routes} | included |
| autopilot-work-routes (mission briefing) | included |
| **Total** | **14 files, 166 tests, all pass** |

`apps/web`: `credits-panel.test.ts` — 24 pass.

## Per-promise readiness

### autopilot.mission_briefing.v1 — yellow → green-READY (code built this pass)

- **Built:** added real `risk` and `receipts` rollups to the Mission Briefing
  projection (`autopilot-mission-briefing.ts`). `risk` rolls up review caveats,
  blocker count, delivery/worktree/change-capture statuses, the
  settlement-blocked reason, and a derived `clear|attention|blocked` level.
  `receipts` rolls up authority-receipt refs, proof refs, verification refs, the
  buyer-payment proof, and settlement eligibility. This closes the
  `cost_risk_receipt_rollup_missing` blocker (previously the briefing carried
  only a cost rollup).
- **Verify:** `npx vitest run src/autopilot-work-routes.test.ts` → 37 pass.
  The delivered-work-order test now asserts the populated `risk`
  (`level: attention`) and `receipts` rollups and that no secret/raw-path
  strings leak.
- **Receipt needed for green:** at least one **live mission** (real work order)
  whose `GET /api/autopilot/work/{workOrderRef}/briefing` JSON is captured with
  a decision-needed state, artifact/test refs, the cost/risk/receipt rollups,
  and public-safe proof refs — referenced from this doc.
- **Owner-gated:** yes (live mission capture + flip).

### autopilot_sites.native_email_sequences.v1 — yellow → green-READY (code built this pass)

- **Built:** the documented missing "home for site form-specs" now exists as a
  typed, tested registry: `site-form-spec-registry.ts` resolves a
  `FormCaptureSpec` by id from a published site/version `metadata_json`
  (`metadata_json.formSpecs` map), agreeing key↔`spec.id`, and degrades to an
  empty registry (→ route 404) on malformed input instead of throwing. This is
  the resolver `site-page-form-routes.ts` injects as `lookupFormSpec`, so the
  public capture route is now wireable.
- **Verify:** `npx vitest run src/site-form-spec-registry.test.ts` → 7 pass;
  `site-page-form-routes` + `site-page-kinds` + `email-sequence-authoring(+routes)`
  + `list-sequence-enrollment` all pass in the consolidated run.
- **Receipt needed for green:** the capture route mounted in `index.ts` with
  this resolver; a wired email send-service with a deliverability smoke
  (send→deliver evidence) and bounce/complaint handling; a customer authoring UI.
- **Owner-gated:** partly — mounting + send-service are buildable; send-vendor
  choice and customer self-serve are product decisions + owner sign-off.

### autopilot_sites.custom_tenant_hostnames.v1 — yellow (software-complete, tested)

- **Status:** registration, DNS-token verification, hostname→tenant mapping,
  request-time resolution, and a live Cloudflare custom-hostname client are
  built and tested (`tenant-custom-hostnames`, `cloudflare-custom-hostname-client`
  — 20 pass).
- **Verify:** `npx vitest run src/tenant-custom-hostnames.test.ts
  src/cloudflare-custom-hostname-client.test.ts` → 20 pass.
- **Receipt needed for green:** a live custom-hostname provision against a real
  zone (set `CLOUDFLARE_API_TOKEN` + `CLOUDFLARE_ZONE_ID`, mount the provision
  route) producing automated DNS-verification + SSL issuance evidence, plus
  request routing into tenant-scoped rendering, and a customer claim UI.
- **Owner-gated:** yes (production Cloudflare credentials + live provision +
  flip). This is config + a real provision smoke, not new core code.

### autopilot_sites.partner_payout_ledger.v1 — red (referral payout rail tested)

- **Status:** operator-gated partner-payout ledger + state transitions (#4986)
  and the Sites referral payout rail (#5458) — paid-event eligibility feed plus
  readiness-gated, idempotent approved→dispatched→settled dispatch that invokes
  the MDK/Spark adapter and refuses non-Bitcoin revenue — are built and tested.
- **Verify:** `npx vitest run src/partner-payout-ledger.test.ts
  src/partner-payout-ledger-routes.test.ts src/site-referral-payout-wire.test.ts
  src/site-referral-payout-adapter.test.ts src/site-referral-payout-ledger.test.ts
  src/site-referral-payout-ledger-routes.test.ts` → 47 pass.
- **Receipt needed for green:** a partner-specific attribution policy
  (referral→customer mapping), payout-ledger linkage to a **real dereferenceable
  public settlement receipt for an actually settled partner payout**, and a
  partner-accessible earnings projection/API. The dispatch rail has never
  settled a real payout.
- **Owner-gated:** yes (payout %/caps sign-off, attribution policy decision,
  first real settled payout).

### autopilot.cloud_credits_ui.v1 — yellow (software-complete, tested)

- **Status:** the Foldkit credits panel (balance/status/rate/min-run/cost
  preview, blocked/under/over/exact-cap) is built and tested; presentational and
  caller-data-driven.
- **Verify:** `npx vitest run src/ui/credits-panel.test.ts` (apps/web) → 24 pass.
- **Receipt needed for green:** a purchase flow, spend tracking, cost-accurate
  preview, and settlement receipts behind the billing backend, with the panel
  bound to live credit data.
- **Owner-gated:** yes (billing backend wiring + flip).

### autopilot.builtin_compute_agent.v1 — yellow (green-candidate, owner-gated runtime)

- **Status:** source wired in Autopilot Desktop (Go-online + Agent pane call a
  Bun built-in-agent RPC, check Pylon + hosted compute readiness, managed
  scratch workspace, daily-start cap, bounded cloud session, no user key). Local
  coding-agent execution lanes independently live-proven 2026-06-19
  (`docs/launch/2026-06-19-coding-agent-live-verification.md`).
- **Verify:** `bun test apps/autopilot-desktop/tests/builtin-agent.test.ts` → 2 pass
  (run in its own process per #5026).
- **Receipt needed for green:** a signed/notarized Desktop build containing the
  built-in-agent source, packaged OpenAgents compute credentials/entitlement, a
  metered/bounded compute path, and public evidence of a from-install Go-online
  session doing useful work with no user API key. The published rc.2 installer
  does not contain this source.
- **Owner-gated:** yes (signed recut + metered from-install smoke).

### autopilot.local_apple_fm_tool_chat.v1 — yellow (software-complete + admitted-Mac smoke)

- **Status:** Apple FM backend contract/client/tools, fleet capability, Swift
  foundation bridge, and the Pylon bridge helper are built; admitted-Mac smoke
  evidence exists.
- **Verify:** `bun test apps/pylon/packages/runtime/src/backends/apple-fm/fake-server.test.ts` → 3 pass.
- **Receipt needed for green:** a signed Desktop recut with helper
  launch/supervision and an installer-based local Apple FM session proof on
  admitted Apple Silicon.
- **Owner-gated:** yes (signed recut + flip).

### autopilot.desktop_gui_client.v1 — yellow (green-candidate, owner-gated DMG proof)

- **Status:** Bun/Electrobun shell + Foldkit webview, loopback Pylon pairing,
  session list/decision cards/timeline, full auto-onboarding EPIC (#5441,
  AO-1..AO-6) built and tested incl. the AO-6 headless smoke driving the real
  local node.
- **Verify:** `bun run --cwd apps/autopilot-desktop verify:deploy` (part of
  `check:deploy`) runs `electrobun-config.test.ts` + the full build + the
  diamond.glb asset check.
- **Receipt needed for green:** the owner-gated from-DMG proof on a clean
  external Mac (rendered window from the signed DMG, real presence on
  production `/api/public/pylon-stats`, a claimed+settled Tassadar window with a
  Bitcoin receipt) **plus** the live PDF/preview/ingest/browser runtimes wired
  and observed, cloud-lane sessions, and a decided distribution/pricing path.
- **Owner-gated:** yes (from-DMG clean-Mac proof).

### autopilot.control_center_fanout_marketplace.v1 — yellow (first-live met, self-serve gate)

- **Status:** first-live single-order market fanout proven (#4783, lane-C,
  customerOptIn-gated, validator-accepted, escrow-settled). Policy + bridge
  built and tested (`lane-c-fanout-policy.ts`, `lane-c-fanout-bridge.ts`).
- **Receipt needed for green:** a **customer-initiated self-serve fanout** and
  plugin-marketplace execution beyond the `code_task` work class, with a public
  settlement receipt. The proven flow was operator-staged.
- **Owner-gated:** yes (self-serve scope decision + flip).

### autopilot.agentic_labor_products.v1 — yellow (direction claim)

- **Status:** product-direction claim backed by the agentic-labor + Sites
  surfaces; not every labor/product flow is self-serve or settlement-backed.
- **Receipt needed for green:** a customer-facing labor/product flow mapped end
  to end to order→review→artifact→acceptance→billing→handoff evidence with a
  public settlement receipt.
- **Owner-gated:** yes.

### autopilot.repo_study_packets.v1 / autopilot.external_repo_studying_pilot.v1 — yellow (internal/pilot only)

- **Status:** public refs-only StudyBench dogfood (MVP-14 comparison) and an
  external-repo studying pilot pipeline on a non-OpenAgents fixture are built
  and tested.
- **Verify:** `bun test packages/probe/packages/runtime/tests/external-repo-studying-product.test.ts` → 2 pass.
- **Receipt needed for green:** customer-private validation/holdout discipline,
  privacy review, self-serve admission/upload controls, marketplace package
  policy, usage metering, pricing, payout eligibility, and settlement receipts.
- **Owner-gated:** yes (multiple independent gates; not a single last-mile).

### autopilot.decision_queue.v1 — planned (Coder Cloud Phase 3)

- **Status:** scoped/planned; cross-client exactly-once decision queue tracked
  as #5004, gated behind the Pylon remote bridge transport (#5000). Precursor
  workroom decision/approval and voice command-proposal state exist.
- **Receipt needed for green:** authenticated command APIs with explicit action
  enums, idempotency, owner approval where needed, receipt closeout, and UI
  projection — built behind the remote-bridge transport.
- **Owner-gated:** yes (depends on #5000 transport; larger than last-mile).

### autopilot.cloud_coding_sessions.v1 — red (Phase 1 exit pending)

- **Status:** lane selector (#4998), grant endpoint (#4999), Pylon cloud
  dispatch (#4997), cloud placement + GCE lease (cloud #86-90) landed. Remaining:
  flip `OA_CODEX_GCE_PROVISIONER` off fake with live ADC provisioning and resolve
  the `cloud.gce.*` event-kind round-trip (#5005).
- **Receipt needed for green:** Phase-1 exit proof — a desktop-originated
  `session.spawn{lane:"cloud-gcp"}` running a real repo-edit Codex session on a
  GCE ephemeral VM, streaming `openagents.codex_workroom_event.v1` into the
  desktop timeline, producing a content-addressed artifact + an
  `openagents.resource_usage_receipt.v1`.
- **Owner-gated:** yes (live ADC provisioning + #5005).

### autopilot.historical_claude_code_mechsuit.v1 — withdrawn

- Intentionally withdrawn historical claim. No green path; remains withdrawn.

### autopilot.all_in_one_business_system.v1 — planned

- Aspirational umbrella claim that aggregates many separately-gated promises.
  Stays planned until its constituent workroom/CRM/finance/legal promises go
  green.

## Summary

| Promise | State | This pass | Owner-gated flip |
| --- | --- | --- | --- |
| autopilot.mission_briefing.v1 | yellow | risk+receipt rollups built+tested (blocker closed) | yes (live mission) |
| autopilot_sites.native_email_sequences.v1 | yellow | form-spec registry built+tested | partly |
| autopilot_sites.custom_tenant_hostnames.v1 | yellow | evidence assembled (20 pass) | yes (live provision) |
| autopilot_sites.partner_payout_ledger.v1 | red | evidence assembled (47 pass) | yes (first settled payout) |
| autopilot.cloud_credits_ui.v1 | yellow | evidence assembled (24 pass) | yes (billing backend) |
| autopilot.builtin_compute_agent.v1 | yellow | evidence assembled (2 pass) | yes (signed recut) |
| autopilot.local_apple_fm_tool_chat.v1 | yellow | evidence assembled (3 pass) | yes (signed recut) |
| autopilot.desktop_gui_client.v1 | yellow | evidence assembled (verify:deploy) | yes (from-DMG proof) |
| autopilot.control_center_fanout_marketplace.v1 | yellow | first-live recorded | yes (self-serve) |
| autopilot.agentic_labor_products.v1 | yellow | direction; flow evidence needed | yes |
| autopilot.repo_study_packets.v1 | yellow | internal dogfood only | yes (many gates) |
| autopilot.external_repo_studying_pilot.v1 | yellow | pilot only (2 pass) | yes (many gates) |
| autopilot.decision_queue.v1 | planned | transport-gated (#5000) | yes |
| autopilot.cloud_coding_sessions.v1 | red | Phase-1 exit pending (#5005) | yes |
| autopilot.historical_claude_code_mechsuit.v1 | withdrawn | n/a | n/a |
| autopilot.all_in_one_business_system.v1 | planned | umbrella | yes |

No state was flipped. Two promises had real last-mile code built this pass
(`autopilot.mission_briefing.v1` cost/risk/receipt rollups;
`autopilot_sites.native_email_sequences.v1` form-spec registry); the remainder
have their green-readiness evidence assembled and the exact remaining receipt and
owner gate recorded above.
