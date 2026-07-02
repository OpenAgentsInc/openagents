# After-Action: Fable WS-17 Desktop Fleet Closeout - 2026-07-02

## Context

On 2026-07-02 the owner narrowed the active Fable push to "fleet working in
desktop app ASAP." Mobile companion / push, T16 public-promise ops,
mobile-dependent AaaS, and GEPA optimizer work are postponed for this push.

PR #8004 recorded that trim in the roadmap. PR #8006 shipped T9.7 Claude
closeout token diagnostics so every completed Claude assignment either reports
exact usage or produces a typed accounting blocker. This closeout covers the
remaining WS-17 issues (#7905, #7906, #7907) under the revised scope.

## Evidence

- `khala fleet status` reported three ready Codex registry accounts:
  `codex-2`, `codex-b7d4438c`, and `codex-dbbb1972`.
- `pylon presence heartbeat --base-url https://openagents.com --json` reported
  `registered=true`, `linked=true`, and `stale=false` for
  `pylon.33afd48282a649047e3a`, with no blocker refs.
- `bun run --cwd apps/pylon smoke:fleet-run-live` returned the intended
  skip-safe unarmed result. No dispatch and no spend occurred.
- `khala fleet status --live` showed the live watchdog healthy with zero active,
  ready, busy, or queued live slots and `activeAssignmentCount=0`.
- A no-dispatch dry-run over the actionable desktop issues
  `#7956,#7955,#7953,#7931` resolved three target slots across the three ready
  Codex accounts:

```sh
khala fleet run --repo OpenAgentsInc/openagents \
  --issues 7956,7955,7953,7931 \
  --commit 84880c527bcd38d2ed1ae3f821c2ed78e660b30a \
  --verify "bun run check:deploy" \
  --dry-run
```

## Decision

T17.1 is complete for the current desktop-fleet push. The fleet has enough
ready local Codex accounts, a fresh hosted heartbeat, skip-safe live-smoke
behavior, and dry-run slot planning to continue desktop Fleet work honestly.

T17.2, the old clean-2B-day acceptance run, is not run and is not counted as
complete for live throughput. After the owner trim there were only nine open
issues total, with four actionable desktop issues in the dry-run set. Running a
target-15/18 overnight window would either fabricate work, reopen postponed
scope, or spend owner account budget without a real backlog. That gate is
postponed until the backlog and owner approval exist again.

T17.3 is complete for the current push through this after-action, the roadmap
revision, the fleet-management spec update, the cockpit runbook update, and the
FleetRun fan-out doc update.

## Regressions And Fixtures

No live fleet regression was observed because no armed run was started. The
important regression class is an acceptance-bound mismatch: a "clean 2B day"
gate must fail closed when the claimable backlog is below the configured floor.

Future first-class acceptance code should expose this typed blocker:

```txt
blocker.fleet_run_acceptance.claimable_units_below_floor
```

The current checked fixture is the active replenishment path: `khala fleet run`
and the legacy supervisor replenishment templates no longer seed GEPA/DSPy
optimizer work during desktop-fleet lockout recovery. Replenishment remains
bounded to desktop-fleet readiness, dispatch waste, and focused test/lint/type
coverage work.

## Next Live Gate

Revive T17.2 only when all of these are true:

- At least 30 real, claimable, non-duplicate work units exist.
- `smoke:fleet-run-live` is explicitly armed and green the same day.
- The owner approves the overnight spend/refill window.
- The run starts from one chat/Fleet action at target 15-18.
- Exact token rows, closeouts, claim refs, duplicate-PR checks, and merge
  verification are the acceptance evidence. Public counter movement alone is
  not evidence.
