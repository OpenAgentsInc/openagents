# Omega Full Auto proof matrix (FA-07)

- Date: 2026-07-24
- Packet: `OMEGA-FA-07`
- Omega issue: [OpenAgentsInc/omega#26](https://github.com/OpenAgentsInc/omega/issues/26)
- Component: `omega-effectd-v0.1.0-rc.6`
- Component source: `5bb31ac857b917b14c6455a7df268825cfbf773f`
- Component archive SHA-256: `b55f703229ff9299923a84b0843f9c926fbd75b08e787f5d6e79744fd114c836`
- Component manifest SHA-256: `13f0e094c5d120426f4ede3afedd24f04abec71e29abcd7700c0fd2e36037953`
- Omega runtime pin: `0ed675bcdf81973d946007d27188da7160d7e17a`
- Freeze: [2026-07-24-full-auto-contract-freeze.md](./2026-07-24-full-auto-contract-freeze.md)
- Assurance: `specs/omega/full-auto.assurance-spec.md` revision 5 is admitted
- Admission receipt: `authority.decision.1954518244492185756509b3cfec6e3e`

## 2026-07-25 rc.8 preparation pass (lane B1)

Wave 0 lane B1 of the
[master delegation plan](./2026-07-25-omega-master-delegation-plan.md) is
preparation only: keep the proofs current against
`omega-effectd-v0.1.0-rc.8`, and do **not** claim the owner-real run. This
section records what that pass drove to green and what it deliberately left
unperformed. Nothing below is an installed-candidate observation, an owner
observation, or a release verdict.

### Engine identity under test

The gates below were driven against the real supervised engine, not fixtures.
The Full Auto engine core is byte-identical to the rc.8 release:

| Check | Result |
| --- | --- |
| rc.8 tag `omega-effectd-v0.1.0-rc.8` resolves to | `509ae747f00f6f7ebb413809ff5bd6ea123e1c1c` |
| `git diff rc.8..main -- packages/omega-effectd/src/engine/` | empty |
| Reconciliation / host-evidence path in `src/protocol/server.ts` | unchanged from rc.8 |
| rc.8 archive SHA-256 pinned by `script/bundle-omega-rc` | `01d11597b054d009296d0381b6cd6ed3d31c83b93e75a58845f2ae47bf33226a` |

Drift from rc.8 on `main` is confined to the Sarah host-contract surfaces
(`sarah-host-contract.ts`, `framed.ts`, `host-bridge.ts`, and the Sarah
branches of `server.ts`) landed by the workroom lane. No FA-07 gate below
depends on those.

The collector and installed-proof scripts were re-checked and are already
current against rc.8. They required no change:

- `script/generate-omega-full-auto-candidate-evidence --self-test` — passed
- `script/collect-omega-full-auto-installed-evidence --self-test` — passed
- `script/prove-omega-rc-install --harness-check` — harness OK
- `script/bundle-omega-rc` and `script/prove-omega-rc-install` already pin the
  rc.8 tag and archive digest

### Gate 1 — incident replay, and its falsification

Gate 1 previously rested on a test that reproduced the incident by truncating
the Full Auto registry file. That asserts the classifier can label an
already-broken state. It does not exercise the composition that failed. The
2026-07-17 audit §2.3 is explicit that "a dedicated replay must recreate
thread pressure while a real Full Auto run advances through multiple turns".

`packages/omega-effectd/src/protocol/server.fa07-incident-replay.test.ts`
now does that. It drives the real framed protocol against a host that models
Omega's bounded five-slot mutable thread cache **including its eviction
policy**, and runs two arms over the same engine:

| Arm | Host eviction policy | Meaning | Required outcome | Observed |
| --- | --- | --- | --- | --- |
| A | `creation_time` | the 2026-07-17 defect | typed stall | `stalled` / `host_thread_missing` / `stop_only` |
| B | `last_access_lru` | the `8cb900bbf9` fix | continued autonomy | further continuation dispatched, no stall |

Both arms assert one property — **never silent death** — defined as: the run
must never remain `running` while nothing was dispatched, nothing failed, and
no stall cause is set. That conjunction is exactly what the owner experienced
for six hours. A typed stall passes. Continued autonomy passes. Silence fails.

The replay was then deliberately falsified twice against a regressed engine,
and watched fail:

| Experiment | Regression | Replay result |
| --- | --- | --- |
| 1 | delete the `!evidence.present` typed-stall guard in `server.ts` | Arm A red: `expected 'retrying' to be 'stalled'` — the failure is still budgeted, but the typed cause and the `stop_only` affordance are lost |
| 2 | additionally swallow host dispatch refusals | Arm A red on `assertNeverSilentDeath` itself, reporting a `running` run with `failedAttempts: 0` and `stallCause: null` |

Both regressions were reverted. The engine tree is unmodified.

### Gate 8 — restated 2026-07-25, and the Desktop escalation path removed

**The gate now reads:** "No model-initiated path can start Full Auto
authority. Only an explicit human action can, wherever that action lives."

The previous row 9 claim ("green (code) — Dedicated panel only") named the
wrong variable. The surface a start comes from was never the safety-relevant
thing. **Who decided** always was. Under the owner's direction to fold Full
Auto into the Omega chat UI, the old wording would have forbidden the intended
product while still permitting the actual defect. Row 9 above is replaced
accordingly, and its test is caller identity.

The finding recorded below was **correct for the Omega GPUI host and wrong as
a general statement**, because the same engine is also embedded by OpenAgents
Desktop. The Desktop half has since been fixed by removal (owner choice (a),
2026-07-25), not by re-scoping:

- On Omega, the launcher's Start button is the only path that reaches
  `supervisor.start_run`. The agent panel's Full Auto menu entry dispatches
  `OpenLauncher` (navigation only), the workroom projection is read-only, and
  the relay action map fails closed on any action ref outside
  `full_auto.{pause,resume,stop}`.
- On OpenAgents Desktop, `apps/openagents-desktop/scripts/full-auto-mcp.ts`
  exposed `full_auto_start` and `full_auto_run_start` as **MCP tools a
  language model can call**, with no owner confirmation step. The MCP server
  reads the loopback bearer from the mode-0600 connection file itself, so the
  model never needed to hold a credential. This was gated by the
  `OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1` opt-in and by the owner having
  registered the MCP server — but that combination is the documented dogfood
  configuration. **REMOVED 2026-07-25.** `full_auto_enable` went with them:
  it grants Full Auto authority to a thread and schedules the first
  continuation, which is a start in substance.
- Separately, the retired composer toggle IPC (`CodexLocalFullAutoSetChannel`)
  was still registered in the Desktop main process and still exposed through
  the preload bridge, letting any renderer caller inherit owner authority
  without proving the call came from a control the owner pressed.
  **REMOVED 2026-07-25**, along with its preload bridge, its renderer intent,
  and the pre-thread sentinel that promoted a toggle onto a brand-new thread.

What the Desktop dogfood workflow lost: a model can no longer start, bootstrap,
or enable Full Auto, and can no longer choose the lane, account, turn cap, or
wall-clock budget an unattended run executes under (routing policy and
guardrails rode only on start/enable). A human still starts runs from the
Desktop launcher, from `scripts/full-auto-cli.ts`, from the loopback OpenAPI
route, and from the Omega chat surface.

Residuals that removal does not close, and that remain open on omega#26:

- Any process running as the user can read `full-auto/control.json` and POST
  to `/v1/full-auto/start` directly. That is an OS-level authority boundary,
  not an MCP one. A model with a shell tool is not stopped by this change.
- `full_auto_continue_now` remains model-callable. It cannot create a grant —
  reconciliation dispatches nothing for a record no human enabled — but it
  does trigger an immediate turn inside an existing grant.

The Desktop removal is pinned by
`apps/openagents-desktop/scripts/full-auto-mcp-tools.test.ts`, which drives
every registered tool through the real dispatcher against a recording control
client and asserts the control operation each one actually reaches. It is a
property over the registered surface, not a grep, so a renamed start fails it.

The engine-side half of the property is now pinned by
`packages/omega-effectd/src/protocol/server.fa07-chat-authority.test.ts`,
which asserts that `apply_control_intent` — the surface every mobile, relay,
cloud, and model-facing control path funnels into — refuses `start`,
`create`, `enable`, and casing/whitespace variants, cannot bring a run into
existence, and does not honour smuggled launcher arguments. Widening that
allowlist to admit `start` turns the test red.

### Gate 3 — restart

`packages/omega-effectd/src/protocol/server.fa07-restart.test.ts` drives a
real supervisor restart (a second service and framed server over the same
data root at a new generation) and asserts the run, its objective and done
condition, its native workspace binding, and its captured turn history all
survive with nothing dropped and nothing duplicated, that reconciliation
resumes afterwards, and that a stale-generation frame cannot mutate the
restored run.

### Verification run for this pass

- `packages/omega-effectd`: 221 tests across 27 files, green
- `pnpm typecheck` in that package: clean
- `pnpm run check` at the repository root: exit 0
- `pnpm run check:ste:all`: OK

### What this pass did NOT do

- **Gate 2 (owner-real multi-turn unattended run)** — reserved owner act, not
  performed and not simulated.
- **Gate 10 (independent assurance on the exact installed candidate)** —
  requires the designated reviewer identity. The producing lane cannot supply
  it.
- Gates 5, 6, and 7 remain source/automated only. No installed-candidate
  observation was produced, and no candidate was built or installed.
- The omega-side Rust restart test
  (`fa07_control_matrix_and_native_join_survive_restart`) was **not** re-run.
  A full Omega build was not started: another lane held an active `cargo`
  build on this host and the volume had reached 100% capacity during the
  session. Recorded as unperformed rather than substituted.

Every gate not driven to green above stays `pending_required_gates`.

## Result

This packet records the FA-07 proof matrix for Omega Full Auto.
It does **not** close `OMEGA-OA-05` release readiness.
It does **not** admit public claims.
The proof design has independent admission. Independent verification of the
exact installed candidate is still required.

## Assurance admission binding

Euler admitted Full Auto AssuranceSpec revision 5 as the
owner-designated independent reviewer. The durable receipt records four
executable criteria green (50 tests across four files) and leaves four criteria
unclassified and unobserved.

| Field | Exact binding |
| --- | --- |
| Original proposal digest | `sha256:b5b84098e820d0dd146b368f224ef7a10b107bf5b383ea2b6740c6d64b6bfc5f` |
| Current admitted document digest | `sha256:a612e2fe875c30b5346e81bc0b897312e5a12690fe0e2e1b56d2fa35ea10b7ee` |
| Receipt path | `docs/assurance/receipts/authority.decision.1954518244492185756509b3cfec6e3e.json` |
| Receipt digest | `sha256:9e58770c258833904396f294e0824a1235bbb6181e5843066703be69cfc387a2` |
| Admission commit | `d6794b73b034ff90c0b221c6a383920680186ac6` |

The spec names `openagents.assurance_reviewer` as verifier and
`openagents.owner` as release authority. Neither role has issued a verdict for
an exact installed rc.6 Omega candidate. Design admission does not satisfy the
candidate, owner-observation, release, or public-claim gates.

## Gate matrix (§12.2)

| # | Gate | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Dedicated launcher / no composer toggle | green (code) | `crates/full_auto_ui` product law and `full_auto_is_not_a_composer_mode_flag`. Agent menu opens `OpenLauncher` only. |
| 2 | Non-overridable guardrail immunity | green (automated) | FA-04 framed capacity and guardrail decode tests |
| 3 | Redaction by explicit field lists | green (automated) | FA-05/FA-07 receipt, attention, and `list_runs` redaction tests |
| 4 | 2026-07-17 eviction incident shape | green (automated) | Drop host thread registry yields `stalled` / `host_thread_missing` / `stop_only` in FA-04 and FA-07 matrix |
| 5 | Owner-real multi-turn unattended run on Omega | blocked | No evidence packet binds an owner-observed run to an exact signed rc.6 candidate. |
| 6 | Visible Codex to Claude handoff on Omega | partial | The framed service now exposes paused-only, live-lane-revalidated, durable handoff under #9215. Live sidebar proof is still required on the replacement packaged candidate. |
| 7 | Packaged restart reconciliation | partial | Supervisor restart and native evidence survive in source tests. No current installed-candidate receipt binds that proof to rc.6 and the landed Omega capabilities below. |
| 8 | Mobile pause/resume/stop typed outcomes | green (source/automated) | rc.6 carries the Sync/mobile transport and typed outcomes. Omega `6a8e287295b9d7dbd9cc7abe02685cfcaa3afbf8` supplies the native OpenAgents session boundary. The installed offline/Sync journey remains unobserved. |
| 9 | No MODEL-INITIATED path can start Full Auto authority (restated 2026-07-25) | green (code, both hosts) | Test is caller identity, not surface. Omega: launcher Start is the only path to `supervisor.start_run`, and `apply_control_intent` is pause/resume/stop only (`server.fa07-chat-authority.test.ts`). Desktop: `full_auto_start` / `full_auto_run_start` / `full_auto_enable` removed from the MCP tool surface, and the composer-toggle IPC removed. The registered surface is pinned by `apps/openagents-desktop/scripts/full-auto-mcp-tools.test.ts` and the renderer intent surface by `shell.test.ts`. A human start from a chat surface is now the intended product and does not violate this gate. |
| 10 | Independent assurance on exact candidate | blocked | Rev5 proof design is admitted, but `openagents.assurance_reviewer` has not verified an exact installed candidate and no release verdict exists. |

## Landed Omega demo capabilities

The following source capabilities are on Omega `main`. Their issue receipts
record focused source tests. They improve the candidate subject, but they are
not installed-candidate observations and do not change any blocked gate above.

| Issue | Omega commit | Landed capability | Evidence boundary |
| --- | --- | --- | --- |
| [#41](https://github.com/OpenAgentsInc/omega/issues/41) | `417ceb520fd06a645c352fd5298d67f4407b9df3` | Sarah's private workroom projects service-backed Full Auto rows with objective, lane, state, latest turn, exact record-derived unattended duration, and explicit terminal reason. | Issue receipt records 94 focused tests. No current signed installed-candidate observation. |
| [#42](https://github.com/OpenAgentsInc/omega/issues/42) | `a480f888f9bc472d3b18198e76b8545cdfc09b7c` | Full Auto shows a bounded provider-account roster, readiness/quota state, and exact account-to-lane mapping, with native Omega Agent authentication guidance. | Issue receipt records 45 focused tests. It does not prove the owner's real account roster on the candidate. |
| [#43](https://github.com/OpenAgentsInc/omega/issues/43) | `705c1fb70344cf9e10c0c1aed651f149c25a3297` | A selected run can render the ordered objective, turn, change, test, host verification, and authority-receipt chain through the shared bounded receipt-inspector grammar. | Issue receipt records 65 focused tests. No completed owner work unit has been independently opened on the candidate. |

## Automated verification

- `pnpm --filter @openagentsinc/omega-effectd test` — includes `server.fa07-proof.test.ts`
- `cargo test -p omega_effectd --lib` — includes `fa07_control_matrix_and_native_join_survive_restart`
- `script/bundle-omega-rc --dry-run` — release-record schema ok (no compile/sign)
- `script/prove-omega-rc-install --harness-check` — harness OK (not installed-app proof)

The commands above are the original FA-07 automated matrix. Later issue
receipts cited in this document are repository evidence pointers, not commands
rerun by this documentation correction and not candidate verdicts.

## Native Codex authentication

The supported Codex lane is Omega's registered `codex-acp` external agent. Omega starts that agent through its native Zed-derived ACP host, and Codex remains the single owner of its local configuration, authentication, and token rotation. A user who is already logged into Codex does not need to import or copy that session into an Omega language-model provider.

Omega commit `31efeaffbc` briefly implemented a token-copying import and was rejected during independent review because it would create two competing refresh-token stores. The replacement candidate reverts that import and keeps the owner-real journey on the existing ACP authority. This path does not depend on the legacy OpenAgents/Pylon account bridge.

## Owner blockers (smallest irreducible)

1. Produce and install a signed Omega RC candidate that binds rc.6, the current
   admitted rev5 AssuranceSpec, and the landed #41–#43 Omega commits.
2. Run one owner-real multi-turn unattended Full Auto journey on that exact
   candidate and retain candidate-bound receipts.
3. Capture one live Codex-to-Claude handoff with workroom, sidebar, report, and
   exact account-to-lane evidence.
4. Exercise restart, offline/Sync recovery, and mobile Pause/Resume/Stop on the
   installed candidate with typed outcomes.
5. Have `openagents.assurance_reviewer` independently verify the exact
   candidate against the admitted rev5 obligations. The producer cannot supply
   this verdict.
6. Record owner observation and acceptance. Then obtain an explicit
   `openagents.owner` release decision. Neither design admission nor source
   tests grant release or public-claim authority.

Until those gates land, `OMEGA-OA-05` stays open and Electron Full Auto remains
the rollback surface.

## Next

- Keep Electron Full Auto as dogfood/rollback
- Continue Omega Agent Computer sequence (`OMEGA-AC-00`…) without claiming Full Auto primary cutover
