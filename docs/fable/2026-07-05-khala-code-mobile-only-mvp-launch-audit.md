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
- There is **no GitHub App** (no installation tokens, no fine-grained
  per-repo permissions) — all repo access rides the user OAuth token.

### 2.2 Mobile app: data plane cloud-clean, three desktop couplings

- `clients/khala-mobile` (Expo RN, `com.openagents.khala.mobile`, iOS build
  7 / Android versionCode 1) syncs exclusively against
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
- The three desktop couplings:
  1. **Auth**: Tailnet auto-discovery pulls `{ownerUserId, token}` from a
     signed-in desktop's `/khala-mobile-pairing` endpoint
     (`src/auth/khala-mobile-pairing-core.ts`); manual token paste is the
     only desktop-free path and requires possessing a token out-of-band.
  2. **Execution**: both lanes are consumed by
     `apps/pylon/src/orchestration/runtime-intent-enforcement.ts` running
     on the *user's* machine.
  3. **Settings → Fleet**: env-var fleet-run id, desktop-oriented copy.
- **No push notifications** (no expo-notifications, no token registration),
  **no IAP code**, **no GitHub/OpenAuth code** in the app — all confirmed
  absent by grep.
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
  - GCE hosting of Pylons is proven (`apps/pylon/deploy/gcloud/`) — an
    **org-owned hosted-Pylon pool** consuming runtime intents is the
    shortest credible path to cloud execution, since
    `runtime-intent-enforcement.ts` already implements both lanes
    end-to-end (workspace materialization, Codex/Claude SDK runs, sync
    event emission).
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
| C | Cloud execution | Both lanes fully implemented in `runtime-intent-enforcement.ts` (pylon-local); cloud-session scaffold; runner gateway; GCE pylon hosting; private `cloud/` Firecracker plane | Org-owned executor pool consuming runtime intents; dispatch-policy change (credit-gated org lane for mobile); private-repo checkout via user OAuth token through the SCM broker; per-run isolation posture; result writeback (branch/PR) via user token |
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
  (session→Khala Sync token issuance), #8470 (mobile GitHub sign-in UI +
  Tailnet contract retirement).
- **WS-B Repos**: #8471 (mobile-bearer repo API), #8472 (repo picker UI +
  thread↔repo binding).
- **WS-C Cloud execution**: #8473 (org cloud executor pool), #8474
  (credit-gated org-lane dispatch policy), #8475 (private-repo checkout via
  user OAuth through the SCM broker), #8476 (isolation posture doc +
  enforcement), #8477 (branch/PR writeback via user GitHub authorization).
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
