# OpenAgents Codex Workroom MVP closure receipt

- Date: 2026-07-13
- Tracking issue: [#8756](https://github.com/OpenAgentsInc/openagents/issues/8756)
- ProductSpec: revision 6, digest
  `fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1`
- Exact candidate: OpenAgents Desktop `0.1.0-rc.9`, source `c388bf7e10`
- Completion audit commit: `04553786ab`
- Result: implementation/evidence lane accepted for closure

## Owner acceptance

After RC9 was installed as `/Applications/OpenAgents.app` and reverified with
deep strict code signing and Apple staple validation, the product owner stated:

> I accept the installed ProductSpec-native Codex workroom journey and its
> read-only review boundary.

This acceptance is limited to the installed journey and read-only boundary. It
does not authorize publication, telemetry collection, public marketing
language, CUT-27 completion, or a concurrency-policy change.

## Close-rule disposition

| Requirement | Evidence | Disposition |
| --- | --- | --- |
| Exact ProductSpec validates | ProductSpec CLI and 95-test ProductSpec suite passed; revision-6 digest remained frozen | Passed |
| Every `CW-AC-01` through `CW-AC-18` criterion has its narrowest true disposition | [`RC9 completion audit`](./2026-07-13-openagents-codex-workroom-rc9-completion-audit.md) | Passed, 18/18 |
| Installed real-Codex journey passes without fallback | Signed/stapled RC9 completed all 12 required steps using the ordinary logged-in Codex session | Passed, 12/12 |
| Gaps and exceptions are explicit | Distinct fault corpus, AssuranceSpec read-only owner exception, conditional rollout gates, and no-Pylon boundary are recorded | Passed |
| Release receipt names exact public-safe refs | [`RC9 candidate receipt`](./2026-07-13-openagents-codex-workroom-rc9-candidate-receipt.md) contains source, artifact hashes, Apple submissions, proof totals, and lifecycle results | Passed |
| Product-owner acceptance | Exact statement above | Accepted |

The exact RC8-to-RC9 lifecycle passed all 11 update, interruption, downgrade,
rollback, diagnostics, uninstall, reinstall, and cleanup entries. The full
Desktop gate passed `1,163` tests with `0` failures plus the built Electron
smoke. The focused fault corpus passed `95` tests with `0` failures.

## Boundary after closure

- RC9 remains unpublished; no update feed or registry was mutated.
- No behavior, Eval, or promise registry is changed by this closure.
- Public workroom/companion language still requires approval before use.
- Telemetry/consent copy still requires approval before collection.
- Publishing Codex-only before CUT-27 still requires an explicit release
  decision and does not complete CUT-27 or its parents.
- Concurrency-policy adoption or expansion retains its separate dogfood gate.
- The owner-directed AssuranceSpec viewer remains read-only/proposal-only and
  does not count as #8756 evidence or activate the AssuranceSpec backlog.

With those boundaries preserved, #8756 can be honestly closed as completed.
