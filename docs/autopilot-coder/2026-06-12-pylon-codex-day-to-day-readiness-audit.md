# Pylon Codex Day-To-Day Readiness Audit

Date: 2026-06-12

## Question

Can the owner switch day-to-day coding immediately to Pylon, with Codex as the
main workhorse and Fable pulled in occasionally?

This audit is intentionally operational. It does not ask whether the
architecture is promising; it asks whether the current system can carry the
owner's normal coding loop today: submit work from Pylon, prefer Codex, choose
Fable/Claude when wanted, run against real repositories, see status, review
results, and keep evidence honest.

## Source Set

Read in this pass:

- Current open issues #4749, #4768, #4772, #4777, #4781, #4782, #4783, and
  #4786, including comments.
- `docs/autopilot-coder/README.md`.
- `docs/autopilot-coder/implementation-log.md`.
- `docs/autopilot-coder/2026-06-09-autopilot-coder-current-status-gap-audit.md`.
- `docs/autopilot-coder/2026-06-10-autopilot-coder-full-flow-audit.md`.
- `docs/autopilot-coder/2026-06-11-autopilot-unified-audit-roadmap.md`.
- `docs/autopilot-coder/2026-06-11-autopilot-agent-runtime-kernel-audit.md`.
- `docs/autopilot-coder/2026-06-11-autopilot-worktree-support-audit.md`.
- `docs/autopilot-coder/terminal-agent-systems/2026-06-11-terminal-agent-systems-operationalization-roadmap.md`.
- Pack C docs for repository/worktree identity, change capture, workspace
  authority, and delivery readiness.
- `apps/pylon/README.md`.
- `apps/pylon/docs/codex-bridge.md`.
- `apps/pylon/docs/codex-agent-task-smoke.md`.
- `apps/pylon/docs/claude-agent-bridge.md`.
- `apps/pylon/docs/claude-agent-task-smoke.md`.
- `apps/pylon/docs/2026-06-11-v030-release-preparation-record.md`.
- `apps/pylon/docs/release-install-smokes.md`.
- `apps/pylon/docs/live-worker-loop-smoke.md`.
- `apps/pylon/docs/packaged-live-network-smoke.md`.
- Current implementation surfaces in `apps/pylon/src/index.ts`,
  `apps/pylon/src/work-requester.ts`, `apps/pylon/src/codex-agent*.ts`,
  `apps/pylon/src/claude-agent*.ts`,
  `apps/openagents.com/workers/api/src/autopilot-work-adapter-selection.ts`,
  `apps/openagents.com/workers/api/src/autopilot-work-pylon-assignment-synthesizer.ts`,
  and the related tests.

This is a docs-only audit. It does not create or change a product promise.

## Verdict

**Not ready as the owner's supported default day-to-day coding environment.**

**Minimally usable for controlled dogfood from the source checkout: yes, with
constraints.** If the owner runs Pylon v0.3 from `main`, has a registered
OpenAgents agent token, has an online owner-linked Pylon, and has local Codex
credentials ready, the Codex assignment path has live receipts and can execute
bounded public-repo work. That is enough for deliberate dogfood tasks where the
operator watches the refs and accepts manual rough edges.

**Not minimally usable as a frictionless replacement for Codex/Codex CLI in the
owner's normal daily loop.** The core blockers are not deep architecture
blockers; they are product and wiring blockers:

- The only published installable package is still `@openagentsinc/pylon@0.2.5`.
  The v0.3 code with Codex/Claude/Fable paths is `0.3.0-rc2` in source and has
  not been published as stable.
- `pylon work submit` exists, but its current request builder hard-codes a
  placeholder commit SHA (`1111111111111111111111111111111111111111`) instead
  of resolving the requested repo/branch to a real pinned commit or requiring a
  `--commit`. That is not day-to-day safe for real repo work.
- "Codex primary, Fable occasionally" is not a first-class preference surface.
  The adapter policy can represent Codex and Claude/Fable, but a dual-capable
  Pylon defaults to Claude, and the CLI does not expose `--adapter codex` /
  `--adapter fable`.
- Delivery is still evidence-first, not a normal coding handoff. Pack C
  defines repo identity, change capture, workspace authority, and PR readiness
  receipts, but live PR writeback / branch push / maintainer merge is not
  something this path should claim yet.
- The still-open gates are live proof gates: overnight unattended proof,
  MVP door-open decision, independent market/provider settlement, and W3
  research evals. Those do not prevent source-level Codex dogfood, but they do
  prevent calling the system broadly ready.

The practical recommendation: **start dogfooding Pylon now as a supervised
source-checkout lane, but do not switch the owner's full daily coding default
until the small P0s in this audit are fixed and one real "owner asks, Codex
edits, tests pass, review result is actionable" run is retained.**

## What Is Actually Working

### Pylon v0.3 source tree

Pylon v0.3 exists in `apps/pylon` as `0.3.0-rc2`. It includes:

- TUI dashboard and headless node mode.
- `pylon work submit/status/review/request/offers/accept`.
- Pylon registration, heartbeat, assignment polling, no-spend assignment
  execution, progress, artifacts, and closeout.
- Codex Agent task execution through `@openai/codex-sdk` as an optional lazy
  dependency.
- Claude Agent task execution through `@anthropic-ai/claude-agent-sdk` as an
  optional lazy dependency.
- Workspace materialization for public GitHub `git_checkout` tasks using a
  shared adapter-neutral materializer.
- Agent Runtime Kernel event/projection convergence for Codex, Claude,
  OpenCode, native, and fixtures.

This source tree is real enough to dogfood. It is not yet the supported package
users get from `npm install -g @openagentsinc/pylon`.

### Published package state

`apps/pylon/README.md` records the key release truth:

- Published supported package: `@openagentsinc/pylon@0.2.5`.
- Current source package: `0.3.0-rc2`.
- Stable v0.3 release is blocked by npm publishing credentials for the
  `@openagentsinc` / `@openagents` scopes.
- The v0.3 release gate passed on macOS for rc2, but the runbook still calls
  for macOS and Linux gate evidence before tagging stable.

For the owner, this means immediate dogfood is a source-checkout workflow:

```sh
cd /Users/christopherdavid/work/openagents
bun install
bun run --cwd apps/pylon start
```

That is acceptable for internal dogfood. It is not a clean daily-driver
install story.

### Codex as the workhorse

The Codex lane is the strongest part of the answer.

Built:

- Readiness probe in `apps/pylon/src/codex-agent.ts`.
- Owner-held credential sources only: `CODEX_API_KEY`, `OPENAI_API_KEY`, or
  the owner's own `codex login` state.
- Capability declaration: `capability.pylon.local_codex`.
- Bounded executor in `apps/pylon/src/codex-agent-executor.ts`.
- Optional `@openai/codex-sdk` lazy import.
- No `danger-full-access`; only `read-only` or `workspace-write`.
- Network disabled inside the Codex SDK thread.
- Workspace escape detection through post-hoc file-change validation.
- CI-safe and live smoke runbooks.
- Live device receipt for a real Codex SDK fixture task.
- Live API-parity receipt for an API-submitted `git_checkout` task on a
  codex-only Pylon against the public fixture repo, with independent `bun test`
  verification and accepted closeout.

For bounded source-level dogfood, Codex is real.

The gap is not "Codex cannot run." The gap is "the owner's daily CLI workflow
does not yet force Codex cleanly, pin the requested repo commit correctly, and
return a convenient patch/PR handoff."

### Fable

"Fable" is not a separate Pylon adapter in this repo. It appears as a Claude
model/profile name, especially `claude-fable-5`.

The practical path is:

- Enable the Claude Agent lane.
- Configure `claudeAgent.model` as `claude-fable-5` in `~/.pylon/config.json`.
- Let Pylon declare `capability.pylon.local_claude_agent` when local Claude
  readiness is true.

The Claude Agent bridge is built and receipt-backed:

- Probe and capability declaration exist.
- BYOK/local-session credential policy exists.
- Bounded executor with PreToolUse workspace guard exists.
- CI-safe and live smokes exist.
- The #4755 live local Claude task and #4756 API-submitted `git_checkout`
  proof are recorded.

So the honest statement is: **Pylon can pull in Fable through the Claude Agent
lane when the local Claude/Fable credential/session is ready.** There is no
`fable_agent_task` work class or Pylon-native "call Fable" adapter.

## Current Workflow Fit

| Workflow need                             | Status                         | Notes                                                                                                                                                          |
| ----------------------------------------- | ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Start Pylon locally                       | Source-ready                   | v0.3 runs from source; published stable package is not v0.3.                                                                                                   |
| Keep a headless worker online             | Source-ready                   | `pylon node` and `PYLON_ASSIGNMENT_WORKER=1` exist for no-spend owner assignments.                                                                             |
| Register / heartbeat / show status        | Built                          | Presence and status commands exist; live worker-loop smoke passed.                                                                                             |
| Submit work from Pylon                    | Built but not day-to-day safe  | `pylon work submit` exists, but sends a placeholder commit SHA today.                                                                                          |
| Read work status/events                   | Built                          | `pylon work status <work-order-ref> [--events]` exists.                                                                                                        |
| Review delivered work                     | Built                          | `pylon work review <work-order-ref> --action ...` exists.                                                                                                      |
| Prefer owner Pylon before paid fallback   | Built as policy                | Own-Pylon/free-lane policy is in the #4786 ladder and code.                                                                                                    |
| Codex execution                           | Live-proven                    | Codex SDK task and `git_checkout` parity ran live with receipts.                                                                                               |
| Fable execution                           | Via Claude Agent               | Use Claude Agent lane with `model: "claude-fable-5"`; live Claude lane is proven, but Fable-specific daily-driver proof is not recorded.                       |
| Adapter choice per task                   | Partially built                | Policy can model adapter requirements, but `pylon work submit` does not expose them and the synthesizer currently selects from placed Pylon capabilities only. |
| Codex as default on dual-capability Pylon | Not built                      | Dual-capability default is Claude. To force Codex today, disable Claude capability or use a codex-only Pylon.                                                  |
| Real repo checkout from CLI               | Blocked by commit pin          | The CLI must resolve or accept a real commit SHA before this is safe.                                                                                          |
| Change capture / delivery refs            | Built as Pack C contracts      | Digest/summary evidence exists; not equivalent to a convenient patch review UX.                                                                                |
| PR draft/writeback                        | Contract-ready, not live claim | Delivery readiness exists; live PR writeback and maintainer merge remain separate authority.                                                                   |
| Paid work / settlement                    | Not daily-driver ready         | Live market/settlement issues remain open.                                                                                                                     |
| Overnight unattended run                  | Not proven                     | #4768 is still open.                                                                                                                                           |
| Public or external market capacity        | Not ready                      | #4777/#4781/#4782/#4783 remain open for independent provider/settlement receipts.                                                                              |

## The Two Most Important Implementation Findings

### 1. `pylon work submit` does not pin the real repo state

`apps/pylon/src/work-requester.ts` builds an Autopilot work request with:

```ts
commitSha: "1111111111111111111111111111111111111111";
```

The server schema requires a 40-character commit SHA, but it does not prove
that this placeholder exists in the requested repository before the assignment
is synthesized. The Pylon workspace materializer later verifies commit-object
existence, so a real run should fail at checkout/materialization for normal
repositories.

This is acceptable for fixture-oriented tests that assert request shape. It is
not acceptable for day-to-day coding.

Required fix:

- Add `--commit <sha>` to `pylon work submit`, or resolve `--repo` + `--branch`
  to a pinned SHA through a safe GitHub API / git ls-remote boundary before
  submission.
- Reject placeholder SHAs in CLI-generated real work.
- Record the pinned commit in the command output so the user knows what code
  the agent worked on.

### 2. Codex-primary preference is not exposed

The adapter-selection policy says requester intent wins, but the current
Autopilot Pylon assignment synthesizer calls `selectCodingAdapter` with only
the placed Pylon's capability refs. In the tested path:

- codex-only Pylon -> `codex_agent_task`;
- dual-capability Pylon -> `claude_agent_task`;
- no candidate capability info -> Claude fallback.

That is correct for the previous platform default, but it conflicts with the
owner's desired day-to-day mode: Codex main, Fable occasionally.

Required fix:

- Add `--adapter codex|fable|claude_agent` to `pylon work submit`.
- Carry that choice as requester intent into the Autopilot request and Pylon
  assignment synthesis.
- Set the owner's local default adapter to Codex in Pylon config.
- Treat Fable as a Claude Agent model choice unless and until there is a
  separate Fable adapter.

Temporary workaround:

- Run a codex-only Pylon by disabling `claudeAgent.enabled` in
  `~/.pylon/config.json`.
- When Fable is needed, re-enable the Claude Agent lane and either submit
  through a future explicit adapter flag or run a targeted Claude/Fable smoke.

That workaround is too clumsy for normal daily work.

## Open Issue Tail

Current open issues as of this audit:

- #4786 parent epic: Autopilot MVP issue ladder.
- #4768 M10: overnight unattended proof, both lanes, both surfaces.
- #4772 M14: MVP exit review / door-open decision.
- #4777 P1: first live negotiated labor job on a real backlog issue.
- #4781 P5: backlog faucet for the open market.
- #4782 P6: spare-capacity provider mode.
- #4783 P7: Lane C fanout.
- #4749 W3: separate Tassadar/Psion research sweep, not an MVP dependency.

The issue tail is not missing decomposition. The terminal-agent operational
roadmap is explicit: Pack A, Pack B, Pack C, and #4836/#4837 are implemented
and closed; Pack D should not be filed yet. The missing ingredient is live
evidence:

- M10 live overnight receipts;
- M14 decision record with receipt refs and accepted deferrals;
- fresh open-market target and independent provider quote/execution;
- validator acceptance, release, payout, and settlement receipts;
- W3 A/B/C baseline artifacts for the separate research issue.

For the owner's local day-to-day switch, #4768 and #4772 matter most. The
market issues matter for serving other people's work or using outside provider
capacity, not for supervised owner-Pylon Codex dogfood.

## Minimal Dogfood Configuration

This is the narrow path that is honest today.

Run from source:

```sh
cd /Users/christopherdavid/work/openagents
bun install
```

Prepare local Pylon config with Codex primary:

```json
{
  "codex": {
    "enabled": true,
    "model": "gpt-5.4-codex",
    "maxTurns": 12,
    "timeoutSeconds": 600,
    "sandboxMode": "workspace-write"
  },
  "claudeAgent": {
    "enabled": false,
    "model": "claude-fable-5",
    "maxTurns": 12,
    "timeoutSeconds": 600
  }
}
```

Environment:

```sh
export PYLON_OPENAGENTS_BASE_URL=https://openagents.com
export OPENAGENTS_AGENT_TOKEN=oa_agent_...
```

Codex readiness must come from one of:

- `CODEX_API_KEY`;
- `OPENAI_API_KEY`;
- the owner's own `codex login` state.

Then start:

```sh
bun run --cwd apps/pylon start
```

For headless owner-assignment dogfood:

```sh
PYLON_OPENAGENTS_BASE_URL=https://openagents.com \
OPENAGENTS_AGENT_TOKEN=oa_agent_... \
PYLON_ASSIGNMENT_WORKER=1 \
bun apps/pylon/src/index.ts node
```

Do not treat this as a broad production workflow yet. Until the commit-pin and
adapter-choice fixes land, use existing Codex/Claude smoke paths for evidence
or submit only carefully prepared assignments whose payloads you inspect.

## P0 Fix List Before Full Daily-Driver Switch

These are small, direct fixes. They should be done before replacing the
owner's normal coding workflow.

1. **Fix `pylon work submit` commit pinning.**
   - Add `--commit`.
   - Or resolve `--repo`/`--branch` to a pinned SHA before submission.
   - Reject placeholder commits outside tests.

2. **Expose adapter choice and default Codex.**
   - Add `--adapter codex|fable|claude_agent`.
   - Persist a local default adapter preference.
   - Carry requester adapter intent through Autopilot work submission and
     assignment synthesis.
   - Keep Fable mapped to Claude Agent `model: "claude-fable-5"` unless a
     dedicated adapter is introduced.

3. **Run one retained owner daily-driver proof.**
   - Use a real currently-open issue or bounded public repo task.
   - Submit from `pylon work submit`.
   - Route to codex-only or explicit Codex Pylon.
   - Materialize a real pinned commit.
   - Codex edits/tests in the bounded workspace.
   - Work status/events show matching state.
   - Review action records the result.
   - Redaction scan and Pack C evidence refs stay clean.

4. **Improve delivery ergonomics.**
   - At minimum, show change summary, patch digest, verification result, and
     where the local retained workspace lives to the operator.
   - Next, wire PR draft readiness into a controlled PR-draft path. Do not
     claim merge authority.

5. **Publish or install-pin v0.3.**
   - Either publish `@openagentsinc/pylon@0.3.0` after release gates and npm
     credential repair, or define an owner-only install pin from the source
     checkout so the daily command does not depend on unpublished package
     semantics.

6. **Run M10.**
   - The system should prove one overnight unattended SHC run and one
     own-Pylon/cloud-Pylon run, with matching terminal and web states, before
     the owner relies on it for "start at night, review in the morning."

## Daily-Driver Decision

Use Pylon today for:

- supervised Codex dogfood;
- Codex and Claude/Fable bridge smokes;
- owner-Pylon no-spend assignment testing;
- validating the work-order spine and receipt discipline;
- terminal/TUI/headless-node workflow testing.

Do not yet use Pylon as the default for:

- arbitrary repo tasks from `pylon work submit`;
- unattended overnight coding without operator watch;
- paid work or market-provider work;
- automatic PR writeback/merge;
- broad "Codex primary with Fable fallback" routing.

The shortest honest path to "yes, switch" is not a new architecture pack. It
is a focused P0 pass on the CLI and adapter preference, followed by one real
owner daily-driver proof. After that pass, the answer can change from
"controlled dogfood only" to "minimally usable for the owner's day-to-day
public-repo tasks, with PR/writeback and paid-market still gated."
