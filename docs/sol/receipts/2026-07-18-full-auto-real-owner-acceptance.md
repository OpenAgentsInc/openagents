# Full Auto real owner acceptance and operator runbook

- Class: receipt
- Final disposition: implementation and owner-real development acceptance complete
- Snapshot: 2026-07-18
- Source snapshot: `3123d926a3`
- Build: `main-3123d926a3`
- Packaging mode: development
- Profile class: `owner_real`
- Owner: OpenAgents Desktop Full Auto / #8976, #9000, #9001, and #9002
- Public evidence:
  [`../evidence/2026-07-18-full-auto-real-owner-acceptance.json`](../evidence/2026-07-18-full-auto-real-owner-acceptance.json)

## Outcome

Full Auto works through the real OpenAgents Desktop system with actual Codex
and Claude provider lanes. The owner-real macOS batch completed all six named
acceptance rows and one additional automatic same-pass Claude-to-Codex
rotation. All six reviewed PASS rows remained visible after the thread-pressure
test and after the fresh-process rotation proof.

The implementation stayed on the existing Effect-owned Desktop runtime. The
Vercel AI SDK Harness fallback was not needed, no AI SDK renderer or second
scheduler was introduced, and no broad Vercel-to-Effect source conversion was
required.

This receipt closes the implementation and owner-real development proof rung.
It is not a packaged, signed, notarized, independently admitted, or public
release claim. #8978 still owns independent assurance and #8979 still owns the
signed packaged release/owner-observation gate; parent #8967 remains open until
those distinct obligations close.

## Candidate identity

| Field | Value |
| --- | --- |
| Source revision | `3123d926a3` |
| Build | `main-3123d926a3` |
| Environment | macOS arm64, development Electron build |
| Profile | normal owner-real Desktop profile |
| Codex lane | `codex-local`; `codex-app-server`; `codex-cli 0.145.0-alpha.18` |
| Claude lane | `fable-local`; `@anthropic-ai/claude-agent-sdk 0.3.172` |
| Telemetry | disabled |
| Started | `2026-07-18T09:55:40.552Z` |
| Ended | `2026-07-18T10:02:26.478Z` |
| Test definition | `77ab05baed3c3ab4974787b4c37d66720eae3eadfd90bcf5f5fea3e4935f8c78` |

The public receipt carries only schema identities, digests, provider runtime
versions, bounded dispositions, and the private-evidence pointer class. It
contains no prompts, responses, tool output, credentials, account identity,
workspace path, local evidence path, or provider-native session identifier.

## Acceptance results

| Test | Final sidebar title | What passed |
| --- | --- | --- |
| 01 | `PASS · TEST 01 · Codex → Claude · context` | One thread retained `ORBIT-17`, the prior result, and a visible owner UI transition from Codex to Claude. |
| 02 | `PASS · TEST 02 · Claude → Codex · context` | One thread retained `LANTERN-42`, the prior result, and a visible owner UI transition from Claude to Codex. |
| 03 | `PASS · TEST 03 · Codex → Claude · objective retention` | The durable objective and done condition reached the target lane across the handoff independently of transcript pressure. |
| 04 | `PASS · TEST 04 · Full Auto · Codex · 3 turns` | One unattended run completed three useful Codex turns with a bounded report and analyzer result. |
| 05 | `PASS · TEST 05 · Full Auto · Claude · restart` | One three-turn Claude run drained to Pause, survived complete Desktop quit/relaunch, retained its run fields, and completed without duplicate dispatch. |
| 06 | `PASS · TEST 06 · Full Auto · thread pressure` | Six ordinary chats created while the run was active did not evict the autonomous thread or duplicate the next continuation. |

Every row has a non-null SHA-256 report digest and analysis digest tied to the
candidate and schema identities. Every row's final failure classification is
null. The exact artifact, report, analysis, run/thread, and handoff digests are
in the linked JSON receipt.

The additional automatic proof began on `fable-local`, recorded a typed
`complete_within_bounds` rotation to `codex-local` in the same admitted pass,
and finished the exact target artifact. Its transition and artifact are bound
by public SHA-256 digests. This was a fresh-process proof, not a manual switch
or a fixture replay.

## What changed to make it work

The implementation range from `dee26f0861` through `3123d926a3` repaired the
production path instead of replacing it:

- every first, continued, rotated, resumed, and restart-recovered turn now
  receives a typed mission packet compiled from the durable objective and done
  condition;
- active run threads stay addressable under ordinary-chat pressure, while
  terminal runs release protected residency;
- host-thread failures and provider failures have distinct typed causes;
- provider error, rate limit, account exhaustion, and missing provider session
  remain inspectable and drive bounded recovery/rotation behavior;
- exact provider lane and model selections persist on the run and rehydrate
  through the corresponding chat row;
- Pause is a deterministic drain boundary; Stop is the interrupting terminal
  action; a provider switch is only legal from Paused;
- terminal Claude SDK results close their subprocesses and settle exactly
  once, including across relaunch;
- reviewed PASS, FAIL, and BLOCKED acceptance rows remain visible under thread
  pressure instead of being hidden by the ordinary composer cache limit; and
- owner-profile close/relaunch now waits for the macOS view service to settle
  and detects shell readiness from an always-present destination.

The existing `FullAutoRun`, journal, lease, routing, handoff, report, analyzer,
guardrail, and UI contracts remain the only product authority. Provider-native
sessions remain private implementation details.

## UI-first operator runbook

No terminal knowledge is required for an ordinary Full Auto run:

1. Open **Full Auto** in the Desktop left rail.
2. Enter a specific **Title**, **Objective**, and **Done condition**. Confirm
   the **Workspace** points at the intended project.
3. Choose the primary **Provider**. Optionally enter the exact **Model**, set a
   **Turn cap**, add one or more **Fallback lanes** in rotation order, and set a
   maximum wall-clock limit.
4. Select **Start**. The run view becomes read-only and shows the durable
   objective, done condition, workspace, provider, cap usage, state, canonical
   conversation, and per-turn outcomes.
5. To inspect without starting another turn, select **Pause**. Pause drains the
   current in-flight turn and then settles at **Paused**; it does not kill a
   provider midway through its result.
6. While Paused, select **Switch to Claude** or **Switch to Codex** if a manual
   handoff is wanted, then select **Resume**. The target receives the same
   durable mission and the bounded accepted handoff history.
7. If the run is Stalled and the cause is recoverable, select **Retry now**.
   Nonrecoverable stalls expose Stop rather than pretending a retry succeeded.
8. Select **Stop** only to end the run permanently. Stop is terminal and cannot
   be resumed.
9. At completion, review the explicit terminal reason, conversation, turn
   outcomes, report, and analyzer result. A cap exit is shown as cap reached,
   not as mission completion.

For acceptance review, retain failed or blocked sidebar rows with their prefix,
fix the linked defect, and create a separately identified rerun. Never delete a
failed row or turn a provider outage into a PASS.

## Verification

The frozen candidate passed the complete repository push gate before the
owner-real run:

- 252 test files: 2,535 passed and 39 skipped;
- Desktop production build;
- Electron smoke and React smoke;
- lint, generated-protocol checks, invariant guards, and repository checks.

The real batch then exercised the actual Electron UI, actual Codex app-server
lane, actual Claude Agent SDK lane, complete quit/relaunch, more-than-five-chat
pressure, report/analyzer production, verdict retention, and automatic
cross-provider rotation.

## Residual gates

- #8978 must complete and independently admit the AssuranceSpec obligations;
  this implementation cannot self-admit them.
- #8979 must bind these behaviors to one signed packaged candidate and the
  distribution release set, including packaged owner quit/relaunch evidence.
- #8967 remains open while either of those children remains open.

Those residuals do not retract the narrower result proved here: Full Auto is
implemented and working in the real owner Desktop development system with both
Codex and Claude, durable pass-off in both directions, restart continuity,
thread-pressure survival, and automatic same-pass failover.
