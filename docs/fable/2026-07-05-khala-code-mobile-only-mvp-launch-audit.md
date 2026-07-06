# Khala Code Mobile-Only MVP — Launch Audit

Date: 2026-07-05
Status: owner-directed pivot audit. This document records the owner decision,
maps everything that exists against everything the mobile-only launch needs,
defines the launch workstreams (filed as GitHub issues — see §9), records the
open-issue triage with a reopen ledger (§8), and drives the product-promise
registry update landing in the same change (§10). Grounded in five parallel
code explorations run 2026-07-05 (auth/OpenAuth, credits/billing/IAP, cloud
inference + server-side execution, mobile desktop-coupling, promises
registry); every "exists"/"missing" claim below cites the file it was
verified against.

## 0. The owner decision (2026-07-05, recorded verbatim in essence)

For the Khala Code MVP launch we go **MOBILE-ONLY**. The current flow's
dependency on the user's desktop Pylon is **postponed** — the app must work
entirely and only on mobile:

- User logs in with **GitHub** using our OpenAuth auth server.
- They **pick a repo**, ask the agent to do stuff, and watch updates.
- **Everything uses credits.** Every new GitHub account gets **$10 free
  credit** ($10 per GitHub account). Credits can be earned (e.g. social
  posts) or bought via **in-app purchases** (RevenueCat or similar).
- Users can **configure what models they use**. Execution runs on
  **OpenAgents Cloud** (Gemini or our coding-agent pool) — never the user's
  own hardware.
- **Push notifications**, **Android and iOS**, **take people's money**.
- We are **SIMPLIFYING**: a straight line to cool usage — connect GitHub, do
  cool shit on mobile, pay for it.

This supersedes, for the MVP launch scope: the desktop-pairing mobile
companion model (`mobile.fleet_companion.v1`'s pairing/relay framing), the
Tailnet auto-auth mandate of 2026-07-04 (the enforced behavior contract
`khala_mobile.auth.tailnet_auto_discovery_before_manual_login.v1` is to be
retired/replaced when the GitHub login lands — this document is the recorded
owner sign-off for that contract change), and the desktop-fleet-first
sequencing in `ROADMAP.md`. The desktop app, Pylon fleet, and cross-device
sync work are **postponed, not deleted** — everything shipped keeps working
and the reopen ledger in §8 says what comes back later.

## 1. One-paragraph verdict

This pivot is more buildable than it looks. The **data plane is already
mobile-cloud-clean**: the Expo app talks only to
`https://openagents.com/api/sync/*` (bootstrap/connect/push), stores chat
locally in SQLite, and sends coding turns as typed
`khala_runtime_control_intent.v1` mutations — nothing in the wire contract
mentions a desktop. **GitHub login already exists server-side** (OpenAuth
issuer at `auth.openagents.com` with a `GithubProvider`, `repo` scope, and
per-user GitHub access tokens already persisted in KV with a repo-listing
service on top). **Cloud inference is live** (the `/khala` chat and the
OpenAI-compatible gateway run fully in the Worker across Fireworks, the
org-owned Hydralisk GPU lane, OpenRouter, Vertex Claude, and Vertex Gemini).
**Credits machinery is rich** (atomic msat ledger, USD→msat bridge, Stripe,
metering hook + pricing engine, an existing $10 trial-grant pattern, a
windowed/capped grant pattern to copy). The four genuinely missing pillars
are: (1) a **native-app auth flow** (PKCE/mobile session tokens — today's
session is cookie-only and mobile's token is literally pulled out of a
signed-in desktop over Tailnet), (2) an **org-owned cloud executor** for
coding turns (today both lanes are executed by the *user's own* Pylon, and
the owner-self dispatch boundary forbids pooled capacity **by design** — the
relaxation is a policy decision this pivot now makes for the mobile lane),
(3) **IAP** (zero StoreKit/Play/RevenueCat code anywhere), and (4) **push
notifications** (zero APNs/FCM/Expo-push code anywhere, client or server).

## 2. What exists and carries over (verified, with paths)

### 2.1 Auth: GitHub via OpenAuth — server-side yes, mobile no

- The OpenAuth issuer is embedded in the main API worker
  (`apps/openagents.com/workers/api/src/index.ts`, `makeAuthIssuer` ~3638),
  served at `auth.openagents.com`, with exactly two providers: GitHub
  (`GithubProvider`) and email one-time code.
- **Login scopes already include `repo`**: `GITHUB_LOGIN_SCOPES =
  ['read:user', 'user:email', 'repo']` (index.ts ~2110).
- On GitHub login success the worker fetches the GitHub user + emails,
  upserts our user, and **stores the GitHub access token in KV**
  (`github-identity:token:<userId>`, 400-day TTL — `onboarding/github.ts`).
- `GitHubRepositoryService` (`onboarding/github.ts`, routes in
  `onboarding/routes.ts`) already lists the user's repos
  (owner/collaborator/org-member) using that stored token — this is the
  "pick a repo" capability, already built, just cookie-gated.
- A separate GitHub **write** connection flow exists
  (`github-write-connections.ts`, scopes `repo`+`workflow`, tokens in KV,
  grants in D1) — the writeback seam for branches/PRs.
- Original audit finding: web sessions were **HttpOnly cookies only**
  (`auth-cookies.ts`, `oa_access`/`oa_refresh`); there was **no PKCE public
  client, no OAuth device grant, and no user bearer-token session** a native app
  could use. The existing device-pairing flows
  (`pylon-openagents-auth-routes.ts`, `khala-code-openagents-auth-routes.ts`)
  mint `oa_agent_` tokens and require a signed-in browser to approve —
  desktop/CLI patterns, not a self-contained mobile login.
- **2026-07-05 #8468 update:** the issuer now admits the public mobile client
  (`OPENAUTH_MOBILE_CLIENT_ID`, default `openagents-khala-mobile`) only for
  GitHub authorization-code + PKCE S256 requests redirecting to `khala://auth`.
  Native clients can exchange the code, refresh tokens, verify a cookie-free
  user bearer session at `GET /api/mobile/auth/session`, and sign out with
  server-side access-token revocation plus optional refresh-token removal via
  `DELETE /api/mobile/auth/session`. Remaining WS-A gaps are #8469
  (session→Khala Sync token issuance) and #8470 (mobile UI + Tailnet contract
  retirement).
- **2026-07-05 #8469 update:** `POST /api/mobile/session` now verifies the
  mobile OpenAuth bearer and returns the mobile app's existing credential shape
  `{ ownerUserId, syncToken }`. The `syncToken` is the current OpenAuth mobile
  access token, which `/api/sync/*` already accepts through the human actor path
  before enforcing the normal Khala Sync scope taxonomy (`scope.user.<owner>`,
  owned `scope.thread.*`, and existing non-mobile rules). Refresh rotation is
  the OpenAuth access-token refresh path; sign-out revokes the same access token
  and optional refresh token through #8468's revocation endpoint. Remaining
  WS-A gap is #8470 (mobile UI + Tailnet contract retirement).
- **2026-07-05 #8470 update:** `clients/khala-mobile` now has the native
  GitHub PKCE sign-in path through Expo AuthSession/WebBrowser. Fresh installs
  no longer run Tailnet auto-discovery before login; signed-out users see one
  primary action, `Sign in with GitHub`. The app exchanges the OpenAuth access
  token for the existing `{ ownerUserId, token }` SecureStore credential shape
  via `POST /api/mobile/session`, then validates the returned bearer against
  Khala Sync before entering the signed-in app. The Tailnet pairing code remains
  only as diagnostic/reference code with its own unit tests; the behavior
  contract `khala_mobile.auth.tailnet_auto_discovery_before_manual_login.v1` is
  retired and replaced by `khala_mobile.auth.github_sign_in_primary_action.v1`.
  Verification covered full mobile tests, mobile typecheck, architecture guard,
  iOS local prebuild/build (`** BUILD SUCCEEDED **`), Android local
  prebuild/build (`BUILD SUCCESSFUL` with Homebrew OpenJDK 17 and the installed
  Android command-line SDK), repo `check:deploy`, and an iPhone 17 simulator
  fresh-install smoke showing the GitHub-only signed-out screen with no Tailnet
  or manual-token controls. The native-module runtime bump was followed by a
  signed iOS OTA baseline publish from pushed `main`: runtime
  `d72044f835d38b35da4a3559784593b45fce2ad8`, Cloud Run revision
  `oa-updates-00054-w5t`, and public manifest verification returned HTTP 200
  multipart with an `expo-signature` manifest part, the matching runtime, 20
  assets, and the `Khala Code` public Expo config.
- There is **no GitHub App** (no installation tokens, no fine-grained
  per-repo permissions) — all repo access rides the user OAuth token.

### 2.2 Mobile app: data plane cloud-clean, three desktop couplings

- `clients/khala-mobile` (Expo RN, `com.openagents.khala.mobile`, iOS build
  8 / Android versionCode 2 after the AuthSession/WebBrowser runtime bump)
  syncs exclusively against
  `https://openagents.com/api/sync/*` with a bearer token; local-first
  SQLite durable store + optimistic overlay + durable cursors
  (`src/sync/khala-mobile-sync-runtime.ts`). Scopes: `scope.user.<id>`
  (threads), `scope.thread.<id>` (messages/turns/events),
  `scope.fleet_run.<id>` (settings fleet view).
- Coding turns are already **cloud-shaped**: the composer pushes
  `chat.appendMessage` + a `runtime.startTurn|appendUserMessage|interruptTurn`
  control intent (`openagents.khala_runtime_control_intent.v1`) with a
  target lane (`codex_app_server` | `claude_pylon`). **The mobile wire
  contract does not need to change for cloud execution** — only who
  consumes the intent does.
- The remaining desktop couplings after #8470:
  1. **Auth retired for MVP default path**: Tailnet auto-discovery and manual
     token paste no longer gate fresh installs. The old pairing endpoint/core
     remains as diagnostic/reference code, but the signed-out app defaults to
     GitHub PKCE + `/api/mobile/session`.
  2. **Execution**: both lanes are consumed by
     `apps/pylon/src/orchestration/runtime-intent-enforcement.ts` running
     on the *user's* machine.
  3. **Settings → Fleet**: env-var fleet-run id, desktop-oriented copy.
- **No push notifications** (no expo-notifications, no token registration),
  **no IAP code** — all confirmed absent by grep in the original audit. GitHub
  OpenAuth code landed in #8470.
- Distribution is proven: TestFlight uploads confirmed VALID via the ASC
  API; local `expo prebuild` + Xcode/Gradle builds; the owned OTA server
  (`apps/oa-updates`, `updates.openagents.com`) has a **proven end-to-end
  signed OTA round trip** (runbook:
  `docs/khala-code/2026-07-05-mobile-ota-updates-runbook.md`). Android has
  a green Gradle assemble but **zero device/emulator boot evidence** and no
  Play Store lane yet.

### 2.3 Cloud inference: live today, no pylon needed

- `/api/khala/chat` (public, streaming) and the OpenAI-compatible
  `/v1/chat/completions` + `/v1/models` run fully server-side
  (`src/khala-chat-routes.ts`, `src/inference/*`).
- Model lanes wired: `fireworks`, `hydralisk` (org-owned GPU, GLM-5.2 REAP
  — default Khala backing), `openrouter` (fallback), `vertex-anthropic`
  (Claude), **`vertex-gemini` (Gemini)**, `openagents-network`. Operator
  knob `KHALA_BACKING_MODEL`; catalog in `src/inference/model-catalog.ts`.
- A **Hosted Gemini executor** exists as a flag-gated Autopilot tool-loop
  (`autopilot-hosted-gemini-executor-env.ts`) — the closest existing thing
  to a server-side agent runner, but not a coding runner.

### 2.4 Coding execution: the deliberate owner-self wall, and the seams through it

- Dispatch resolution (`src/inference/coding-workflow-delegation.ts`)
  requires an owner-linked, heartbeat-fresh (≤5 min), capability-advertising,
  capacity-available Pylon. No org-pool fallback exists —
  `artanis-owner-authority.ts`: "never pooled/third-party/marketplace
  capacity." **This is an enforced invariant, not an accident.** The pivot
  makes the policy decision to add a credit-gated org-cloud lane for mobile
  users; the invariant for *other users' pylons* stays.
- Existing seams to build the cloud executor on:
  - `src/cloud/cloud-coding-session-routes.ts` — a typed, flag-gated
    (default-off, fails-closed) "our cloud" coding-session surface with
    trust-tier placement, adapter choice (`codex` | `claude_agent`), and a
    metering hook emitting `openagents.resource_usage_receipt.v1`. Points
    at the private `oa-codex-control` control plane (`OA_CLOUD_CONTROL_URL`)
    whose Firecracker microVM provisioner lives in the private `cloud/`
    repo.
  - `src/runner-gateway.ts` + `runner-backends.ts` — the multi-backend
    runner substrate (`shc_vm` | `gcloud_vm` | `cloudflare_container`)
    already consumed by Omni runs, which clone from `github.com`.
  - `apps/pylon/src/cloud-control-client.ts` — the Pylon→cloud offload
    client with GCE lease lifecycle events, tested against a fake control
    plane.
  - GCE VM setup for Pylon is proven (`apps/pylon/deploy/gcloud/`) and is
    retained as historical/operator infrastructure. The mobile MVP substrate
    is now the Agent Computer path: nested-virt GCE hosts plus the private
    `cloud/` Firecracker provisioner, with the Pylon runtime only as
    software inside the microVM image.
- **2026-07-06 #8473 update:** the executor spine now exists in
  `apps/pylon`: `runtime-intent-supervisor.ts`, opt-in exact usage receipts,
  and an opt-in `hosted_khala` lane backed by the OpenAgents gateway/Vertex
  Gemini default (`gemini-3.5-flash`). Under the owner-decided Agent Computer
  strategy, those lanes become runtime behavior inside the microVM image rather
  than a shared OS pool. The Worker route
  `POST /api/khala/cloud/runtime-turn-usage` writes exact external
  `token_usage_events` receipts with provider attribution for
  `pylon-codex-org-capacity`, `pylon-claude-org-capacity`, or
  `vertex-gemini`. Mobile sync entities and the mobile wire contract remain
  unchanged. Ops details live in
  `docs/khala-mobile/2026-07-06-org-cloud-executor-runbook.md`.
- **2026-07-06 substrate decision (supersedes the same-day exe.dev
  recommendation):** the cloud execution substrate is **Agent Computers** —
  Firecracker microVMs on OpenAgents' own GCP infrastructure, separately
  billable against credits — per the owner-decided strategy in
  `docs/khala-code/2026-07-06-agent-computers-strategy.md` (issue #8503).
  The exe.dev evaluation
  (`docs/khala-code/2026-07-06-exe-dev-cloud-delegation-audit.md`) is
  retained as history with a superseded banner; its authority-model
  conclusions carry over. "Hosted Pylon" is retired from planning and
  product language — the Pylon runtime remains the internal executor
  inside the agent-computer image, but the provisioned/metered/billed unit
  is the agent computer.
- Repo access: cloud checkout today is **public pinned SHA only**
  (`apps/pylon/src/workspace-materializer.ts` rejects private repos). The
  SCM auth-broker seam exists (`openagents.pylon.scm_auth_broker.v1`,
  credential-helper pattern, fail-closed, on-disk credential scanner) but
  currently brokers **Forge** repos, not github.com. Wiring the stored user
  GitHub OAuth token through that broker + relaxing the visibility contract
  is the private-repo path.

### 2.5 Credits and payments: three pools, strong primitives, wrong keying

- **Pool A** — USD billing ledger (`billing.ts`, `billing_ledger_entries`):
  includes an existing **$10 trial grant** (`INITIAL_TRIAL_CREDIT_CENTS =
  1000`, idempotency `billing:trial:${userId}`) — lazily granted, Autopilot-
  oriented.
- **Pool B** — msat inference ledger (`payments-ledger.ts`,
  `agent_balances` with `CHECK (balance_msat >= 0)`, idempotent PayIns):
  what the inference gateway actually charges via
  `src/inference/metering-hook.ts` + the table-driven pricing engine
  (`pricing.ts`, 1 credit = $0.01, 40% default margin). The **USD→msat
  bridge** (`usd-credit-bridge.ts`) grants card-origin credit with the
  RL-3 asset boundary (USD-origin msat is inference-spendable, never
  Bitcoin-withdrawable).
- **Pool C** — free-allowance pools (`inference-free-allowance.ts`,
  $10 verified-owner cap keyed to X owner-claims, per-IP-hash mint caps).
- Payment rails live/wired: **Stripe Checkout** (mature), Lightning/Spark
  MPP (flag-gated), and the Khala Code paid-plan payment-intent pattern
  (`khala-code-paid-plan-payments.ts`, rails CHECK-constrained to
  `stripe_checkout|lightning_mpp` — the exact pattern to extend with an IAP
  rail).
- Grant pattern to copy: `business-starter-credit.ts` (windowed, capped by
  SQL trigger, idempotent, receipted).
- **Zero IAP**: no RevenueCat/StoreKit/Play Billing code anywhere
  (confirmed). No App Store server-notification verification, no SKU
  catalog.
- Anti-abuse today is thin for a self-serve grant: one-per-user idempotency
  and IP-hash mint caps; no device attestation (DeviceCheck/Play Integrity).
- Usage truth: `token_usage_events` is the canonical exact usage record for
  both inference and Codex/Claude coding turns — the metering seam for
  "everything uses credits" already exists.

### 2.6 Push notifications: nothing, anywhere

Confirmed zero client (`expo-notifications` absent) and zero server
(APNs/FCM/Expo-push absent) push infrastructure. `khala-sync-push-routes.ts`
is data sync, not notifications. Fully greenfield.

## 3. Gap analysis → the launch workstreams

Reconciliation note: the credits exploration reported "no GitHub identity";
the auth exploration verified the GithubProvider IS live in the issuer with
tokens stored — the real gap is **mobile-native flow + grant keying**, not
the provider.

| # | Pillar | What exists | What's missing (→ issues, §9) |
|---|---|---|---|
| A | Mobile GitHub sign-in | Issuer + GithubProvider + repo scope + stored tokens; SecureStore credential shape on mobile | PKCE public client (or device grant) on the issuer; mobile redirect allowlist; **user bearer session** for non-browser clients; session→Khala Sync token issuance; mobile sign-in UI; retire the Tailnet contract |
| B | Repo picker | `GitHubRepositoryService` lists repos server-side | Mobile-bearer-authorized repo endpoints; repo picker UI; thread↔repo binding in sync entities |
| C | Cloud execution | Both lanes fully implemented in `runtime-intent-enforcement.ts` (pylon-local); cloud-session scaffold; runner gateway; historical GCE Pylon hosting; private `cloud/` Firecracker plane; #8473 executor spine + `hosted_khala`/Gemini runner + exact runtime usage receipts; #8503 Agent Computer public seam and nested-virt host docs/tests | Dispatch-policy change (credit-gated Agent Computer lane for mobile); private-repo checkout via user OAuth token through the SCM broker; per-work-context Firecracker isolation enforcement; result writeback (branch/PR) via user token; credit charging from exact receipts |
| D | Credits | Pools A/B/C, metering hook, pricing, USD→msat bridge, grant patterns, `token_usage_events` | $10 grant keyed per GitHub account (idempotent on GitHub user id) landing spendably in Pool B; coding-run metering wired to balance gate; balance UI + insufficient-credit UX; abuse hardening (account-age heuristics, attestation) |
| E | IAP | Payment-intent→fulfillment pattern; asset boundary | Entire IAP rail: RevenueCat (or StoreKit2/Play Billing) client; server receipt validation + webhook; SKU catalog; fulfillment→credits; Apple 3.1.1 compliance (credits consumed in-app are digital goods — **must** use IAP on iOS); restore/refund/clawback |
| F | Model config | Model catalog + lanes incl. Gemini; operator-level backing knob | Per-USER model preference honored by chat + coding executor; mobile settings UI |
| G | Push | Nothing | expo-notifications + device token registry API; server sender (Expo push service first); notify events (turn finished / needs input / credit low) |
| H | Product surface | 3 screens, contracts registry, OTA, TestFlight | Settings rework (drop desktop Fleet section); onboarding straight line (sign in → $10 → pick repo → first task → watch updates); contracts pivot (retire desktop-pairing contracts, add mobile-only ones) |
| I | Launch ops | iOS TestFlight lane proven; Android builds green | Android emulator/device proof + Play internal-testing lane; App Store submission pack (privacy labels, IAP review, review notes); E2E QA (Maestro) for the whole straight line; promises/copy gates |
| J | Growth (post-MVP-gated) | X owner-claim verification pattern | "Post to earn credits" flow reusing tweet verification |

## 4. The straight line (the product definition to build against)

1. Install from App Store / Play (or TestFlight/internal track pre-launch).
2. One button: **Sign in with GitHub** (OpenAuth PKCE, in-app browser).
3. Land signed in with **$10 credit** already granted (visible immediately).
4. **Pick a repo** from your GitHub list (or paste a URL / start repo-less).
5. Type (or speak) what you want done. The turn runs on **OpenAgents
   Cloud** against your chosen model (Gemini default, configurable).
6. Watch live updates in the thread (already-working sync scopes). Leave
   the app; get a **push notification** when it finishes or needs you.
7. Results land as a branch/PR on your repo via your GitHub authorization.
8. Credits drain per exact usage; buy more via **in-app purchase** when
   you run out. That's the whole product.

Explicitly OUT of the MVP: desktop pairing, Tailnet anything, user-owned
pylons, fleet cockpit on mobile, cross-device dogfood, Bitcoin payouts,
plugin revenue share (registry records stay planned), voice beyond the
existing on-device STT gating.

## 5. Key risks and the honest calls

- **Apple IAP policy (3.1.1)**: credits spent on in-app digital services
  must be purchasable via IAP on iOS. External-purchase links are a
  minefield; the MVP takes IAP (RevenueCat abstracts both stores) and
  keeps Stripe for web. Pricing must absorb the 15–30% store cut.
- **Untrusted repo execution**: running agent turns against arbitrary user
  repos on org capacity needs a real isolation boundary. The private
  `cloud/` Firecracker plane is the destination; an interim posture of
  per-run isolated workspaces on a dedicated org GCE pool (no shared
  secrets, scoped tokens, credential scanner enforced) must be explicitly
  documented as the interim trust model.
- **Owner-self dispatch invariant**: relaxing it must be *additive* — a new
  org-cloud lane with its own admission gate (credit balance + mobile
  session), never a widening of access to other users' pylons.
- **Free-credit abuse**: $10/GitHub account invites farm accounts. Mitigate
  with idempotent per-GitHub-id grants, GitHub account-age/activity
  heuristics, per-IP mint caps (pattern exists), device attestation as
  fast-follow, and clawback (exists).
- **Gemini/Vertex pricing truth**: `VERTEX_COST_IS_LIST_TODO` — real
  per-token cost is still list rate; fine for MVP margins, flagged.
- **Android evidence gap**: no boot proof yet; the Play lane is net-new.

## 6. What we are NOT throwing away

The desktop app, Pylon fleet system, Khala Sync engine, OTA server,
TestFlight lane, behavior-contracts machinery, QA harness, and the entire
credits/metering substrate all remain live and are load-bearing for this
pivot. The postponements are scoped: desktop-*pairing* flows, desktop-fleet
*sequencing*, and the D1-decommission long tail.

## 7. Relationship to prior roadmaps

- `ROADMAP.md` (desktop-fleet push) — its WS-11 "mobile companion
  postponed" is inverted: mobile is now first; desktop fleet is postponed.
- `2026-07-04-mobile-companion-and-khala-sync-report.md` — its §9 Expo
  addendum is the framework baseline; its pairing-model framing is
  superseded by this pivot.
- `ROADMAP_QA.md`, `ROADMAP_BIZ.md`, QA Swarm, Reactor (rx-*), business
  fulfillment (bf-*), lead-gen (lg-*) lanes — unaffected in content, but
  not the active push.

## 8. Open-issue triage (all 15 reviewed) and the reopen ledger

Owner direction: close what is not relevant to this push as
wontfix/later, record what to reopen down the road. All 15 open issues
were closed 2026-07-05 with comments pointing here. **Reopen ledger:**

| Issue | What it was | Reopen when |
|---|---|---|
| #8282 | EPIC: Khala Sync (engine live; remaining children were D1-decommission cleanups) | Post-MVP cleanup wave — reopen alongside #8330 |
| #8330 | KS-8.19 cron consolidation + D1 retirement (closing sweep) | Post-MVP, after the soak-gated domains below |
| #8335 | KS-8.6 Artanis read cutover evidence (soak-gated) | Post-MVP; soak evidence keeps accruing passively |
| #8336 | KS-8.9 entitlements read cutover (soak-gated) | Post-MVP; note entitlements gate-reads matter to credits — do not flip during launch |
| #8337 | KS-8.7 billing D1 decommission (likely partly permanent) | Post-MVP owner call |
| #8362 | KS-8.18 identity/auth read cutover (owner-hardening-gated) | Dedicated auth-hardening project, post-MVP |
| #8339 | Epic: ONE-UI (React+Tailwind everywhere) | The mobile MVP epic (§9) is the active tracker; reopen for the web/desktop migration remainder post-MVP |
| #8348 | TS-6 web app-shell migration (needs apps/start auth infra) | Post-MVP; note the mobile session work (WS-A) may build the session primitives TS-6 needed |
| #8351 | TS-10b UI velocity receipt (time-gated) | When the 30-day window has data, post-MVP |
| #8354 | MC-5 cross-device dogfood (phone↔desktop↔web) | When desktop pairing returns; evidence bundle already compiled |
| #8420 | KS-6.10 capstone: retire legacy web sync spine | Post-MVP with #8422-#8425 |
| #8422 | KS-6.11 parent (team chat/thread files/agent-goal CRUD) | Post-MVP |
| #8423 | KS-6.11a team chat client repoint (one verified deletion left) | Post-MVP quick win — smallest reopen on this list |
| #8424 | KS-6.11c agent-goal CRUD sync cutover (from-scratch build) | Post-MVP |
| #8425 | KS-6.x notifyAgentRunSyncScopes consumer verification | Post-MVP with #8420 |

Prior closure analysis for these (what each needs when reopened):
`docs/cleanup/2026-07-05-open-issues-closure-audit.md`.

## 9. The launch backlog (filed as GitHub issues)

Filed 2026-07-05 under the epic **#8467** (the epic carries the live
dependency map in its first comment):

- **WS-A Mobile auth**: #8468 (closed: PKCE/mobile session on the issuer), #8469
  (closed: session→Khala Sync credential bridge), #8470 (closed: mobile GitHub
  sign-in UI + Tailnet contract retirement).
- **WS-B Repos**: #8471 (mobile-bearer repo API), #8472 (repo picker UI +
  thread↔repo binding).
- **WS-C Cloud execution**: #8473 (closed: org cloud executor pool), #8503
  (AC-1: Agent Computers — arm the Firecracker/GCE provisioning path, first
  real microVM turn), #8474 (credit-gated dispatch/admission against
  agent-computer capacity), #8475 (private-repo checkout via user OAuth
  through OUR SCM broker — the only credential path into an agent
  computer), #8476 (isolation posture per the strategy doc §4:
  Firecracker per-work-context), #8477 (branch/PR writeback via user
  GitHub authorization).
- **WS-D Credits**: #8478 ($10 GitHub-keyed signup grant), #8479 (coding-run
  metering + balance gate), #8480 (balance + history UI).
- **WS-E IAP**: #8481 (RevenueCat client integration), #8482 (server
  receipt validation + IAP rail + fulfillment), #8483 (store compliance:
  3.1.1, restore, refunds/clawback).
- **WS-F Models**: #8484 (per-user model config end to end).
- **WS-G Push**: #8485 (device token registration), #8486 (server push
  sender + notify events).
- **WS-H Surface**: #8487 (Settings rework), #8488 (onboarding straight
  line), #8489 (behavior-contracts pivot).
- **WS-I Launch ops**: #8490 (Android proof + Play lane), #8491 (App Store
  submission pack), #8492 (E2E QA for the straight line), #8493
  (promises/copy launch gates).
- **WS-J Growth (post-MVP-gated)**: #8494 (post-to-earn credits).

## 10. Product-promise registry changes (landing with this doc)

Registry bump `2026-07-05.3 → 2026-07-05.4` (see the registry note of the
same version):

- **New planned record `khala_code.mobile_mvp.v1`** — the mobile-only MVP
  claim (GitHub sign-in, cloud execution on OpenAgents Cloud, $10 credit
  grant, IAP credit purchases, per-user model config, push notifications,
  iOS + Android). Planned; no state flips; evidence accrues on the §9
  issues.
- **`mobile.fleet_companion.v1` rescoped** (stays planned): the
  desktop-pairing companion model is postponed by owner direction
  2026-07-05; record now points at `khala_code.mobile_mvp.v1` as the active
  mobile path and drops the stale "native SwiftUI / no Expo" framing
  (superseded by the 2026-07-04 Expo decision).
- No green flips; the green-count pin (34) is untouched.

## 11. Execution parallelization (added 2026-07-05, after MM-A1/MM-A2 landed)

Status at time of writing: #8468 (MM-A1 PKCE/mobile session) and #8469
(MM-A2 session→sync token) are closed by the main Codex agent. The
remaining issues split into concurrent lanes chosen to avoid seam
collisions. **Ownership rule: each lane exclusively owns its named
surfaces.** Cross-lane contracts land as small contract-first commits on
`main` before consumers build on them; if a lane needs a contract another
lane hasn't landed yet, it posts the needed shape as a comment on that
issue and continues with unblocked work.

### Lane 0 — main Codex agent (serial; the critical path)

**Cloud execution spine + metering**, in order:
`#8473 (C1) → #8474 (C2) → #8475 (C3) → #8476 (C4) → #8477 (C5) → #8479
(D2, once #8478 lands in Lane 2)`.

Owns exclusively: the org-executor infrastructure (`apps/pylon` runtime
consumer + deploy), the dispatch/admission seams in `workers/api`
(`coding-workflow-delegation.ts`, cloud session routes, the new admission
gate), and the `INVARIANTS.md` updates that come with #8474. This is the
deepest-risk authority work, inherently sequential, and Codex has the
freshest context on these seams from A1/A2.

### Lane 1 — Sonnet: mobile app surface (exclusive owner of `clients/khala-mobile`)

Serial within the lane:
`#8470 (A3 sign-in UI + Tailnet retirement) → #8472 (B2 repo picker) →
#8487 (H1 Settings rework) → #8480 (D3 balance UI) → #8488 (H2
onboarding) → #8489 (H3 contracts pivot final pass)`.

Seam notes: #8472's **first commit must be the typed thread↔repo binding
contract in `packages/khala-sync`** — Lane 0's C1/C3 consume it. Build the
picker against #8471's route shape (Lane 2); agree the shape in issue
comments if B1 hasn't merged yet.

### Lane 2 — Sonnet: Worker API billing/repos/models

Serial within the lane: `#8471 (B1 repo API) → #8478 (D1 $10 grant) →
#8484 (F1 per-user model config)`.

Owns: the repo-listing, grant, balance/history, and model-preference route
seams in `workers/api`. Seam notes: F1's server side (preference store +
read API) is the bulk; Lane 0's C1 consumes the preference read. F1's
Settings-row UI goes in as a follow-up after Lane 1's #8487 merges — this
lane does not edit `clients/khala-mobile` screens while Lane 1 is active.

### Lane 3 — Sonnet: push + IAP server (greenfield surfaces)

Serial within the lane: `#8485 (G1 device registration) → #8486 (G2 push
sender) → #8482 (E2 server IAP rail) → #8483 (E3 compliance pass)`.

Seam notes: G1's client half adds `expo-notifications` (native module →
runtime-fingerprint bump per the OTA runbook) — land the
`clients/khala-mobile` native/app.json change as one small coordinated
commit between Lane 1 issues, not concurrently with them. E2 builds
against the RevenueCat webhook contract with fixtures; live sandbox
verification waits for E1.

### Held / gated — do not start yet

- **#8481 (E1 RevenueCat client)**: needs owner-created RevenueCat account
  + store products first — file the NEEDS_OWNER entry, then it joins
  Lane 3.
- **#8490–#8493 (I1–I4)**: convergence tier — start after Lanes 0–3 land
  their cores. Exception: #8490's emulator-smoke half can start whenever a
  Sonnet slot frees (Play Console steps stay owner-gated).
- **#8494 (J1)**: post-MVP-gated.

### Conflict rules (all lanes)

- `clients/khala-mobile` belongs to Lane 1. `packages/khala-sync` changes
  are contract-first mini-commits announced on the epic. `workers/api`
  splits by route seam: Lane 0 dispatch/cloud, Lane 2
  billing/repos/models, Lane 3 push/IAP.
- Merge to `main` early and often; rebase before push; never touch another
  lane's files; clean worktree per issue; tests + `check:deploy` green;
  comment + close each issue; NEEDS_OWNER routing for gated steps; no
  `--no-verify`.

### Instruction block for the main Codex agent (copy-paste)

> Change of plan for parallelization: other agents are taking the mobile
> UI lane (#8470, #8472, #8487, #8480, #8488, #8489), the
> billing/repos/models API lane (#8471, #8478, #8484), and the push/IAP
> server lane (#8485, #8486, #8482, #8483). You focus exclusively on the
> cloud execution spine, in order: #8473 → #8474 → #8475 → #8476 → #8477,
> then #8479 once #8478 has landed. Do not modify `clients/khala-mobile`
> or the billing/push/IAP route seams — if you need a contract from
> another lane (the thread↔repo binding from #8472, the model-preference
> read from #8484, the grant/balance from #8478), consume the merged
> contract from main, or post the shape you need as a comment on that
> issue and continue with what's unblocked. Same per-issue loop as before
> (clean worktree, tests + check:deploy green, comment/close, push to
> main, cleanup). All other rules from the original delegation stand. The
> full lane map is §11 of
> docs/fable/2026-07-05-khala-code-mobile-only-mvp-launch-audit.md.

## 12. Status ledger + relaunch lanes (2026-07-06 — supersedes §11)

Everything below reflects the state after the first parallel dispatch wave
finished and the Codex (Lane 0) agent stopped. §11's lane map is historical;
this section is the operative plan. Two new owner decisions are folded in:

1. **IAP is postponed for the first MVP build** (#8481 closed as postponed).
   Credits are assigned **manually by the owner through Aiur** (below). The
   server IAP rail from #8482 stays landed-but-dormant, so IAP's eventual
   return is client-integration work only.
2. **Aiur** — a separate owner-only admin app at `aiur.openagents.com`
   (fresh Cloudflare Worker, TanStack Start, same Khala Sync engine),
   filed as #8499 (scaffold + owner-only auth + deploy), #8500 (credits
   console — the manual-grant surface that replaces IAP at launch), #8501
   (ops views: users/runs/executor health). #8500 is on the MVP critical
   path: it is how users get credits at launch.
3. **Agent Computers on Firecracker/GCP (supersedes the brief same-day
   exe.dev direction)** — owner decision recorded in
   `docs/khala-code/2026-07-06-agent-computers-strategy.md`: turns execute
   in Firecracker microVMs on our own GCE infrastructure via the
   already-built `cloud/`-repo provisioner and the flag-gated
   `cloud-coding-session-routes.ts` seam (#8503 arms it). Agent computers
   are **separately billable** — active compute time draws against credits
   alongside exact token usage. Product authority is unchanged: OpenAgents
   owns admission, accounting, sync, and owner-scope invariants.

### 12.1 What landed (17 of 27 original workstream issues closed + shipped)

All merged to `main`, each closed with evidence on its issue:

- **WS-A auth**: #8468 (OpenAuth PKCE public mobile client + cookie-free
  bearer sessions, `GET/DELETE /api/mobile/auth/session`), #8469
  (`POST /api/mobile/session` → `{ownerUserId, syncToken}`; sync accepts
  the mobile bearer through the human-actor path), #8470 (mobile GitHub
  sign-in UI + Tailnet auto-auth retirement, incl. a real device/simulator
  smoke + OTA receipt).
- **WS-B repos**: #8471 (`GET /api/mobile/repos[/{owner}/{name}]`, typed
  token-missing/expired failures), #8472 (repo picker UI +
  `ChatThreadEntity.repoBinding` contract in `packages/khala-sync` — the
  contract the cloud executor consumes).
- **WS-D credits (2 of 3)**: #8478 ($10 GitHub-account-keyed signup grant
  into Pool B via the usd-credit-bridge, double-idempotent, race-verified,
  RL-3-compliant, account-age + per-IP anti-abuse floors), #8480 (balance +
  transaction history UI; also wired the real model picker).
- **WS-C cloud execution (1 of 5)**: #8473 (org-cloud hosted executor
  spine, hosted Khala/Gemini lane, exact runtime usage receipts, and the
  operator runbook; core implementation commit `961d2f94ed` after rebasing
  the stranded `decbe52666` work onto `6552e38c08`).
- **WS-E IAP (2 of 3)**: #8482 (server `iap_revenuecat` rail: webhook
  auth, SKU catalog, idempotent Pool B fulfillment, refund clawback —
  **dormant by owner decision**), #8483 (store-compliance checklist; found
  credit packs run at a small loss at the standard 30% store cut →
  Small Business Program recommendation; flagged missing account-deletion
  mechanism).
- **WS-F**: #8484 (per-user model preference store + mobile API + picker;
  one deliberately-deferred finding: the public gateway's
  single-Khala-alias invariant was NOT relaxed — owner/Lane-0 call,
  documented in INVARIANTS.md).
- **WS-G push**: #8485 (expo-notifications + `push_device_tokens` +
  mobile-bearer register/unregister, permission prompt on first dispatch,
  native builds verified both platforms), #8486 (Expo push sender,
  payload-safety fuzz-tested, `POST /api/internal/push/notify-events`
  ingest seam built against the C-lane's documented shape).
- **WS-H surface**: #8487 (Settings rework — Fleet section gone), #8488
  (onboarding straight line), #8489 (contracts pivot final pass + push
  tap→thread deep-link handler gap found and fixed).
- **Post-lane fixes on main**: `a87e0a5c0a` (architecture-gate closure for
  the wave), `19cdf912ea` (pre-existing raw-Env debt ratchet 167→171,
  tracked by #8498), `da3b5bbe64` (stale-workspace-link DX note, #8497).
- **In flight at time of writing**: a visual-only sign-in screen redesign
  (owner-picked "Nexus Beam" wireframe direction: beam + code glyphs +
  diamond aperture background, restyled GitHub button, drop the
  "no desktop/Tailnet" note) — single-file change to
  `clients/khala-mobile/src/components/sign-in-screen.tsx`, pushing to
  main when verified.

### 12.2 What's open and why

| Issue | State | Why it's open |
|---|---|---|
| #8503 (AC-1 Agent Computers) | **In progress, public seam landed; live proof owner-gated** | Arms the existing Firecracker/GCE provisioning path (`cloud-coding-session-routes.ts` + the private `cloud/` provisioner) and proves the first real mobile turn inside a microVM, with lifecycle receipts. Public repo now projects Agent Computer work-context/lifecycle/resource receipt refs from `cloud.gce.*` events and documents/tests the nested-virt GCE host bootstrap under `apps/pylon/deploy/agent-computer/`. Remaining proof requires owner-gated live host/image/control-plane receipts. Strategy: `docs/khala-code/2026-07-06-agent-computers-strategy.md`. |
| #8474–#8477 (C2–C5) | Not started | Were serialized behind C1; now build against the Agent Computers strategy (redirect comments on each issue): #8474 admits against agent-computer capacity from the control-plane ledger, #8475 delivers scoped credentials into the microVM via OUR SCM broker only, #8476 documents/enforces the strategy §4 Firecracker posture, #8477 unchanged. All exe.dev framing withdrawn. |
| #8479 (D2 metering) | Not started | Expanded: charges BOTH meters — exact token receipts (landed) and agent-computer compute-time from lifecycle receipts (`resource_usage_receipt.v1`). Owns the mid-run exhaustion policy + pre-dispatch cost line. Compute rate is NEEDS_OWNER. Host/VM metrics are ops telemetry only, never billing truth. |
| #8490 (I1 Android/Play) | **Closed 2026-07-06** (Lane S3) | Agent-doable half shipped: real Android SDK/emulator bring-up, real Gradle debug build, launch+interaction Maestro pass (both a reused iOS flow AND a new sign-in-tap flow), Android build+upload runbook. Play Console app record/signing/upload remains owner-gated (`~/work/NEEDS_OWNER.md`). |
| #8491 (I2 App Store pack) | **Closed 2026-07-06** (Lane S3) | Agent-doable submission-readiness doc shipped (listing copy, screenshot shot-list, privacy nutrition label, required-reason API notes, age rating, App Review notes, TestFlight staging plan). Found a real, previously-undocumented-as-launch-blocking gap: account deletion (Apple 5.1.1(v)) is unbuilt — filed #8502 as a tracked follow-up. ASC account actions remain owner-gated. |
| #8492 (I3 E2E QA) | **Closed 2026-07-06** (Lane S3) | Confirmed both platforms' Maestro flows (from #8490) run the SAME `.yaml` unmodified; added real RN component-mount coverage for RepoPickerScreen (new enforced contract, extends the ChatComposer harness with a FlatList leaf stub). Full straight-line E2E (through a completed cloud-executed turn) stays honestly deferred — needs a seeded test GitHub account (`~/work/NEEDS_OWNER.md`) AND the remaining C-lane issues below. |
| #8493 (I4 promise gates) | In progress (Lane S3) | This pass's own evidence-accrual + copy-audit work — see §12.4. |
| #8502 (account deletion, App Review 5.1.1(v)) | **New**, open | Filed by Lane S3 from #8491's audit; Worker/API + mobile scope, blocks a real external-facing App Store/external-TestFlight submission (internal TestFlight is not blocked by it). |
| #8499–#8501 (Aiur) | #8499 closed, #8500/#8501 open | Owner-directed 2026-07-06; #8500 is MVP-critical (manual credits). |
| #8481 (E1 RevenueCat) | **Closed postponed** | Owner decision: no IAP in first MVP; server rail stays dormant. |
| #8494 (J1 post-to-earn) | Open, post-MVP-gated | Unchanged. |
| #8498 (raw-Env cleanup) | Open, background | Structural debt tracker; not launch-critical, not in any lane below. |

### 12.3 Relaunch lanes (Sonnet agents only; owner decision 2026-07-06)

Same ownership/conflict rules as §11 (exclusive surfaces, contract-first
cross-lane shapes, clean worktree per issue, tests + `check:deploy` green,
comment+close, no `--no-verify`, NEEDS_OWNER routing). Three lanes:

- **Lane S1 — cloud execution spine (the critical path)**:
  #8473 is landed; continue with **#8503 (AC-1 Agent Computers) → #8474 →
  #8475 → #8476 → #8477 → #8479** in order, per
  `docs/khala-code/2026-07-06-agent-computers-strategy.md`. The substrate
  is Firecracker microVMs on our own GCE hosts through the already-built
  `cloud-coding-session-routes.ts` → `oa-codex-control` seam — no exe.dev,
  no "hosted Pylon" language. S1 owns `apps/pylon` org-executor surfaces +
  the dispatch/admission and cloud/usage route seams in `workers/api`
  (control-plane authority stays in the private `cloud/` repo).
- **Lane S2 — Aiur**: #8499 → #8500 → #8501 in order. Owns the new
  `apps/aiur/` tree end-to-end plus the owner-gated admin credit routes it
  adds to the main Worker (`/api/admin/credits/*` seam only). #8500 lands
  the moment its routes + UI verify — it must not wait for #8501.
- **Lane S3 — launch ops prep (everything not gated on the C-lane)**:
  #8490's emulator-smoke half + Android runbook, #8491's
  metadata/privacy-label/review-notes prep (ASC actions → NEEDS_OWNER),
  #8492's E2E flows for what exists today (sign-in → repo pick →
  compose; extend to full straight line when S1 lands), and #8493's
  running promise-evidence accrual. Owns `clients/khala-mobile`'s test/QA
  surfaces and `docs/khala-mobile/` — coordinate with the in-flight
  sign-in redesign commit before touching that one file.

Dependency notes: S2 is fully independent of S1. S3's full-straight-line
E2E and #8493's final pass converge on S1's completion. Nothing waits on
IAP anymore. The owner-gated residue at launch time: Play Console + App
Store Connect actions, promise green sign-offs, and (post-MVP) the
RevenueCat account when #8481 reopens.

### 12.4 Lane S3 complete (2026-07-06)

All four Lane S3 issues (#8490-#8493) are closed. Summary, in order:

- **#8490 (I1)**: real Android SDK/emulator bring-up from scratch (Homebrew
  `android-commandlinetools`, `emulator` + `system-images;android-35;
  google_apis;arm64-v8a`), a real local Gradle debug build, install, launch,
  and a passing Maestro flow — both the existing iOS `LaunchFallback.yaml`
  reused unmodified AND a new `LaunchGitHubSignInInteraction.yaml` (tap ->
  real external-browser handoff). Android build+upload runbook written.
  Play Console app record/signing/upload stays owner-gated.
- **#8491 (I2)**: App Store submission-pack doc (listing copy bounded by
  `khala_code.mobile_mvp.v1`'s `safeCopy`, screenshot shot-list, privacy
  nutrition label grounded in a real grep pass — no third-party analytics/
  crash SDK exists, IAP doesn't apply since postponed — required-reason API
  notes, age rating, App Review notes, TestFlight staging plan). Found and
  filed a real gap: account deletion (Apple 5.1.1(v)) is unbuilt — **#8502**,
  new tracked follow-up (Worker/API + mobile scope).
- **#8492 (I3)**: confirmed cross-platform Maestro flow reuse; added real RN
  component-mount coverage for `RepoPickerScreen` (new enforced contract
  `khala_mobile.repo_picker.rn_component_mount_coverage.v1`, extends the
  shared `bun test` harness with a `FlatList` leaf stub). Found and fixed two
  real `bun:test` cross-file `mock.module` leakage bugs in the process
  (documented in the new test file's header for future authors). Also found
  a genuine signed-in dogfood session on a dev Mac's iOS simulator, used
  carefully for read-only bonus evidence without extracting its credential
  or spending real credits through it. Full straight-line E2E (through a
  completed cloud-executed turn + push) stays honestly deferred pending a
  seeded test GitHub account and the remaining C-lane issues.
- **#8493 (I4)**: this pass's own evidence-accrual on `khala_code.
  mobile_mvp.v1` and launch-copy audit (see the registry note landing with
  this same change for the exact evidence/blocker updates). No green flip;
  none of the mobile app's own copy (`clients/khala-mobile/src/i18n/copy.ts`)
  was found to overclaim.

Two new NEEDS_OWNER items from this lane (Play Console setup, a seeded
public-safe test GitHub account) join the existing owner-gated residue above.
Nothing in Lane S3's scope required touching `apps/pylon`, `apps/aiur`, or
the Worker's dispatch/admin route seams — the in-flight sign-in redesign
(commit `83ad352bd3`) was left untouched except for one small, unrelated,
already-necessary fix: `LaunchFallback.yaml` and its policy test asserted a
"No desktop, Tailnet..." string that redesign correctly dropped, repointed at
the new stable tagline (commit `4c805e56b1`).
