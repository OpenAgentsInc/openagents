# NEEDS_OWNER

## Khala Mobile P0.8 Launch Readiness

Source issue: OpenAgentsInc/openagents#8543
Receipt:
`docs/khala-code/receipts/2026-07-07-qam-8-launch-readiness.md`

Khala Mobile has a signed-in iOS simulator thread-smoke receipt and Android
launch/sign-in handoff evidence, but #8543 requires a stricter launch account
and full straight-line E2E on both platforms. This is owner-gated because the
seed account must have real GitHub authorization, real credit-grant visibility,
and writeback scope without committing credentials or private repo data.

Owner actions required:

- Create or approve a public-safe GitHub test account for Khala Mobile launch readiness.
- Grant only the repo scopes needed for the smoke repo and writeback proof.
- Seed a visible $10 launch credit grant and record the public-safe grant receipt ref.
- Run the full straight-line E2E on iOS simulator and Android emulator.
- Review the launch promises/copy pass only after both platform E2E receipts exist.

Exact operator path after owner approval:

1. Store credentials only in `~/work/.secrets/khala-maestro.env`.
2. Boot the owned iOS simulator and install a Release build of `com.openagents.khala.mobile`.
3. Run `clients/khala-mobile/scripts/signed-in-thread-smoke-run.sh` as the baseline smoke.
4. Extend/run the launch E2E flow for sign in -> $10 grant visible -> pick repo -> dispatch turn -> live updates -> push/writeback link -> credits drain.
5. Repeat the same full launch E2E on Android through `bun run --cwd clients/khala-mobile qa:android:emulator` once the Android lane is green.
6. Record public-safe receipt refs only: platform, build, flow name, verdict, seed account ref, grant receipt ref, writeback URL/ref, and credit-drain assertion. Do not record tokens, raw chat bodies, private repo contents, raw sync rows, local machine identifiers, or secret paths beyond the gitignored env-file location above.
7. Only then review launch promises/copy and flip any promise or contract state with the real receipts attached.

Until those steps complete, #8543 remains `INCONCLUSIVE`: no launch copy or
promise may claim the full mobile straight-line E2E is proven on both platforms.

## Khala Mobile P0.9 Store Submissions

Source issue: OpenAgentsInc/openagents#8544
Receipt:
`docs/khala-code/receipts/2026-07-07-qam-9-store-submissions.md`

P0 is not satisfied until both store submissions are actually in review and the
submission IDs plus review states are recorded as evidence. Agents may prepare
packs and receipts, but App Store Connect and Play Console submissions are
owner-console actions.

Owner actions required:

- Create or confirm the App Store Connect app record for com.openagents.khala.mobile.
- Upload the final locally built iOS archive through Apple Transporter or Xcode Organizer.
- Enter current App Store metadata, screenshots, privacy answers, age rating, and review notes.
- Submit the iOS build for review and record the App Store Connect submission ID and review state.
- Create or confirm the Play Console app record for com.openagents.khala.mobile.
- Upload the final locally signed Android App Bundle to the intended Play track.
- Enter current Play listing, data-safety, content-rating, tester/release notes, and review answers.
- Submit the Play release and record the Play Console release/submission ID and review state.

Exact owner-console URLs:

- App Store Connect apps: https://appstoreconnect.apple.com/apps
- Play Console: https://play.google.com/console

Pre-submission evidence that must be attached or rechecked:

1. P0.8 launch readiness: `docs/khala-code/receipts/2026-07-07-qam-8-launch-readiness.md`.
2. App Store submission pack: `docs/khala-mobile/2026-07-06-app-store-submission-pack.md`.
3. Android build/upload runbook: `docs/khala-mobile/2026-07-06-android-build-and-upload-runbook.md`.
4. Account deletion and 3.1.1/IAP posture must be rechecked against the exact build submitted.
5. Public-safe screenshots must be captured from the approved seed account, not mock data.

Until both submissions are actually in review, record the P0.9 verdict as
`not_submitted`; do not claim store submission, TestFlight external review,
Play internal review, production review, or P0 exit completion.

## QA Swarm Rate Card Draft

Source issue: OpenAgentsInc/openagents#8061

Resolved for public `/business` package bands by OpenAgentsInc/openagents#8079.
The prices may be quoted only with the operator-assisted caveats and receipt
plans from the page; checkout, self-serve hosted runs, first paid delivery
receipts, and broad hosted availability remain gated.

Modeled package bands from the QA Swarm product plan:

| Package          | Draft modeled band            | Notes before publication                                                                                              |
| ---------------- | ----------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Swarm Audit      | $1,000-$5,000 fixed           | Needs owner-approved scope limits, target-adapter caveats, redaction policy, and deliverable template.                |
| QA-on-every-push | $2,000-$10,000/month retainer | Needs hosted-run receipts, exact accounting, runner-capacity policy, and clear operator-assisted vs. self-serve copy. |
| Swarm Sprint     | $5,000-$15,000                | Needs backlog-size bounds, delivery acceptance criteria, and paid-delivery receipt path.                              |

Remaining owner-gated work: decide when QA Swarm hosted-run receipts,
checkout/intake receipts, and outward-facing third-party report artifacts may be
treated as live delivery evidence.

## Reactor Rate Card and Pilot Arming

Source issue: OpenAgentsInc/openagents#8271
Source plan:
`docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md`

Reactor is a planned private/customer-controlled open-model deployment lane, not
an available product or public price sheet. The registry records
`reactor.private_deployment.v1`, `reactor.model_provenance.v1`, and
`reactor.model_policy.v1` are planned-only boundaries until the owner approves
copy and the policy, refusal, eval, metering, and deployment receipts exist.

Modeled package bands from the Reactor plan:

| Package                    | Draft modeled band       | Notes before publication                                                                                                                  |
| -------------------------- | ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Reactor Assessment         | $2,500-$7,500 fixed      | Needs owner-approved scoping, model-policy workshop output, hardware/spec boundary, and public-safe receipt plan.                         |
| Reactor Pilot              | $10,000-$25,000          | Needs customer-controlled or dogfood node receipt, policy-enforced serving smoke, exact metering, and customer-data boundary proof.       |
| Reactor Managed            | $2,500-$10,000/month     | Needs owner-approved operating responsibilities, upgrade/eval cadence, incident policy, and customer data/corpus custody boundary.        |
| Data Liberation            | $2,500-$10,000           | Needs source-system export authority, verification receipts per record class, and redaction/privacy handling before public package copy.  |
| Harness evolution add-on   | quoted                   | Needs accepted harness-optimization receipt format and clear non-weight-changing scope for each customer task family.                     |
| Fine-tune / flywheel add-on | quoted                  | Needs customer-owned training-data authority, model-artifact ownership terms, eval receipts, and no compliance/custody overclaim.         |

NEEDS-OWNER: Decide whether these bands may appear in public copy, approve the
first Reactor assessment/pilot offer language, choose the policy/eval receipt
format, choose whether the first proof must be dogfood or customer-controlled,
and explicitly approve any compliance, sovereignty, data-custody, or
US-origin-only wording before it appears outside owner-gated notes.

Until those owner steps are complete, the shipped state remains: Reactor is a
draft lane with planned registry records only; no customer install, public
price, policy-enforced serving proof, air-gapped update proof, compliance claim,
or customer-data-custody proof exists.

## Khala Code Desktop Signed Release Gate

Source issue: OpenAgentsInc/openagents#8245
Public install-truth surface: OpenAgentsInc/openagents#8246
Outside-user evidence intake: OpenAgentsInc/openagents#8247

Khala Code Desktop has a buildable Electrobun app, but RL-1 is not complete
until the release owner signs, notarizes, staples, uploads, and smoke-tests a
real macOS DMG. The repo may document and preflight the path, but the signing
identity, notary credentials, update-feed write, and clean-Mac first-run proof
are owner-gated.

Current state:

- `clients/khala-code-desktop/electrobun.config.ts` names the app
  `Khala Code` with bundle id `com.openagents.khala.code.desktop`.
- `bun run --cwd clients/khala-code-desktop build:rc` and
  `bun run --cwd clients/khala-code-desktop build:stable` are the unsigned
  channel build paths.
- `bun run --cwd clients/khala-code-desktop release:plan -- --version <version> --channel rc --artifact ./Khala-Code-<version>.dmg`
  validates the product tag, RC/stable/latest rules, GitHub tag convention, and
  Khala-specific updates-feed path before signing work starts.
- `bun run --cwd clients/khala-code-desktop release:macos -- --version <version> --channel rc`
  is the owner-run macOS lane. It reuses
  `apps/autopilot-desktop/scripts/notarize-macos.sh` with `OA_DESKTOP_APP_PATH`
  pointed at the Khala app, re-creates the DMG from the stapled `.app`,
  signs/notarizes/staples the DMG, stages
  `desktop/khala-code-desktop/<channel>/feed.json`, and refuses to upload or
  create a GitHub release unless the owner sets the explicit release env flags.
- No signed/notarized/stapled Khala Code DMG, owner-approved updates-feed path,
  GitHub release, or clean-Mac first-run smoke receipt is recorded here yet.
- `/code/download` and `GET /api/public/khala-code/download-counts` exist for
  public install truth, but they are not release receipts. The page must keep
  the desktop DMG marked as a pending public artifact until the receipt set
  below exists, and the counter may report only exact
  `khala_code_download_events` rows or an empty response with blocker refs.
- `POST /api/public/khala-code/outside-user-runs` and
  `GET /api/public/khala-code/outside-user-runs/{receiptRef}` now exist for
  opt-in outside-user run receipts. They are evidence intake only: no
  background phone-home, no paths/prompts/tokens/logs/account ids/machine ids,
  and no promise-state movement until a real receipt is reviewed.

NEEDS-OWNER: Provide or confirm the Developer ID/notary environment on an
owner-controlled machine, approve the Khala Code version and release channel,
approve the update-feed destination, run or authorize the signing/notarization
flow, re-create/sign/notarize/staple the DMG from the stapled `.app`, upload the
artifact, recruit or approve at least one outside-user run receipt, and record
public-safe receipt refs for the clean-Mac smoke. The smoke must prove the app
boots from the DMG and, when Codex is missing or unauthenticated, shows the
honest `npm install -g @openai/codex` / `codex login` path without claiming
Khala Code bundles or replaces Codex.

Receipt refs required before RL-1 may be called complete:

- signed app
- notarized app
- stapled app
- recreated DMG from the stapled app
- signed DMG
- notarized DMG
- stapled DMG
- updates-feed upload
- GitHub release
- clean-Mac first-run smoke showing the missing-Codex install/login hint

## Khala Code Paid Plan Payment Arming

Source issue: OpenAgentsInc/openagents#8248

The paid-plan payment-collection leg now exists in source but remains
owner-armed and fail-closed by default. `POST /v1/khala-code/plans/purchases`
does not grant the paid-privacy entitlement until payment is settled: card
purchases create a Stripe Checkout Session, and crypto purchases create a
Spark/MPP Lightning invoice whose preimage is verified locally before the
existing paid-privacy entitlement receipt is granted.

NEEDS-OWNER: Decide when to set `KHALA_CODE_PAID_PLANS_ENABLED`, choose and
configure `KHALA_CODE_PAID_PLAN_STRIPE_PRICE_ID`, choose
`KHALA_CODE_PAID_PLAN_PRICE_SATS`, confirm the production Stripe webhook secret
and Spark/MPP Lightning invoice issuer are live, approve the card/LN public
copy, and record one public-safe production collected-purchase receipt before
any promise-green or “paid plan is live” claim.

Until those owner steps are complete, the shipped state remains: flag off by
default, paid plan not purchasable, no entitlement granted without a settled
payment receipt.

## Khala Code Desktop Trace Capture Arming

Source issue: OpenAgentsInc/openagents#8250

Khala Code Desktop now has an explicit default-off free-plan trace-capture
consent control, local persisted consent RPCs, and a fail-closed capture
planner. The planner admits only free-plan session events with explicit consent,
owner arming, Rampart redaction success, and an owner_only ingest sink. Paid-plan
capture opt-out, missing owner arming, missing ingest, or redaction failure all
return `not_captured`, and the marker remains payout/settlement inert.

NEEDS-OWNER: Decide when to set `KHALA_CODE_DESKTOP_TRACE_CAPTURE_ENABLED=1`,
approve the owner_only trace-ingest destination and retention/deletion policy,
approve the final desktop disclosure/copy beside
`data.free_tier_capture_disclosure.v1`, confirm the paid-plan exclusion policy
for desktop sessions, and record one public-safe production receipt proving a
consented free-plan event was redacted before owner_only ingest. Do not move
`khala_code.free_plan_trace_capture.v1` out of planned/yellow or claim desktop
capture is live until that receipt exists.

Until those owner steps are complete, the shipped state remains: consent UI and
local fail-closed planner exist; production capture is not armed, no owner_only
ingest sink is live, no user trace is captured by this desktop path, and capture
grants no payout or settlement eligibility.

## Khala Code Trace Plugin Revenue-Share Precedent

Source issue: OpenAgentsInc/openagents#8251

The RL-7 precedent receipt spine now exists in source. The admin-token route
`POST /api/operator/khala-code/trace-plugin-revenue-share-precedents` records
only public-safe refs for one consented trace digest, admitted+registered
routable plugin, exact routed usage/idempotency, contributor attribution, msat
accounting, Spark payout receipt, and settlement receipt. The public readback
`GET /api/public/khala-code/trace-plugin-revenue-share-precedents/{receiptRef}`
is dereferenceable evidence. The route does not move sats, dispatch payout,
accept payout destinations, publish traces, define a rate, or flip promise
state.

NEEDS-OWNER: Produce and approve the actual n=1 live precedent before any public
copy claims someone was paid by plugin routing: choose the consented trace and
redacted digest, approve the admitted plugin and registry refs, route one real
request through it with exact usage/idempotency evidence, approve the
contributor attribution and amount envelope, run or authorize the Spark payout
through the existing money-moving authority, verify the public-safe payout and
settlement refs, and then post the precedent intake body using the template in
`docs/khala-code/2026-07-04-trace-plugin-revenue-share-precedent-template.md`.

Until those owner steps are complete, the shipped state remains: the ledger,
route, public receipt shape, and docs exist; there is no production receipt row,
no live paid contributor, no rate/pool policy, no market-demand proof, and
`khala_code.trace_derived_plugins.v1` /
`khala_code.plugin_backend_revenue_share.v1` remain planned.

## QS7 Rhys Sales Motion Owner Gate

Source issue: OpenAgentsInc/openagents#8067

This is an owner-review staging note for the outward-facing Swarm Audit demo PR
against `RhysSullivan/executor`. The external PR must not be opened, commented
on, or otherwise sent publicly until the owner signs off on the exact artifact
packet.

Current state:

- The former product-surface prerequisites have landed: QS2 provides the
  run-level share URL, QS8 provides chill-evals variant comparison, and QS9
  provides the third-party target-adapter contract.
- A live read-only browser audit against the executor public landing page was
  captured under run token `qs7-executor-20260703T151831Z`.
- The packet now has fingerprinted browser media, terminal asciicast, terminal
  snapshots, result verdict, committed e2e test, share projection, and
  chill-evals comparison refs in
  `docs/fable/2026-07-02-qs7-rhys-sales-motion-owner-review.md`.
- The observed browser verdict is `CONFIRMED`; the chill-evals comparison has a
  passing baseline and an intentionally false candidate that fails honestly.
- The share URL staged for review is
  `https://openagents.com/qa/qa-run.executor.qs7-public-home.20260703`.
- No external repository PR has been opened.

NEEDS-OWNER: Review
`docs/fable/2026-07-02-qs7-rhys-sales-motion-owner-review.md`, approve or edit
the public-safe artifact packet, choose the public attachment/media location,
and explicitly authorize the outward-facing `RhysSullivan/executor` PR.

## QA Swarm Hosted-Run Engagement Arming

Source issue: OpenAgentsInc/openagents#8070

The QS10 contract now models hosted-run receipts, exact-only metering rows, and
engagement receipts through the existing business quick-win lifecycle. The
settlement/payment seam is intentionally inert unless the owner explicitly arms
it.

NEEDS-OWNER: Decide when QA Swarm hosted-run engagement receipts may evidence
`buyer_paid` / `provider_settled` refs, which checkout/payment source is
approved for those refs, and what operator review is required before any paid
QA Swarm engagement is represented as paid or settled.

## QA Swarm First Paid Delivery Receipt

Source issue: OpenAgentsInc/openagents#8252

The RL-8 first-engagement spine now exists for operator-assisted Swarm Audit
commitments:

- `POST /api/operator/qa-swarm/first-engagements`
- `GET /api/public/qa-swarm/first-engagements/{receiptRef}`

The route records only public-safe intake plus checkout/deposit receipt refs,
provisions the workspace and active service promise, and creates the
business-commitment-ledger row. It intentionally leaves first paid delivery,
self-serve hosted delivery, payout, and settlement unclaimed.

NEEDS-OWNER: For the first real QA Swarm sales outcome, review the production
intake/payment evidence, approve the target-adapter/redaction boundary, decide
which public-safe delivery receipt may count as the first paid delivery, and
record that receipt before any copy claims paid QA Swarm delivery exists.

## First-Dollar Revenue Claim Sign-Off

Source issue: OpenAgentsInc/openagents#8253

The RL-9 revenue-event provenance ledger now records Khala Code paid-plan and
QA Swarm first-engagement revenue events with an internal/external label and a
public-safe first-dollar evidence bundle:

- `GET /api/public/revenue-loop/first-dollar-evidence/{bundleRef}`

The bundle joins the receipt ref, source ledger row, and provenance label for
registry evidence. It does not itself prove settlement, paid delivery,
self-serve availability, market demand, or permission to publish external
revenue copy.

NEEDS-OWNER: Before any product-promise state movement, public external-demand
claim, paid-plan availability claim, or "first paid delivery" copy cites this
route, review a concrete production bundle and the backing payment/delivery
evidence, confirm the label is truly external, and explicitly approve the
public copy boundary.

## Aiur Admin Panel — Owner Allowlist and DNS — RESOLVED 2026-07-06

Source issues: OpenAgentsInc/openagents#8499 (epic #8467, Khala Code
mobile-only MVP) and #8526 (CFG-11, Cloudflare → Google consolidation
epic #8515).

Aiur (`apps/aiur/`) is the owner-only admin panel for manually granting
mobile-app credits (replacing IAP for the first MVP build) and viewing
ops/health data. It is now served entirely from **Google Cloud Run**
(service `openagents-aiur`, project `openagentsgemini`, region
`us-central1`), gated by a hard, fail-closed allowlist
(`AIUR_OWNER_USER_IDS`) checked on every request — unset or empty denies
everyone, including a legitimately signed-in GitHub user. See
`docs/khala-code/2026-07-06-aiur-admin-deploy-runbook.md` for the full
deploy/verify steps.

STATUS — no owner action remains. The full CFG-11 cutover is done:

1. Owner allowlist is set. Secret Manager secret `aiur-owner-user-ids`
   (`openagentsgemini`) = `github:14167547`, mounted into the Cloud Run
   service as `AIUR_OWNER_USER_IDS` via `secretKeyRef`. Fail-closed is
   verified: an unauthenticated request to
   `https://aiur.openagents.com/api/aiur/access` returns 200
   `{"kind":"signed_out"}` and no dashboard data loads.
2. DNS cutover is done. `aiur.openagents.com` resolves via a DNS-only
   (grey-cloud) CNAME → `ghs.googlehosted.com.` in the Cloudflare
   `openagents.com` zone, and the Cloud Run domain mapping cert is
   provisioned (`Ready`/`CertificateProvisioned` = True). The live
   hostname serves from Cloud Run (`server: Google Frontend`,
   `x-cloud-trace-context` present, no `cf-ray`).
3. The legacy Cloudflare Worker `openagents-aiur` (account
   `arcadecd@gmail.com`) has been deleted, freeing the hostname and
   removing the stale custom-domain binding. The live hostname was
   re-verified serving 200 from Cloud Run AFTER the delete.

Optional owner smoke (not a blocker): sign in at
`https://aiur.openagents.com/` with GitHub and confirm you land signed in
(not denied) and the "Khala Tokens Served" panel shows a live number —
this confirms `github:14167547` is your verified OpenAuth user id. If you
are instead denied, the allowlist value needs to be updated to your actual
verified OpenAuth `userId` (update the `aiur-owner-user-ids` secret and the
Cloud Run service picks up `latest` on the next revision).
