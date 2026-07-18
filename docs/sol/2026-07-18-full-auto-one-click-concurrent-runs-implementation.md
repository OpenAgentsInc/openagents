# Full Auto: one-click launcher, concurrent runs, and operator monitor

Date: 2026-07-18
Status: implemented; real-system verification in progress
Authority: `specs/desktop/full-auto.product-spec.md` rev 13

## Decision

Keep the current Effect-owned Desktop runtime and provider adapters. Do not
replace this path with the Vercel AI SDK for the present sprint. The real
Codex/Claude system already has the hard parts that an AI SDK rewrite would
need to rebuild: native authenticated provider sessions, durable per-thread
leases, restart recovery, workspace authority, typed routing and rotation,
liveness, reports, and runRef-scoped control routes. The fastest honest path
is to remove the product bottlenecks around that proven runtime.

## Owner-reviewed problems

The previous launcher exposed nine fields as primary UI and allowed the
objective and done-condition textareas to consume most of the viewport. The
running view repeated the full mission above the conversation. Opening Full
Auto while one run was active redirected to that run, and both the registry
and start action rejected another run profile-wide. There was no persistent
catalog from which an owner could monitor and stop an arbitrary run.

The 2026-07-18 real owner-profile run also showed that provider turns can be
healthy for minutes at a time. A running state with an unchanged attempt
counter is not itself a stall; liveness continues to use the durable journal
and existing watchdog instead of UI polling heuristics.

## Implemented contract

- The default launcher asks for one bounded mission prompt.
- A blank advanced title is deterministically inferred from the mission's
  first line. A blank done condition resolves to: complete the objective, run
  relevant verification, and report the result or a concrete blocker.
- The default owner-admitted routing policy is Codex then Claude. The main
  process still validates both lanes fail-closed before it creates anything.
- Title, done condition, workspace, primary provider, model, fallback order,
  turn cap, and wall-clock guardrail live in a collapsed **Advanced** section.
- Objective and done-condition textareas have bounded initial and maximum
  heights; resizing is explicit rather than layout-consuming.
- Up to eight non-terminal `FullAutoRun` records may coexist. A ninth start
  refuses before minting a thread. Each admitted run has a distinct
  `runRef` and `threadRef`; lifecycle, liveness, report, Pause/Resume/Stop, and
  handoff stay scoped to the exact run.
- The existing durable per-thread lease still permits at most one Full Auto
  turn on any given thread. Distinct run threads can be admitted and in flight
  independently; no global lifecycle boolean is introduced.
- A persistent run monitor lists every active run plus recent terminal runs,
  refreshes from main, opens an exact run, and stops any active run by
  `runRef`. Starting a new run never hides or mutates existing ones.
- Agent control is the same authenticated loopback surface used before:
  `GET /v1/full-auto/runs` monitors all runs and
  `POST /v1/full-auto/runs/{runRef}/stop` cancels exactly one. CLI equivalents
  are `full-auto runs` and `full-auto run-stop <runRef>`; MCP remains a thin
  client of the same routes.

## Safety and invariant change

Rev 13 removes only the profile-wide active slot. It does not relax workspace
binding, provider admission, per-thread exactly-once dispatch, failure budgets,
turn caps, or terminal transition rules. `INVARIANTS.md` now names the admitted
concurrent-run boundary and still excludes cross-machine admission,
fleet/multi-repository scheduling, and provider selection outside an
owner-admitted ordered policy.

## Verification matrix

| Requirement | Primary evidence |
| --- | --- |
| One prompt starts a run | launcher validation/handler and React DOM tests |
| Advanced hidden by default | React DOM `details.open === false` test |
| Codex → Claude default | launcher validation test for ordered routing policy |
| Multiple active runs | registry and HTTP control-server two-run tests |
| Monitor every active run | React DOM monitor test |
| Stop arbitrary run | runRef intent + HTTP/CLI stop route |
| No duplicate turn per run | retained per-thread lease/reconciliation suite |
| Real provider execution | owner-profile Full Auto run receipt and follow-up run |

## Real-system run protocol

1. Build/relaunch OpenAgents Desktop with the control surface enabled.
2. Start one mission through the compact launcher and a second through the
   authenticated run-start route while the first remains non-terminal.
3. Confirm `runs` reports two distinct active run/thread identities.
4. Observe each run through at least one accepted real provider turn.
5. Stop one run from the UI monitor and the other through the control API;
   confirm their terminal transitions are independent.
6. Start a normal Codex→Claude Full Auto mission and retain the bounded report
   and public-safe receipt. Inspect raw provider evidence only in owner-private
   storage and never copy transcript content into this document.

## Follow-on exclusions

This is not a fleet scheduler, a queue, or an unbounded provider fan-out
system. Resource-policy tuning can add an explicit typed capacity limit later
without restoring a single global run slot. A future Vercel AI SDK or Effect AI
harness can still be evaluated for new provider integrations, but replacing a
working durable runtime is not on the critical path to Full Auto now.
