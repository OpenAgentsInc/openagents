# Pylon v0.2 OpenAgents product surface Release Gate Runbook

Date: 2026-06-07
Related issues: #432, #438, #489, #490, #491, #492, #493, #499, #505

## Summary

`@openagentsinc/pylon@0.2.5` is now the downloadable package launcher and npm
`latest` points at `0.2.5`. This package-launcher release shipped after the
OpenAgents product surface/Nexus network-readiness gate passed for a limited release: macOS arm64
and Linux x86_64 package-launcher smokes work, and two distinct Pylons have
public-safe accepted-work bitcoin receipts.

This runbook is the release classifier for the current OpenAgents product surface-owned Nexus path.
It replaces the old plan to make the Google Cloud Nexus VM the normal release
gate. Old Google Cloud Nexus evidence may be retained as transition context,
but a future agent must not require SSH into that VM to decide whether Pylon
v0.2 can be announced.

The typed checklist lives in:

- `workers/api/src/pylon-v02-openagents-release-gate.ts`
- `workers/api/src/pylon-v02-openagents-release-gate.test.ts`

#434 selected the Worker-safe MDK-compatible runtime boundary, #438 retained
an Artanis-administered real small-bitcoin assignment with public-safe
settlement receipt evidence, #489 aligned npm/GitHub release artifact evidence
for `pylon-v0.2.4`, and #505 published the corrected `0.2.5` package launcher
with OpenAgents registration and MDK wallet-readiness flags.

Do not announce unrestricted earning, native Windows readiness, WSL Ubuntu
readiness, hosted MDK direct programmatic payout readiness, or autonomous
Artanis production operation solely because the launcher release exists. The
gate records public evidence and does not grant wallet spend, provider
mutation, autonomous scheduling, or settlement authority by itself.

#499 added the stronger network-readiness release freeze. #500 through #505
closed it for the limited downloadable launcher release. The current network
state is:

```text
limited_launcher_release_shipped
```

The canonical freeze and release-unfreeze checklist lives in:

- `docs/nexus/2026-06-07-pylon-network-readiness-release-freeze.md`

Future package, GitHub binary, or broad earning releases should still reuse the
#499 checklist before widening claims beyond the current limited launcher
release.

## Current Proof Epic

The post-#481 proof epic is #485 through #488. The purpose is to close the
remaining gap between "operator release review evidence exists" and "the
Artanis/Nexus/Pylon paid-work loop is repeatable enough to claim publicly."

- #485 added the evidence-only Artanis/Pylon proof trace checker. It classifies
  a single assignment chain across dispatch, accepted work, artifact/proof,
  payment, settlement, public receipt, real bitcoin movement, and terminal
  settlement. It cannot dispatch work, mutate Pylons, create receipts, spend
  bitcoin, settle payments, or publish releases.
- #486 added the operator proof-run API around the settlement bridge:
  `POST /api/operator/nexus-pylon/proof-runs`.
- #487 requires multiple distinct Pylons with complete paid-work traces before
  stronger release claims are allowed.
- #488 publishes the proof state through Artanis public and Forum surfaces
  without overclaiming autonomy, release, or settlement. The public Artanis
  report now includes `pylonOpenAgents product surfaceReleaseGate`, `/artanis` renders an OpenAgents product surface
  release-gate panel, and the Artanis Nexus/Pylon Forum bridge can emit
  blocked or passed release-gate updates into the Pylon release work-log topic.
- #489 aligned the public release artifact layer: GitHub lists
  `pylon-v0.2.4`, npm `@openagentsinc/pylon@latest` resolves to `0.2.4`, and
  a clean package-resolved `npx` smoke reports `version: 0.2.4`,
  `tagName: pylon-v0.2.4`, `installMethod: release_asset`, and offline ready
  runtime status without local package-directory authority.
- #491 retained the second distinct paid-work proof trace. The public Artanis
  report now has `multiPylonPaidWorkProofComplete: true`,
  `multiPylonObservedDistinctPylonCount: 2`, no blocker refs, and state
  `ready_for_operator_release_review`.
- #492 verified that the ready-for-operator-review state is published through
  `/api/public/artanis/report`, the rendered `/artanis` page, and the Pylon
  release work-log Forum topic.
- #493 added the retained release-review record and rollback plan in
  `docs/nexus/2026-06-07-pylon-v02-release-review-record.md`.
- #499 froze new Pylon releases and earning/download announcements until the
  live network readiness sequence proves fresh install, registration,
  heartbeat, wallet readiness, job assignment, accepted-work proof, bitcoin
  payout, public receipts, repeated multi-host smokes, and rollback drills.
- #500 added source-level opt-in Pylon launcher registration in
  `OpenAgentsInc/openagents@b04ebe4be` and retained one clean local production
  smoke plus one reachable Arch Linux second-host registration/heartbeat smoke in
  `docs/nexus/2026-06-07-pylon-self-serve-registration-smoke.md`.
- #501 added source-level opt-in MDK agent-wallet readiness reporting in
  `OpenAgentsInc/openagents@6983d0512` and retained one production smoke with
  redacted wallet readiness plus payout-target admission in
  `docs/nexus/2026-06-07-pylon-mdk-wallet-readiness-smoke.md`.
- #505 published `@openagentsinc/pylon@0.2.5`, verified npm `latest`, retained
  fresh macOS and Arch Linux package-launcher registration plus wallet
  readiness smokes, and recorded rollback commands in
  `docs/nexus/2026-06-08-pylon-downloadable-launcher-release-0.2.5.md`.
- #502 added OpenAgents product surface assignment leases, owned Pylon assignment lists, live
  assignment accept/progress/artifact-proof refs, and operator accepted-work
  closeout. Production smoke `pylon.issue502.local.20260608024927` /
  `assignment.public.issue502.20260608024927` closed as `accepted_work` with
  retained public-safe proof refs. Real bitcoin payout remains #503.

## Authority Boundary

The release gate is evidence-only.

It can:

- classify required evidence as passed, pending, or blocked;
- expose public-safe blocker refs;
- point future agents to runbook commands and receipt surfaces;
- distinguish optional old Google Cloud transition evidence from required
  OpenAgents product surface/Nexus evidence.

It cannot:

- publish a Pylon release;
- publish packages or assets;
- mutate providers;
- dispatch paid work;
- spend bitcoin;
- settle payouts;
- upgrade public claims by itself.

The typed gate enforces these authority fields as false:

- `releasePublicationAllowed`;
- `publicClaimUpgradeAllowed`;
- `walletSpendAllowed`;
- `settlementMutationAllowed`;
- `providerMutationAllowed`;
- `oldGoogleCloudNexusRequired`.

## Current Required Gate State

| Gate | Current state | Evidence or blocker |
| --- | --- | --- |
| OpenAgents product surface payout ledger migration deployed | Passed | #431, `workers/api/src/nexus-treasury-payout-ledger.test.ts` |
| Payment authority service deployed | Passed | #428, `workers/api/src/treasury-payment-authority.test.ts` |
| Simulation adapter conformance tests green | Passed | #427, `workers/api/src/pylon-marketplace-payout-flow.test.ts` |
| MDK adapter mocked tests green | Passed | #431, `workers/api/src/treasury-payment-mdk-agent-wallet-adapter.test.ts` |
| Live MDK runtime boundary explicit | Passed | #434 selected a Worker-safe MDK-compatible route boundary and keeps native node-control behavior outside Worker code. |
| Worker does not import native MDK node runtime | Passed | Source audit says do not import `@moneydevkit/lightning-js` or `createMoneyDevKitNode` into Worker code. |
| Real two-wallet MDK movement proof green | Passed | `receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507` |
| Pylon registration and heartbeat green | Passed | `workers/api/src/pylon-api-routes.test.ts` |
| Pylon wallet readiness green | Passed | #436 bucketed wallet readiness plus #501 source-level MDK agent-wallet readiness smoke for `pylon.issue501.local.20260608023035`. |
| Assignment acceptance and status green | Passed | `workers/api/src/pylon-marketplace-jobs.test.ts`, `workers/api/src/pylon-provider-job-lifecycle.test.ts`, and #502 production smoke `assignment.public.issue502.20260608024927`. |
| Artifact/proof upload green | Passed | `workers/api/src/pylon-accepted-work-proof-links.test.ts` and #502 production smoke proof ref `proof.public.issue502_echo_verified`. |
| Live assignment lease closeout green | Passed | #502, `docs/nexus/2026-06-08-pylon-live-assignment-closeout-smoke.md`, final state `accepted_work`. |
| Settlement receipts green | Passed | #429, #431, and #438 public-safe settlement projections. |
| Artanis simulated assignment green | Passed | #408 and #430 model assignment/progress/settlement Forum flow. |
| Artanis real small-bitcoin assignment green | Passed | #438 retained an Artanis assignment, accepted-work proof, artifact/proof refs, payout authority refs, settlement receipt, and Forum update intent. |
| Multi-Pylon paid-work proof | Passed | #491, `multiPylonPaidWorkProofComplete: true`, two distinct Pylon refs, no blocker refs, and terminal public-safe receipt evidence for `receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3`. |
| Public-safe receipt page green | Passed | `/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507` and `/nexus-pylon/receipts/receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221` |
| Operator dashboard green | Passed | `/api/operator/nexus-pylon/receipts/{receiptRef}` and operator dashboard route coverage. |
| Forum update bridge green | Passed | #430, Pylon release work-log topic. |
| Pylon v0.2.4 GitHub/npm artifact alignment | Passed | #489, GitHub release `pylon-v0.2.4`, npm `@openagentsinc/pylon@latest` at `0.2.4`, and clean package-resolved `npx` smoke. Superseded for npm latest by #505 package launcher `0.2.5`. |
| Pylon v0.2.4 clean launcher smoke | Passed for macOS package-launcher path | #490, local macOS arm64 clean HOME/cache no-launch and forwarded `status --json` smokes passed; #505 adds macOS package-launcher registration and wallet-readiness smoke for `pylon.issue505.npm.20260608035130`. |
| Source-level self-serve Pylon registration | Passed for limited launcher release | #500 added `--register-openagents`; #505 proves the npm `0.2.5` package launcher exposes the flag and registers on macOS and Arch Linux. Linux still resolves to `pylon-v0.2.2`, so do not claim Linux binary asset parity with macOS `pylon-v0.2.4`. |
| Source-level MDK wallet and payout-target readiness | Passed | #501, `OpenAgentsInc/openagents@6983d0512` adds `--setup-mdk-wallet`; production smoke `pylon.issue501.local.20260608023035` registered, heartbeated, posted wallet readiness, and requested payout-target admission with redacted refs only. |
| AGENTS.md and OpenAPI current | Passed | `/AGENTS.md` and `/api/openapi.json` describe current Pylon/Nexus receipt bounds. |

Optional transition evidence:

| Evidence | Required? | Notes |
| --- | --- | --- |
| Old Google Cloud Nexus health | No | Useful only as imported context. It is not a normal release classifier and must not block OpenAgents product surface/Nexus release classification by requiring SSH. |

## Clean Launcher Host Smoke Evidence

#490 retained the first package-resolved launcher smoke for
`@openagentsinc/pylon@latest` after #489 aligned npm to `0.2.4`.

Local macOS Apple Silicon evidence:

- `npx -y @openagentsinc/pylon@latest ... --skip-model-download
  --skip-diagnostics --no-launch --json`
- result: `version: 0.2.4`, `tagName: pylon-v0.2.4`,
  `installMethod: release_asset`, `cached: false`, target `darwin/arm64`,
  and desired mode `offline`;
- the command used a fresh HOME, install root, and Pylon home, so it did not
  depend on the operator's existing Pylon state;
- the terminal UI was intentionally not launched.

Forwarded status evidence:

- `npx -y @openagentsinc/pylon@latest ... --json -- status --json`
- result: `version: 0.2.4`, `tagName: pylon-v0.2.4`,
  `installMethod: release_asset`, target `darwin/arm64`, runtime mode
  `offline`, authoritative status `ready`, no runtime error, and two sellable
  offline launch products;
- host-local paths, node identity details, and machine-specific inventory were
  intentionally not copied into this public runbook.

Second-host route-around:

- local `tailscale status` could not reach the local Tailscale service from
  this shell;
- `ssh -o BatchMode=yes -o ConnectTimeout=5 christopherdavid@archlinux`

## Package Launcher 0.2.5 Evidence

#505 published `@openagentsinc/pylon@0.2.5` after the #499 through #504 network
readiness sequence closed. The package launcher now exposes
`--register-openagents`, `--setup-mdk-wallet`, `--openagents-api`,
`--mdk-wallet-home`, and related flags.

Evidence:

- `npm view @openagentsinc/pylon@latest version dist-tags bin --json` returned
  `latest: 0.2.5`;
- clean local macOS package-launcher smoke registered
  `pylon.issue505.npm.20260608035130`, resolved `pylon-v0.2.4`, and reported
  `walletReady: true`;
- clean Arch Linux package-launcher smoke registered
  `pylon.issue505.archnpm.20260608035227`, resolved `pylon-v0.2.2`, and
  reported `walletReady: true`; and
- rollback instructions are retained in
  `docs/nexus/2026-06-08-pylon-downloadable-launcher-release-0.2.5.md`.
  returned `Connection refused`;
- `ssh -o BatchMode=yes -o ConnectTimeout=5 christopherdavid@100.97.233.57`
  timed out;
- `ssh -o BatchMode=yes -o ConnectTimeout=5 christopherdavid@100.72.151.98`
  timed out.

Because no second host was reachable without blocking, #490 proves clean local
package invocation and forwarded status on macOS arm64 only. Linux, WSL Ubuntu,
native Windows, and a live Tailnet/SHC host remain unproven for general
availability claims.

Adjacent-repo cleanup:

- `gh release list --repo OpenAgentsInc/psionic --limit 20` returned no
  Psionic GitHub releases to delete.
- Psionic docs did contain confusing "Pylon Release" wording for the Qwen legal
  source boundary. Commit `ce3b0e0c` in `OpenAgentsInc/psionic` retitled those
  docs as the Psionic Qwen legal Pylon boundary and added explicit copy that
  they are not the OpenAgents public Pylon v0.2 release record.

## Multi-Pylon Paid-Work Proof Evidence

#491 moves the multi-Pylon proof check from blocked to passed.

Public report evidence from `GET /api/public/artanis/report`:

- `pylonOpenAgents product surfaceReleaseGate.state`:
  `ready_for_operator_release_review`;
- `pylonOpenAgents product surfaceReleaseGate.multiPylonPaidWorkProofComplete`: `true`;
- `pylonOpenAgents product surfaceReleaseGate.multiPylonObservedDistinctPylonCount`: `2`;
- `pylonOpenAgents product surfaceReleaseGate.multiPylonObservedPylonRefs`:
  `pylon.public.artanis.bridge.8b378373` and
  `pylon.public.issue_438_edge_wallet`;
- `pylonOpenAgents product surfaceReleaseGate.blockerRefs`: empty;
- `pylonOpenAgents product surfaceReleaseGate.multiPylonProofRefs` includes
  `artanis-mdk-bridge-8b378373002501f3e896dcd3`,
  `assignment.public.issue_438.issue_438_artanis_1780822221`,
  `proof.public.mdk_agent_wallet.real_bitcoin_moved.8b378373002501f3e896dcd3`,
  `receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221`, and
  `receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3`.

Second receipt evidence from
`GET /api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.artanis_mdk_bridge_8b378373002501f3e896dcd3`:

- `realBitcoinMoved: true`;
- `movementMode: real_bitcoin`;
- public projection state: `settled`;
- public Pylon ref: `pylon.public.artanis.bridge.8b378373`;
- assignment ref: `artanis-mdk-bridge-8b378373002501f3e896dcd3`;
- public amount: 21 satoshis of bitcoin.

This proof originally changed the gate to ready for operator release review.
#505 later moved the public gate to `limited_launcher_release_shipped` for the
npm launcher. It still does not grant release creation, wallet spend,
settlement mutation, provider mutation, autonomous scheduling, or broad
public-claim-upgrade authority; the public report keeps those authority flags
false.

## Public Publication Evidence

#492 verifies that the current release-gate result is visible on the public
surfaces agents and operators inspect.

`GET /api/public/artanis/report` currently projects:

- state: `limited_launcher_release_shipped`;
- state label:
  `Pylon v0.2 package launcher is shipped with listed platform and authority limits`;
- `multiPylonPaidWorkProofComplete: true`;
- observed distinct Pylons: `2`;
- blocker refs: empty;
- `releasePublicationAllowed: false`;
- `walletSpendAllowed: false`;
- `settlementMutationAllowed: false`;
- `providerMutationAllowed: false`;
- `publicClaimUpgradeAllowed: false`.

Rendered `/artanis` verification:

- the client-rendered page was checked with a temporary Playwright runner
  against local Chrome;
- the rendered body includes `OpenAgents product surface release gate`,
  `Pylon v0.2 OpenAgents Nexus release gate evidence is complete`, and
  `2 / 2 distinct Pylons`.

Forum publication:

- canonical topic:
  `https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888`;
- topic title: `Pylon release work log`;
- Artanis post #3 states that the dedicated Artanis identity is posting, links
  the latest public receipt and public report, says the paid-work evidence has
  moved into release review, and repeats that release publication, wallet
  spend, provider mutation, and scheduled autonomous operation remain behind
  separate launch gates.

No `/AGENTS.md`, OpenAPI, or capability-manifest change was needed for #492
because no agent-facing API authority changed.

## Release Review Record

The retained release-review decision and rollback plan lives at:

- `docs/nexus/2026-06-07-pylon-v02-release-review-record.md`

Historical #493 decision:

- state: `ready_for_operator_release_review`;
- operator approval state: not approved for general availability;
- release action state: release artifacts are public, but no new release action
  is created or approved by the review record;
- autonomous Artanis state: not approved.

Current #505 decision:

- state: `limited_launcher_release_shipped`;
- npm package-launcher release: `@openagentsinc/pylon@0.2.5`;
- native Windows and WSL Ubuntu: not public-ready claims;
- hosted MDK direct programmatic payout: still blocked by app configuration;
- autonomous Artanis production operation: not approved.

That record includes rollback instructions for bad npm latest/package state,
bad GitHub release metadata, bad public copy, false release-gate claims, bad
Forum posts, duplicate or stuck Artanis ticks, and bad payment/settlement
receipt projections.

## Automated Checklist

Run the focused gate test:

```bash
bun run --cwd workers/api test -- src/pylon-v02-openagents-release-gate.test.ts
```

Run the adjacent payout, receipt, Forum, and Pylon tests:

```bash
bun run --cwd workers/api test -- \
  src/artanis-real-small-bitcoin-assignment-smoke.test.ts \
  src/pylon-v02-openagents-release-gate.test.ts \
  src/treasury-payment-authority.test.ts \
  src/treasury-payment-mdk-agent-wallet-adapter.test.ts \
  src/nexus-treasury-payout-ledger.test.ts \
  src/nexus-pylon-visibility-routes.test.ts \
  src/pylon-api-routes.test.ts \
  src/pylon-marketplace-jobs.test.ts \
  src/pylon-provider-job-lifecycle.test.ts \
  src/pylon-accepted-work-proof-links.test.ts \
  src/pylon-settlement-bridge.test.ts \
  src/artanis-nexus-pylon-adapters.test.ts \
  src/artanis-nexus-pylon-forum-bridge.test.ts \
  src/openagents-agent-onboarding-routes.test.ts \
  src/openagents-openapi-routes.test.ts
```

Run typecheck:

```bash
bun run --cwd workers/api typecheck
```

Before a real release, also run the full Worker test suite:

```bash
bun run --cwd workers/api test
```

## Live Verification Commands

Public receipt API:

```bash
curl -fsS \
  https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507 \
  | jq '{receiptRef, movementMode, realBitcoinMoved, payoutMovement, settlement}'
```

The public API must show:

- `movementMode: "real_bitcoin"`;
- `realBitcoinMoved: true`;
- `payoutMovement.terminalSettlementClaimAllowed: true`;
- `settlement.providerRef: "provider.public.mdk_agent_wallet"`;
- `settlement.stateLabel: "Settled"`.

Artanis real-assignment receipt API:

```bash
curl -fsS \
  https://openagents.com/api/public/nexus-pylon/receipts/receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221 \
  | jq '{receiptRef, assignmentRef, movementMode, realBitcoinMoved, payoutMovement, settlement}'
```

The issue #438 public API must show:

- `assignmentRef: "assignment.public.issue_438.issue_438_artanis_1780822221"`;
- `movementMode: "real_bitcoin"`;
- `realBitcoinMoved: true`;
- `payoutMovement.terminalSettlementClaimAllowed: true`;
- `settlement.providerRef: "provider.public.mdk_agent_wallet"`;
- `settlement.stateLabel: "Settled"`.

Public receipt page:

```bash
curl -fsS \
  https://openagents.com/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507 \
  | rg "OpenAgents Nexus / Pylon receipt|real bitcoin moved: yes|Settled"

curl -fsS \
  https://openagents.com/nexus-pylon/receipts/receipt.nexus.issue_438.settlement.issue_438_artanis_1780822221 \
  | rg "OpenAgents Nexus / Pylon receipt|real bitcoin moved: yes|Settled"
```

Pylon registry/status:

```bash
curl -fsS https://openagents.com/api/pylons \
  | jq '{count: (.pylons | length), first: .pylons[0]}'
```

Public Pylon stats:

```bash
curl -fsS https://openagents.com/api/public/pylon-stats \
  | jq '{status, pylonsOnlineNow, pylonSessionsOnlineNow, sellablePylonsOnlineNow, asOfLabel}'
```

Pylon release work-log Forum topic:

```bash
curl -fsS \
  https://openagents.com/api/forum/topics/88888888-4004-4004-8004-888888888888 \
  | jq '{topic: {id: .topic.topicId, title: .topic.title}, postCount: (.posts | length)}'
```

AGENTS.md and OpenAPI:

```bash
curl -fsS https://openagents.com/AGENTS.md \
  | rg "Pylon v0.2|Nexus/Pylon|MDK|Forum"

curl -fsS https://openagents.com/api/openapi.json \
  | jq '.paths | keys[]' \
  | rg '/api/public/nexus-pylon/receipts|/api/pylons|/api/forum'
```

Operator receipt route requires admin auth:

```bash
curl -fsS \
  https://openagents.com/api/operator/nexus-pylon/receipts/receipt.nexus.issue_431.settlement.issue_431_authority_1780818513507 \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  | jq '{receiptRef, movementMode, realBitcoinMoved, payoutMovement, settlement}'
```

Do not print the token in shell logs or issue comments.

## Release Classification Rules

Classify as blocked if any required gate is absent, pending, or blocked.

Classify as ready for operator release review only if:

- every required gate in `requiredCheckKinds` is present;
- every required gate has `status = "passed"`;
- the public receipt/API surfaces still project public-safe real-bitcoin
  evidence;
- #434 has selected and documented a Cloudflare-compatible live MDK runtime
  boundary;
- a retained Artanis-administered real small-bitcoin assignment exists;
- at least two distinct Pylons have complete paid-work traces with terminal
  public-safe settlement receipts; and
- the public Forum release/update flow has retained the release-gate result.

Current public surfaces:

- `https://openagents.com/artanis`
- `https://openagents.com/api/public/artanis/report`
- `https://openagents.com/forum/t/88888888-4004-4004-8004-888888888888`

The report field `pylonOpenAgents product surfaceReleaseGate` is the canonical public-safe machine
projection. It includes the current blocked state, blocker refs, public receipt
refs, observed and required distinct Pylon counts, and the claim booleans that
must remain false until the gate passes.

Even when the gate is ready for operator release review, the checklist still
does not publish the release or grant release authority. A human/operator must
perform the release action through the normal release workflow.

## Blocked Claims

Do not say:

```text
Pylon v0.2 is released.
Pylon v0.2 accepted work payouts are live.
Artanis is assigning paid Pylon jobs with live bitcoin settlement.
The old Nexus VM proves Pylon v0.2 is ready.
```

Allowed now:

```text
OpenAgents product surface has a public-safe Nexus/Pylon release gate with real two-wallet MDK
movement evidence and retained Artanis assignment evidence. The gate now has
two distinct complete Pylon paid-work traces and is ready for operator release
review. The gate remains evidence-only and grants no release, spending,
settlement, provider-mutation, or public-claim-upgrade authority by itself.
```

## #434 Source-Audit Impact

The MoneyDevKit source audit added a required release-gate item:

- the live MDK runtime path must be explicit before any buyer-checkout claim;
- the Worker must not import `@moneydevkit/lightning-js`;
- the Worker must not host `createMoneyDevKitNode` directly;
- the selected path must be either a pure hosted/platform MDK API compatible
  with Cloudflare Workers or a Node-capable sidecar/function for native
  node-control behavior.

Fake preview helpers, query-parameter checkout success, or generic "MDK
webhook" language are not release evidence.

## Follow-Up Work

The typed gate no longer exposes required-evidence blockers for #434 or #438.

The next step is an explicit operator release-review decision. That decision is
outside this evidence classifier. It must not store raw invoice, raw payment
hash, preimage, mnemonic, wallet config, wallet home path, exact wallet
balance, private payout target, provider access token, webhook secret, customer
data, or operator-only notes in public projections.
