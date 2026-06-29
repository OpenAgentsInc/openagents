# Pylon v0.3 Release Cluster Promise Audit

Date: 2026-06-10

Registry version at audit time: `2026-06-10.4`

Status: full status audit of the five-promise Pylon release cluster and the
implementation issue plan to take all five to green. This is the second epic
in the get-to-green campaign, following the five-streams epic
(`2026-06-10-five-bitcoin-revenue-streams-promise-audit.md`, issues
#4635–#4653). The **Delegation Contract** in that audit is binding for every
issue below and is inlined in each filed issue body.

## The Cluster

Five promises share blockers and evidence surfaces, so they move as one
coordinated push (wave 5 of the campaign trajectory):

| Promise | State | Blockers |
|---|---|---|
| `pylon.v03_release_candidate.v1` | yellow | `pylon_v03_stable_release_not_green`, `pylon_v03_live_network_smokes_incomplete` |
| `pylon.release_tomorrow.v1` | yellow | `pylon_v03_stable_release_not_green` (shared); Windows/WSL deliberately out of scope by owner decision in version `2026-06-10.26` |
| `pylon.install_without_wallet_knowledge.v1` | **red** | `mdk_send_readiness_not_proven_for_restore`, `live_install_to_bitcoin_smoke_incomplete` |
| `pylon.no_dark_capacity_accounting.v1` | green | Cleared by #4659 provider lifecycle accounting, #4660 retained two-day history, and receipt `promise_transition_cd1c3145-eccd-4985-b48a-99f8b1b20fbe` |
| `autopilot.codex_probe_pylon_successor.v1` | yellow | `live_probe_pylon_runtime_gates_incomplete` |

The remaining shared blocker `pylon_v03_stable_release_not_green` still clears
two release promises at once; `pylon.no_dark_capacity_accounting.v1` has moved
out of the blocked set.

## Live Evidence Snapshot (2026-06-10)

- `apps/pylon` is `@openagentsinc/pylon@0.3.0-rc1` (Bun/Effect/OpenTUI) with
  a real local release gate: `bun run release:gate` runs unit/runtime tests,
  bootstrap/status/inventory/operator JSON smokes, dashboard startup smoke,
  package dry-run, and a local package-install smoke
  (`apps/pylon/docs/launch-gates-no-overclaim.md`).
- The local macOS install smoke exists (`bun run smoke:install:local`); the
  Linux equivalent is documented for `ubuntu-latest` but **not wired into
  CI** because the GitHub token in use lacks workflow scope
  (`apps/pylon/docs/release-install-smokes.md`). Windows and WSL are
  untouched.
- The install-to-bitcoin smoke contract already exists in the worker
  (`workers/api/src/pylon-install-to-bitcoin-smoke.ts`) with three modes:
  `ci_no_spend`, `sandbox_fake_payment`, and `live_small_sats`. The live
  mode requires an operator approval ref, a spend cap, and **original funded
  wallet-home mode, not mnemonic-only restore** — encoding the MDK restore
  gap directly into the gate.
- The MDK wallet readiness classifier, CLI wrappers, payout-target admission
  refs, and local ledger events are implemented
  (`apps/pylon/docs/mdk-wallet-readiness-ledger.md`).
- `GET /api/public/pylon-stats`: 0 Pylons online / wallet-ready / sellable;
  19 registered; `0.3.0` and `0.3.0-rc1` clients seen in the last 24h;
  2,323 sats of receipted accepted-work payouts.
- `GET /api/public/pylon-capacity-funnel` (live): 19 registered, **19 dark**
  (4 never heartbeated, 15 stale heartbeat). The funnel works; what is
  missing for green is provider job-lifecycle records and retained
  historical snapshots.
- The #4633 live no-spend loop proved the production assignment rail end to
  end, with the recorded caveat that the worker loop executed a
  validation-class task driven by the agent itself, **not a repo-checkout
  task executed by the packaged Pylon binary** — that caveat is exactly the
  `live_probe_pylon_runtime_gates_incomplete` blocker.

## Relationship To The Five-Streams Epic (#4635–#4653)

Four five-streams issues already generate evidence this cluster needs — the
cluster issues reference them rather than duplicating:

- **#4638** (GO ONLINE provider loop) and **#4641** (paid compute smoke)
  produce the online/wallet-ready presence and real job activity that the
  network smokes and the capacity funnel count.
- **#4642** (v0.3 live GEPA endpoint smoke + paid settlement) covers the
  GEPA slice of `pylon_v03_live_network_smokes_incomplete`; the cluster's
  network-smoke issue scopes to the non-GEPA surfaces (registration,
  heartbeat, wallet readiness, payout-target admission, presence).
- **#4647** (labor intake on the contributor's own agent) overlaps the
  packaged-binary runtime-gates issue; the two coordinate on the runtime
  contracts surface.

## Promise-By-Promise Status And Path

### `pylon.v03_release_candidate.v1` (yellow)

Live: the rc exists in-repo with a passing local release gate. Missing:
(a) stable `0.3.0` published with the gate green and the release record
written; (b) live network smokes from the packaged binary against
production. Path: issues 3, 1, then 9 below.

### `pylon.release_tomorrow.v1` (yellow)

Live: rc release exists; macOS/Linux are the declared launch platforms.
Missing: stable release (shared blocker) and the Windows/WSL question
answered honestly — proven, or explicitly documented as unsupported with
the promise copy narrowed to match. The claim is "a new version of Pylon
releases", so an honest platform matrix plus stable 0.3.0 can green this
without universal-platform proof, provided the copy never implies
universality. Path: issues 2 and 9.

### `pylon.install_without_wallet_knowledge.v1` (red)

The flagship contributor claim. Live: wallet readiness classification,
payout-target admission, the smoke contract with the `live_small_sats`
gate. Missing: (a) the MDK restore send-readiness question resolved —
either proven or converted into an explicit, detectable wallet-home
preservation rule with a send-readiness preflight; (b) one full live
install-to-bitcoin run on a real machine with operator approval, spend cap,
payment receipt, settlement receipt, and public projection. Path: issues 4
then 5.

### `pylon.no_dark_capacity_accounting.v1` (green)

Live: the public funnel with the full dark-capacity taxonomy, provider
job-lifecycle-backed stage counts from #4659, and retained count-only hourly
and daily history from #4660. The green transition cited receipt
`promise_transition_cd1c3145-eccd-4985-b48a-99f8b1b20fbe` after the live
history route showed buckets for both 2026-06-10 and 2026-06-11. The boundary
is unchanged: capacity presence is not accepted work, payment, settlement, or
withdrawal evidence.

### `autopilot.codex_probe_pylon_successor.v1` (yellow)

Live: the #4633 end-to-end production loop, promiseRef linkage, the
Probe-runtime port into `apps/pylon/packages/runtime`. Missing: the packaged
binary executing a real Codex-backed task (repo checkout, bounded sandbox)
through the live assignment lifecycle — richer real execution, not
agent-driven validation tasks. Path: issue 8.

## Implementation Plan: GitHub Issues

**Filed on GitHub 2026-06-10 as #4654–#4663**, one per plan step in order
(plan 1 = #4654 … plan 10 = #4663). Same conventions as the five-streams
epic: each body is
self-contained, inlines the Delegation Contract, names its lane, surfaces,
and dependencies. Owner posture is default-yes (recorded 2026-06-10):
agents decide and solicit input post-hoc; escalate only for funding/spend
enablement or material policy deviations.

**Issue 1 — `pylon: wire the release gate and Linux install smoke into CI`**

> The release gate (`bun run release:gate`) and the Linux install smoke are
> documented but not in CI because the GitHub token in use lacks workflow
> scope. Add `.github/workflows/` coverage running `bun install`,
> `bun run test`, `bun pm pack --dry-run`, and `bun run smoke:install:local`
> on `ubuntu-latest` and `macos-latest` for `apps/pylon` changes. If the
> executing agent's token cannot create workflow files, produce the exact
> workflow file in a normal commit path and flag the operator to move it —
> that operator step is the only Lane B element here. Acceptance: CI runs
> green on both platforms for an `apps/pylon` PR; release-install-smokes doc
> updated to point at CI instead of "documented only".

**Issue 2 — `pylon: Windows and WSL install smokes and an honest platform matrix`**

> `native_windows_wsl_unproven` blocks `pylon.release_tomorrow.v1`. Run the
> bootstrap/install smoke on WSL Ubuntu and native Windows (Bun supports
> Windows; the OpenTUI dashboard may not). Outcome is evidence either way:
> a passing smoke proves the platform; a failing one gets documented in a
> support matrix (`apps/pylon/docs/platform-support.md`) with the failure
> class, and the promise copy stays narrowed to the proven set. Tailnet
> machines can host the WSL run; flag the operator if no Windows host is
> reachable. Acceptance: platform matrix doc with smoke evidence per
> platform; registry transition proposing the blocker clear (proof) or an
> honest re-scope (documented unsupported), receipt-first either way.

**Issue 3 — `pylon: live v0.3 network smoke from the packaged binary (non-GEPA surfaces)`**

> Scope: the packaged `@openagentsinc/pylon` binary, installed fresh, runs
> register → heartbeat → wallet readiness → payout-target admission against
> production, and shows up in `GET /api/public/pylon-stats` and the capacity
> funnel as a live, non-dark Pylon. #4642 owns the GEPA assignment slice;
> this issue owns everything the network needs before assignments. Keep the
> smoke repeatable (`bun run smoke:...` entry) and document it in
> `apps/pylon/docs/`. Acceptance: one real machine registered via the
> packaged binary, heartbeating fresh, wallet-ready in public stats;
> transition receipt proposing the `pylon_v03_live_network_smokes_incomplete`
> clear (jointly evidenced with #4642).

**Issue 4 — `pylon: MDK restore send-readiness proof, preflight, and upstream report`**

> `mdk_send_readiness_not_proven_for_restore` is the recorded MDK gap:
> mnemonic-only restore showed positive balance but zero outbound capacity.
> Three deliverables: (a) reproduce the restore case and the original
> wallet-home case side by side, documenting both; (b) implement a
> send-readiness preflight in `pylon wallet status` so the state is
> detectable before any spend attempt (receive-ready ≠ send-ready is already
> the classifier's law); (c) file the public-safe findings upstream with the
> MDK author. The blocker clears either by proof that restore preserves
> outbound capacity or by converting the rule into explicit, detectable
> operator guidance (wallet-home preservation) enforced by the preflight —
> the install-to-bitcoin smoke contract already requires original
> wallet-home mode for live runs, so the honest re-scope path is fully
> consistent. No wallet material in any artifact. Acceptance: side-by-side
> evidence doc, preflight shipped with tests, upstream report ref, blocker
> transition proposed receipt-first.

**Issue 5 — `pylon: live install-to-bitcoin smoke (live_small_sats) on a real machine`**

> The cluster capstone (Lane B). Run the existing smoke contract
> (`pylon-install-to-bitcoin-smoke.ts`) in `live_small_sats` mode end to
> end: fresh install of the packaged binary on a real machine → registration
> → heartbeat → MDK wallet readiness → payout-target admission → assignment
> lease → accepted-work closeout → payment receipt → public settlement
> receipt → public projection. Requires: operator approval ref, spend cap,
> funded original wallet-home (per issue 4). Clears
> `live_install_to_bitcoin_smoke_incomplete`; with issue 4 it takes
> `pylon.install_without_wallet_knowledge.v1` red → yellow (green when the
> path is self-serve rather than operator-staged — say which in the
> transition). Acceptance: full ref bundle for every stage, public receipt,
> transition receipt-first, registry bumped and deployed.

**Issue 6 — `worker: provider job-lifecycle records feeding the capacity funnel`**

> `provider_job_lifecycle_missing` blocks
> `pylon.no_dark_capacity_accounting.v1`. The funnel reason-codes dark
> capacity but the assigned/running/artifact-producing/accepted stages have
> no durable per-provider job-lifecycle records behind them. Persist
> lifecycle events (offered, accepted, running, artifact_submitted,
> closeout, accepted_work) keyed to registered Pylons — the five-streams
> waves (#4638–#4648) and #4642 generate the real activity — and project
> stage counts from those records instead of inference. Counts only, no
> device identifiers (the funnel's existing caveat). All multi-statement
> writes via `db.batch`. Acceptance: funnel stages backed by queryable
> lifecycle records for real Pylons; tests cover every stage transition;
> deployed.

Closeout: #4659 is deployed and closed; lifecycle-backed stage accounting is
now part of the public count-only funnel.

**Issue 7 — `worker: retained capacity-funnel history snapshots`**

> `capacity_funnel_snapshots_missing`: the funnel is live-only. Add a
> scheduled snapshot (the worker already has a `* * * * *` cron — piggyback
> at a sane interval, e.g. hourly rows + daily rollup) persisting funnel
> counts to D1, and a public history endpoint
> (`GET /api/public/pylon-capacity-funnel/history`) returning the retained
> series, counts only. With issue 6 this completes the promise's green
> requirements; propose the yellow → green transition receipt-first.
> Acceptance: snapshots retained across days, public history route live,
> OpenAPI + AGENTS.md surfaces updated (mirror + sha pin), deployed.

Closeout: #4660 reached two real days of retained live buckets on
2026-06-11, then recorded receipt
`promise_transition_cd1c3145-eccd-4985-b48a-99f8b1b20fbe`; registry version
`2026-06-10.27` marks `pylon.no_dark_capacity_accounting.v1` green.

**Issue 8 — `pylon: packaged-binary real-task runtime smoke (Codex-backed, bounded sandbox)`**

> `live_probe_pylon_runtime_gates_incomplete`: the #4633 loop was driven by
> the agent, not the packaged binary, and ran a validation-class task. Run a
> real Codex-backed task — repo checkout, bounded working directory, an
> actual change with verifiable output — executed **by the installed
> `pylon` binary's worker loop** through the live assignment lifecycle.
> Coordinate with #4647 (labor intake) on the runtime-contracts surface:
> #4647 owns the labor-market job contract; this issue owns the
> packaged-binary execution evidence for the successor promise. No-spend is
> acceptable; the deliverable is runtime-gate evidence, not payment.
> Acceptance: live assignment executed by the packaged binary with
> artifact/proof refs, runbook in `apps/pylon/docs/`, transition proposed
> for `autopilot.codex_probe_pylon_successor.v1` yellow → green.

**Issue 9 — `pylon: stable 0.3.0 release with gate evidence and registry flips`**

> The shared-blocker closer. When issues 1–3 are green: flip the package to
> `0.3.0`, run the full release gate, publish `@openagentsinc/pylon@0.3.0`
> (npm publish auth is an operator step — flag when ready), write the
> release record in `apps/pylon/docs/`, update `launch-gates-no-overclaim.md`
> allowed copy, and propose the `pylon_v03_stable_release_not_green` clear —
> which moves **both** `pylon.v03_release_candidate.v1` and
> `pylon.release_tomorrow.v1` (the latter also needs issue 2's platform
> answer). Transitions receipt-first, one registry bump per promise flip,
> serialized with other registry-touching issues. Acceptance: stable package
> installable from npm, release record + gate evidence committed, both
> promises moved with receipts, deployed.

**Issue 10 — `cluster: release-cluster verification sweep and green proposals`**

> The wrap issue, analogous to #4652 for five-streams. When issues 1–9 have
> landed: re-verify every blocker in the cluster against live state in one
> pass (stats, funnel + history, receipts, release record, platform matrix,
> runtime smoke), propose the remaining transitions receipt-first, and post
> the cluster wrap-up in the product-promises Forum with the full evidence
> map. Nothing in this issue builds; it verifies, records, and proposes.
> Acceptance: every cluster promise either green or carrying an honest
> named remainder; `lastVerifiedAt` populated for all five; Forum wrap-up
> posted.

**Lane map (plan step → issue → lane → primary surfaces → depends on):**

| Plan | Issue | Lane | Primary surfaces | Depends on |
|------|-------|------|------------------|-----------|
| 1 | #4654 | A (operator moves workflow file if token lacks scope) | NEW `.github/workflows/`, `apps/pylon/docs/release-install-smokes.md` | — |
| 2 | #4655 | B (needs Windows/WSL host access) | NEW `apps/pylon/docs/platform-support.md`, smoke scripts | — |
| 3 | #4656 | A | `apps/pylon` smoke entries + docs | coordinates with #4638/#4642 |
| 4 | #4657 | A (small-sats repro is operator-funded) | `apps/pylon/src/wallet.ts`, NEW evidence doc | — |
| 5 | #4658 | B (operator approval + funded wallet) | smoke scripts, registry files | #4656, #4657 |
| 6 | #4659 | A | worker funnel/lifecycle modules | activity from #4638–#4648 |
| 7 | #4660 | A | worker cron + NEW history route, OpenAPI, AGENTS.md mirrors | #4659 |
| 8 | #4661 | A (no-spend) | `apps/pylon` worker loop + runtime contracts | coordinates with #4647 |
| 9 | #4662 | B (npm publish auth) | `apps/pylon` package/version/docs, registry files | #4654, #4655, #4656 |
| 10 | #4663 | B (verification + Forum post) | registry files, Forum | #4654–#4662 |

Registry-touching issues (5, 7, 9, 10) serialize their registry commits with
each other and with the five-streams registry issues (#4641/#4645/#4648/
#4651/#4652).

## Expected Registry Motion

- `pylon.no_dark_capacity_accounting.v1` green in version `2026-06-10.27`
  (issues 6–7).
- `autopilot.codex_probe_pylon_successor.v1` yellow → green (issue 8).
- `pylon.v03_release_candidate.v1` yellow → green (issues 1, 3, 9).
- `pylon.release_tomorrow.v1` yellow → green (issues 2, 9) — with copy
  narrowed to the proven platform matrix if Windows/WSL stays unproven.
- `pylon.install_without_wallet_knowledge.v1` red → yellow (issues 4–5),
  green when the install-to-bitcoin path runs self-serve without operator
  staging — that final step likely lands with the five-streams stacking
  work (#4652) since both need a continuously online contributor machine.

Side effect: issue 4's send-readiness work clears
`mdk_agent_wallet_send_readiness_insufficient_capacity` framing on
`payments.money_dev_kit.v1` or re-scopes it honestly — propose that
transition in the same pass.

## Evidence Reviewed

- Registry records for all five promises (`product-promises.ts`,
  version 2026-06-10.4)
- `apps/pylon/package.json` (0.3.0-rc1), `apps/pylon/src/` modules,
  `apps/pylon/docs/launch-gates-no-overclaim.md`,
  `apps/pylon/docs/release-install-smokes.md`,
  `apps/pylon/docs/mdk-wallet-readiness-ledger.md`,
  `apps/pylon/docs/2026-06-09-pylon-v0.3-launch-promise-reconfiguration-audit.md`
- `apps/openagents.com/docs/2026-06-08-pylon-install-to-bitcoin-launch-smoke.md`
  (+ the worker smoke contract it documents)
- Live: `GET /api/public/pylon-stats` (0 online / 19 registered, v0.3
  clients seen 24h), `GET /api/public/pylon-capacity-funnel` (19/19 dark:
  4 never heartbeated, 15 stale)
- Five-streams epic: `2026-06-10-five-bitcoin-revenue-streams-promise-audit.md`
  and issues #4635–#4653 (#4638/#4641/#4642/#4647 overlap this cluster)
- #4633 closeout evidence (live no-spend loop and its packaged-binary caveat)
