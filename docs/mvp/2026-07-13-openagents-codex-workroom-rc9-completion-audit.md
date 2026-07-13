# OpenAgents Codex Workroom RC9 completion audit

- Date: 2026-07-13
- Subject: ProductSpec revision 6, digest
  `fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1`
- Exact candidate: `0.1.0-rc.9` from `c388bf7e10`
- Candidate receipt:
  [`2026-07-13-openagents-codex-workroom-rc9-candidate-receipt.md`](./2026-07-13-openagents-codex-workroom-rc9-candidate-receipt.md)
- Implementation verdict: all 18 acceptance criteria are implemented and have
  passing evidence at their narrowest required proof level
- Closure verdict: owner-accepted implementation/evidence lane; not published

## Criterion matrix

| Criterion | Disposition | Narrow evidence |
| --- | --- | --- |
| `CW-AC-01` | Passed | Exact signed/notarized RC9 install and launch; package, fuse, runtime compatibility, and preflight oracles. |
| `CW-AC-02` | Passed | Exact installed proof uses the ordinary logged-in Codex session with isolated proof `CODEX_HOME`; settings and smoke oracles exclude Pylon/account-linking UI. |
| `CW-AC-03` | Passed | Desktop coding-catalog and WorkContext tests prove stable opaque refs independent of paths, ports, processes, machines, and provider thread IDs. |
| `CW-AC-04` | Passed | Exact installed ProductSpec open plus workroom validation tests for missing sections and missing/duplicate author-visible criterion IDs. |
| `CW-AC-05` | Passed | Workroom revision tests prove digest/revision display, diff preview, confirmation, revision bump, retained prior identity, and explicit reconciliation. |
| `CW-AC-06` | Passed | Exact accepted two-packet plan and native child packet; deterministic dependency, criterion mapping, duplicate/cycle refusal, and mutation-lease fencing. |
| `CW-AC-07` | Passed | Signed app contains the hash-pinned product-owned `productspec-work` skill; exact real turn uses its native app-server registration; absence/corruption/version mismatch fail explicitly. |
| `CW-AC-08` | Passed | Exact producer evidence is independently host-verified while owner disposition remains pending; tests refuse prose authority, self-verification, and authority-tool access. |
| `CW-AC-09` | Passed | Revision/digest mismatch tests stop dispatch, preserve active work identity, and require reconcile/supersede/cancel before new work. |
| `CW-AC-10` | Passed | Metadata-first and 100 MiB history oracles prove top-level-only, age-unbounded paging and stable selection/title/status/order through restart. |
| `CW-AC-11` | Passed | Exact real root Codex task was admitted before dispatch and terminalized once with typed text, plan, shell/tool, artifact, usage, and lifecycle evidence. |
| `CW-AC-12` | Passed | Command-contract, host-routing, native-menu, keyboard, palette, local-turn journal, retry, conflict, stop, steer, queue, question, approval, and plan-review oracles. |
| `CW-AC-13` | Passed | Exact child card opens the independent child transcript; graph/history tests retain parentage and prevent flattening, duplication, rerooting, and top-level leakage. |
| `CW-AC-14` | Passed | Workspace service tests prove bounded relative tree, status, selected exact diff, timeline correlation, visible revocation/conflict, and no general filesystem/Git authority. |
| `CW-AC-15` | Passed | Exact renderer and second-process restart restore without redispatch; handoff tests fence active work, preserve packet identity, and distinguish continuation, repository handoff, and gap recovery. |
| `CW-AC-16` | Passed | Focused 95-test fault corpus proves lost-ack replay, ordering/idempotence, durable cursor repair before live, stale-generation fencing, revocation, and distinct quota/rate/auth/policy states. |
| `CW-AC-17` | Passed | Diagnostics, Electron-boundary, renderer-control, content-projection, workspace, and privacy scans enforce the prohibited-data and prohibited-authority lists. |
| `CW-AC-18` | Passed | Exact RC9 installed 12-step real-Codex proof plus exact RC8→RC9 interruption/update/downgrade/rollback/diagnostics/uninstall/reinstall/cleanup journal. |

## Verification totals

- Full Desktop gate at source commit `c388bf7e10`: `1,163` pass, `39` retired
  out-of-scope UI skips, `0` fail, then built Electron smoke passed.
- Focused `CW-AC-16` cross-app/runtime/workspace/preflight corpus: `95` pass,
  `0` fail.
- Exact installed RC9 journey: `12/12` required steps passed.
- Exact RC8-to-RC9 release lifecycle: `11/11` journal entries passed.
- ProductSpec validation: required again after this audit is committed; the
  canonical spec content and digest are unchanged.

## What remains

Implementation is complete against the 18 frozen criteria. After installation
of the exact signed/stapled RC9 candidate, the product owner explicitly
accepted the installed ProductSpec-native journey and its read-only review
boundary. The separate
[`closure receipt`](./2026-07-13-openagents-codex-workroom-mvp-closure-receipt.md)
records the exact statement and close-rule disposition.

The other ProductSpec owner gates remain explicit but are conditional on later
actions rather than unconditional #8756 close blockers: public
workroom/companion language requires approval before use; telemetry/consent
copy requires approval before collection; publishing this Codex-only shape
before CUT-27 requires an explicit release decision; and raising or adopting
the initial concurrency ceiling requires the stated dogfood-governance
decision. Closing an unpublished implementation/evidence issue does not take
any of those actions.

This audit does not publish RC9, update registries, or authorize public
promises. With installed-journey acceptance recorded, #8756 can close while
rollout and governance actions retain their separate gates.
