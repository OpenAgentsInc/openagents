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
| 9 | No ordinary chat sets Full Auto authority | green (code) | Dedicated panel only. No composer Full Auto start path. |
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
