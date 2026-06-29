# Autopilot Desktop AO-6 — End-to-End First-Run Smoke + From-DMG Runbook

Date: 2026-06-18
Final proof update: 2026-06-19
Issue: #5447 (AO-6, verification). Parent EPIC: #5441.

This is the AO-6 verification record + manual runbook for the Autopilot Desktop
auto-onboarding EPIC. It proves that a clean macOS (Apple Silicon) machine,
installing the signed DMG and opening the app with **no terminal and no env
vars**, reaches a **registered → presence-live → Tassadar-joined → earning**
state, and that the AO-3 identity choice works.

It records refs, command shapes, gate names, and verifier outcomes only. It
contains no signing secrets, notary credentials, local tokens, identity seeds,
provider payloads, or raw private logs.

The work splits into two layers:

1. **Automated (this repo, runs headless on owned infra)** — the AO-6 smoke
   harness drives the REAL Pylon node through the desktop launcher against a mock
   `openagents.com`, and asserts the whole AO-1..AO-4 chain converges. No GUI, no
   terminal, no env vars, no faked results.
2. **Live-production proof (a physical Mac + a fresh signed DMG + a live
   validator pair)** — the genuinely physical gates: the from-DMG rendered
   window on a clean Mac, the node's appearance on the production Pylon API, and
   an actual claimed + **settled** Tassadar window with a real Bitcoin receipt.

The automatable scope is done, and the live-production from-DMG proof completed
on **2026-06-19**. The public-safe evidence is recorded in §3.

---

## 1. The automated smoke (what it proves, and how to run it)

### Run it

```sh
bun run --cwd apps/autopilot-desktop scripts/auto-onboarding-e2e-smoke.ts
```

This extends the Phase-1 headless convergence proof
(`scripts/auto-onboarding-headless-proof.ts`, AO-1/AO-2) rather than duplicating
it. It boots a fresh, empty managed home, points onboarding + presence at a local
mock of the `openagents.com` Worker, and drives the actual launcher
(`superviseManagedNode`) + the actual Pylon node — exactly the clean-machine
first-run path, with no GUI and no env vars.

Exit `0` means every automatable gate passed. The token in play is a fake
`oa_agent_...` value minted by the mock; the smoke asserts it is never printed to
any status/log surface, and never reads or prints an identity seed.

### Gates the harness proves AUTOMATICALLY

Part A — AO-3 first-run identity choice (#5444), both paths:

- Fresh machine: no existing identity detected; the choice is needed; create-new
  is always available.
- Use-existing: a seed-bearing `~/.openagents/pylon` home is **detected by marker
  presence only** (the seed is never read), the public npub is surfaced, and the
  choice screen still offers create-new alongside it.
- Use-existing persists the choice **outside** the seed home (never overwrites
  it), the seed marker is left byte-for-byte untouched (the v1.0.3/Orwell rule),
  and a use-existing choice against a **seedless** home is rejected (never adopt
  the wrong home).

Part B — create-new (named) convergence through the REAL node (AO-1/AO-2 +
the AO-3 name flowing through):

1. The node generated its identity (`identity.json` written).
2. The agent self-registered (`POST /api/agents/register`).
3. Registration used the node npub as `externalId`.
   - **3b (AO-3):** the user-chosen display name flowed into registration's
     `displayName`.
4. The minted token was persisted to the managed home (`agent-credential.json`).
5. Presence registered (`POST /api/pylons/register`).
6. Presence used the **bearer agent token** (not NIP-98).
7. The Spark payout target registered (`POST .../spark-payout-target`, #5305).
8. The Tassadar assignment worker polled for work (`GET .../assignments`).
9. The token was never printed to any status/log surface.
   - Plus: create-new minted a **fresh** managed home (not the existing fixture).

Part C — AO-4 wizard live-state projection (#5445), driven by the REAL observed
signals from Part B (no faked progress):

- After the chain converges, the wizard projects identity / registered /
  node-online / wallet / payout / presence as `done`.
- The chain is **not** falsely `complete` before a settled payout; the "you are
  here" pointer points at an unfinished step (Tassadar / earning).
- A claimed-but-unsettled snapshot shows Tassadar + claimed `done` and earning
  `active` (work claimed, awaiting settlement) — still not `complete`.
- The earning step is `done` and the chain is `complete` **only** once the wallet
  balance is > 0 (real settled sats). This is exactly the state the §3
  live-production run produced on real infra.
- A failed node surfaces a **retryable** failure (offline → retry, never a
  dead/blank screen).

### Black-screen regression guard (commit `73cada159`)

```sh
bun test --cwd apps/autopilot-desktop tests/black-screen-guard.test.ts
```

Foldkit's runtime renders `view(model).body`, so `view` (and the crash boundary
`crashView`) MUST return a `Document` (`{ title, body }`), not a bare `Html`.
When they returned a bare `Html`, `.body` was `undefined` and nothing mounted →
a black window. The guard:

- asserts `view(model)` returns a mountable `Document` for the default first-run
  (`network`) pane, the AO-4 `onboarding` pane, and **every** declared pane, and
- asserts at the source level that `crashView` is typed `=> Document` and returns
  a `{ title, body }` literal (importing `main.ts` would boot the webview
  runtime, so its module-private `crashView` is guarded structurally).

The test fails if a future change regresses either back to a bare `Html`.

### Run the whole desktop suite + builds

```sh
bun run --cwd apps/autopilot-desktop test
bun build --cwd apps/autopilot-desktop src/ui/main.ts  --outdir /tmp/ao6-ui  --target browser
bun build --cwd apps/autopilot-desktop src/bun/index.ts --outdir /tmp/ao6-bun --target bun
```

---

## 2. Why the rest is live-production proof (and must not be faked)

The harness proves the chain converges against a controlled mock and the real
local node. Three gates are physical and cannot be honestly automated in this
repo:

- **A rendered window on a clean, external Apple-Silicon Mac.** A headless smoke
  cannot confirm what a stranger sees when they open the signed `.dmg`. The
  black-screen guard proves the `Document` contract in source/unit form; it does
  not screenshot the published build on a fresh machine.
- **Production presence.** The smoke registers against a mock. Real appearance on
  `https://openagents.com/api/public/pylon-stats` requires the real Worker, a
  real minted agent token, and a network round-trip from a real install.
- **A settled Bitcoin receipt.** A claimed Tassadar window only becomes *earned*
  sats once a **live validator pair** settles it. That is real money on real
  infra and depends on a counterparty; it cannot be synthesized.

These require: a physical Mac, a fresh signed DMG (AO-5 / the current
`autopilot-desktop` release), and a live validator pair on the Tassadar run.

---

## 3. Completed from-DMG proof (2026-06-19)

Evidence directory:
`docs/launch/artifacts/ao6-20260619T010148`.

Build and notarization:

- App notarization submission:
  `117fe77a-0e62-43e7-b2dd-4e1794810cf1`, status `Accepted`.
- DMG:
  `apps/autopilot-desktop/artifacts/stable-macos-arm64-Autopilot.20260619T010148.notarized.dmg`.
- DMG SHA-256:
  `22db620c12c97f819fd6045eebd86cceb51c7cffc1ef2fc0d5b3f8446dd46358`.
- DMG notarization submission:
  `ccc6c3f9-fd2f-4477-9dc4-ad9c27613fec`, status `Accepted`.
- Gatekeeper:
  DMG and installed app accepted as `Notarized Developer ID`; `hdiutil verify`
  returned `VALID`.

From-DMG UI proof:

- Installed from the notarized DMG into:
  `/Users/christopherdavid/Applications/OpenAgents-ao6-20260619T010148/Autopilot.app`.
- Clean first launch screenshot:
  `initial-window.png` — rendered Get Started window, no black screen.
- Identity choice screenshot:
  `after-choice-cliclick-scaled.png` — create-new identity named
  `AO6 Patched2 DMG`.
- Wallet-ready wizard screenshot:
  `after-choice-wallet-done.png` — identity, agent registration, node online,
  wallet receive-ready, payout target, and presence all projected from live
  state.
- After settlement the app auto-navigated to Chat, which is the expected
  completed-onboarding behavior in the reducer. Local wallet status is captured
  in `wallet-status-after-settlement-summary.json`.

Production pylon proof:

- Pylon ref: `pylon.fa4e9049a4329f3d56e2`.
- Public pylon detail:
  `pylon-detail-summary.json` shows display name `AO6 Patched2 DMG`, status
  `active`, latest heartbeat `online`, `walletReady: true`,
  `sparkPayoutTargetReady: true`, and Spark payout target
  `payout.spark.3596dd0026ab64132e90ed1d`.
- Local wallet after settlement:
  `wallet-status-after-settlement-summary.json` shows `balanceSats: 10`,
  `receiveReady: true`, `sendReady: true`, and readiness `send-ready`.

Live Tassadar proof:

- Lease:
  `training.lease.00ea30b2-5165-4ca3-9398-a11545970ffa`.
- Run/window:
  `run.tassadar.executor.20260615`,
  `training.window.tassadar.executor.20260615.w1`.
- Verification challenge:
  `training.verification.challenge.9fd49062-f82c-46ee-a2a0-242d36dd126e`.
- Replay verdict:
  `Verified`, class `exact_trace_replay`, no failure codes, commitment digest
  and replay digest both
  `digest.tassadar.ao6.patched2.20260619T010148`.
- Settlement receipt:
  `receipt.nexus.tassadar_run_settlement.idempotency.tassadar.ao6.patched2.20260619T010148.manual.v1`.
- Settlement summary:
  `manual-settlement-response-summary.json` records `amountSats: 5`,
  `movementMode: real_bitcoin`, `realBitcoinMoved: true`, contributor
  `pylon.fa4e9049a4329f3d56e2`, and `adapterKind: spark_treasury`.
- Run summary after settlement:
  `training-run-detail-after-manual-summary.json` records
  `providerConfirmedSettledPayoutSats: 1020`, `settledReceiptCount: 5`,
  `qualifiedContributorCount: 5`, and `corpusAcceptedTraceCount: 11`.

Result: AO-6's final from-DMG live-production proof is complete for #5447 and
the #5441 EPIC evidence chain.

---

## 4. From-DMG runbook for reruns

Run this on a **clean** Apple-Silicon Mac (a fresh user account or VM profile is
sufficient — the point is "no developer tooling, no env vars, no prior Pylon").
Capture only public-safe evidence (npub / refs / counts / screenshots that do not
show seeds, tokens, or raw payout addresses).

### Pre-req

- The current signed + notarized DMG (per `docs/DEPLOYMENT.md` Autopilot Desktop
  row + `apps/oa-updates/docs/release-signing-runbook.md`). At audit time:
  `autopilot-desktop-v1.0.0-rc.3`, Apple Dev ID `HQWSG26L43`.
- A live Tassadar run with at least one validator able to pair + settle.

### Steps + expected evidence

1. **Install + open from the DMG, NO terminal.**
   - Download the signed DMG, drag to Applications, open it.
   - Expected: Gatekeeper accepts it (notarized Developer ID); the window
     **renders** the immersive first-run scene — **no black/blank window**.
   - Evidence: a screenshot of the rendered first-run window.
     - Gate: *From-DMG rendered window on a clean Mac.*

2. **AO-3 identity choice — create new (named).**
   - On the "Get started" first screen, choose **Create a new Autopilot
     identity** and give it a name.
   - Expected: the wizard advances; the name is the one used at registration.
   - Evidence: a screenshot of the choice screen + the chosen name.
     - Gate: *AO-3 create-new identity choice (named).*

3. **AO-3 identity choice — use existing (separate run, machine with a Pylon).**
   - On a machine that already has a Pylon home, confirm the choice screen shows
     **Use your existing Pylon identity** with the recognizable `pylon.<short>` /
     npub, and that choosing it boots that home (wallet/payout/history carry
     over, not a fork). Confirm create-new is still offered alongside it.
   - Evidence: a screenshot showing the detected existing identity + that
     choosing it did not fork (same npub afterward).
     - Gate: *AO-3 use-existing identity choice (no fork).*

4. **AO-1 self-register + token persisted.**
   - Expected: the wizard's "Agent registered" step reaches `done`; the agent is
     visible in the registry/forum.
   - Evidence: the registry/forum entry ref (public-safe). The token must **not**
     appear anywhere.
     - Gate: *Agent self-registered + visible.*

5. **AO-2 node online + wallet receive-ready + payout target + presence.**
   - Expected: the wizard shows node-online, wallet receive-ready, payout-target,
     and presence as `done`; and the node appears on production
     `https://openagents.com/api/public/pylon-stats`.
   - Evidence: a screenshot of the wizard chain + the pylon-stats entry (npub /
     ref only).
     - Gate: *Production presence on `/api/public/pylon-stats`.*

6. **Tassadar join → claim → SETTLE → earn.**
   - Expected: the node joins the Tassadar run, claims a window, and — with a
     live validator pair — the window **settles**, producing a real Bitcoin
     receipt; the wizard's "First sats earned" step reaches `done` and the chain
     reads `complete`.
   - Evidence: the settled-window receipt ref + the balance > 0 (public-safe);
     a screenshot of the completed wizard.
     - Gate: *Real claimed + settled Tassadar window with a Bitcoin receipt.*

7. **AO-4 wizard end-to-end on screen.**
   - Expected: the whole chain (identity → registered → node online → wallet →
     payout → presence → Tassadar → claimed → earned) is visible completing on
     screen, with a Retry affordance shown on any transient failure (never a
     dead/blank state).
   - Evidence: the completed-wizard screenshot.
     - Gate: *Full chain visible completing in the wizard.*

### Recording the result

When rerunning, append a dated "From-DMG proof" section to this file with the
source commit, the DMG release tag + digest, and the public-safe evidence
refs/screenshots for each gate above.

---

## Source references

- AO-6 smoke harness: `apps/autopilot-desktop/scripts/auto-onboarding-e2e-smoke.ts`.
- Phase-1 headless convergence proof (extended, not duplicated):
  `apps/autopilot-desktop/scripts/auto-onboarding-headless-proof.ts`.
- Black-screen guard: `apps/autopilot-desktop/tests/black-screen-guard.test.ts`;
  the fix it guards: commit `73cada159` (view/crashView must return a Document),
  surfaced in `apps/autopilot-desktop/src/ui/view.ts` (`export const view`) and
  `apps/autopilot-desktop/src/ui/main.ts` (`crashView`).
- AO-3 identity choice: `apps/autopilot-desktop/src/bun/identity-choice.ts`.
- AO-1/AO-2 onboarding: `apps/autopilot-desktop/src/bun/agent-onboarding.ts`,
  `apps/autopilot-desktop/src/bun/node-launcher.ts`.
- AO-4 wizard projection: `apps/autopilot-desktop/src/shared/onboarding-status.ts`;
  wired in `apps/autopilot-desktop/src/bun/index.ts`.
- Audit (the gap this closes): `docs/launch/2026-06-18-autopilot-desktop-availability-audit.md`.
- CS-B1 packaged-node proof (prior, narrower scope):
  `docs/launch/2026-06-18-autopilot-desktop-cs-b1-proof.md`.
- Release / signing: `docs/DEPLOYMENT.md`,
  `apps/oa-updates/docs/release-signing-runbook.md`.
