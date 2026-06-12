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

**Not ready at this exact commit as the owner's Pylon-only daily-driver
surface.**

**Much closer than the earlier wording implied for supervised use.** The
daily-driver MVP being asked for here is not "run all night unattended and
prove the market." It is: the owner is sitting at the machine, watching Pylon,
and wants Codex to make normal repo edits from inside Pylon, with Fable
available occasionally as review/planning backup.

For that bar, overnight unattended proof (#4768), market/provider settlement,
and independent paid-capacity receipts are **not** gating. They matter for
autonomous overnight runs, serving other people's work, and public readiness
claims. They do not block a local supervised switch.

The honest blocker is narrower and more immediate:

- The TUI composer is still wired to OpenCode, not Codex. In
  `apps/pylon/src/tui/app.tsx`, `submitPrompt()` looks up `opencode` and calls
  `runOpencodeStream(...)`. A normal prompt typed into Pylon does not yet
  start a Codex coding session.
- Pylon's existing Codex SDK assignment executor is deliberately bounded to
  `read-only` / `workspace-write`; it does not expose the owner-requested
  supervised `--dangerously-bypass-approvals-and-sandbox` / `danger-full-access`
  behavior. That is correct for public assignments, but too restrictive for
  local supervised dev mode.
- Pylon does not yet have a first-class `pylon dev`/current-repo mode that
  assembles repo context, instruction refs, account readiness, execution mode,
  checks, patch summary, and reload in one loop.
- `pylon work submit` exists, but its current request builder hard-codes a
  placeholder commit SHA (`1111111111111111111111111111111111111111`) instead
  of resolving the requested repo/branch to a real pinned commit or requiring a
  `--commit`. That blocks the network/work-order lane, not the fastest local
  supervised lane.
- "Codex primary, Fable occasionally" is not a first-class preference surface.
  The adapter policy can represent Codex and Claude/Fable, but a dual-capable
  Pylon defaults to Claude, and the CLI does not expose `--adapter codex` /
  `--adapter fable` for work orders.

The practical recommendation is now sharper: **switch the implementation focus
immediately to a local supervised Pylon dev mode, with Codex as the composer
backend and an explicit dangerous local execution option.** Once that lands,
the owner can switch day-to-day coding to Pylon while still watching the run.
Autopilot work-order commit pinning, paid market, PR writeback, and overnight
proof remain follow-on lanes.

Top-priority issues filed from this correction:

- #4839: P0 composer/current-repo Codex mode.
- #4840: P0 local-only dangerous Codex execution mode.
- #4841: P0 `pylon dev doctor` repo/instruction/account context projection.
- #4842: P0 dev check/apply/reload loop.
- #4843: P1 work-order commit pinning and adapter intent.

**Not minimally usable as a Pylon-only replacement today.** The core blockers
are product and wiring blockers:

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
- Delivery is still evidence-first in the work-order lane, not a normal coding
  handoff. That is acceptable for the local dev path if the first version shows
  the changed files, focused checks, and reload state inside Pylon.

The minimum switch criterion is one retained supervised proof:
the owner types a real repo request into Pylon, Pylon runs Codex in the active
repo, Codex edits, focused checks pass, Pylon shows the patch/check summary,
and the owner can reload or continue without leaving the Pylon loop.

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
- Assignment executor intentionally has no `danger-full-access`; only
  `read-only` or `workspace-write`.
- Network disabled inside the Codex SDK thread.
- Workspace escape detection through post-hoc file-change validation.
- CI-safe and live smoke runbooks.
- Live device receipt for a real Codex SDK fixture task.
- Live API-parity receipt for an API-submitted `git_checkout` task on a
  codex-only Pylon against the public fixture repo, with independent `bun test`
  verification and accepted closeout.

For bounded assignment dogfood, Codex is real.

The gap is not "Codex cannot run." The gap is "the owner's daily CLI workflow
does not yet route the Pylon composer to Codex, does not expose the local
supervised dangerous mode the owner is willing to use, and does not return a
convenient patch/check/reload handoff."

The local Codex CLI already exposes the needed supervised mechanism on this
machine:

```sh
codex exec --json -C <active-repo> --dangerously-bypass-approvals-and-sandbox <prompt>
```

That should be consumed by a local-only `pylon dev` / composer path, not by the
public assignment executor.

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

| Workflow need                                  | Status                         | Notes                                                                                                                                                          |
| ---------------------------------------------- | ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Start Pylon locally                            | Source-ready                   | v0.3 runs from source; published stable package is not v0.3.                                                                                                   |
| Type a coding prompt into Pylon and run Codex  | Not built                      | Composer currently routes to OpenCode, not Codex. This is #4839.                                                                                               |
| Run Codex unrestricted while the owner watches | Not built in Pylon             | Codex CLI supports it; Pylon needs a local-only dangerous mode. This is #4840.                                                                                 |
| Show active repo/instructions/accounts         | Not built                      | Needed for confidence before dangerous local Codex runs. #4838 covers the pane; #4841 covers the projection/doctor.                                            |
| Check/apply/reload after a Codex edit          | Not built                      | Needed for Pylon-to-Pylon daily work. This is #4842.                                                                                                           |
| Keep a headless worker online                  | Source-ready                   | `pylon node` and `PYLON_ASSIGNMENT_WORKER=1` exist for no-spend owner assignments, but this is not required for local supervised daily-driver MVP.             |
| Register / heartbeat / show status             | Built                          | Presence and status commands exist; live worker-loop smoke passed. Not required for the fastest local switch.                                                  |
| Submit work from Pylon work-order lane         | Built but not day-to-day safe  | `pylon work submit` exists, but sends a placeholder commit SHA today. #4843 tracks the fix.                                                                    |
| Read work status/events                        | Built                          | `pylon work status <work-order-ref> [--events]` exists for the work-order lane.                                                                                |
| Review delivered work                          | Built                          | `pylon work review <work-order-ref> --action ...` exists for the work-order lane.                                                                              |
| Prefer owner Pylon before paid fallback        | Built as policy                | Own-Pylon/free-lane policy is in the #4786 ladder and code. Not required for local supervised dev mode.                                                        |
| Codex assignment execution                     | Live-proven                    | Codex SDK task and `git_checkout` parity ran live with receipts.                                                                                               |
| Fable execution                                | Via Claude Agent               | Use Claude Agent lane with `model: "claude-fable-5"`; live Claude lane is proven, but Fable-specific daily-driver review flow is not recorded.                 |
| Adapter choice per work-order task             | Partially built                | Policy can model adapter requirements, but `pylon work submit` does not expose them and the synthesizer currently selects from placed Pylon capabilities only. |
| Codex as default on dual-capability Pylon      | Not built for work orders      | Dual-capability default is Claude. Local dev mode should default to Codex independently of that placement policy.                                              |
| PR draft/writeback                             | Contract-ready, not live claim | Delivery readiness exists; live PR writeback and maintainer merge remain separate authority. Not needed for supervised MVP.                                    |
| Paid work / settlement                         | Not daily-driver MVP           | Live market/settlement issues remain open, but do not block owner-supervised local use.                                                                        |
| Overnight unattended run                       | Not daily-driver MVP           | #4768 is still open, but it gates "start at night, review in the morning," not "sit here watching Codex."                                                      |
| Public or external market capacity             | Not daily-driver MVP           | #4777/#4781/#4782/#4783 remain open for independent provider/settlement receipts.                                                                              |

## Most Important Implementation Findings

### 1. The TUI composer does not run Codex

`apps/pylon/src/tui/app.tsx` still labels the composer path as:

```ts
// --- Composer -> OpenCode interaction --------------------------------------
```

`submitPrompt()` then looks up `opencode` and calls `runOpencodeStream(...)`.
That means the first-screen Pylon experience cannot yet be "type request,
Codex edits current repo."

Required fix:

- Add a Codex composer backend, defaulting to Codex for dev/daily-driver mode.
- Drive the active repo/current checkout directly.
- Stream Codex CLI JSONL or equivalent events into the existing feed.
- Make missing Codex CLI/auth a visible typed blocker, not an OpenCode error.
- Keep OpenCode as an optional backend if desired, not the default for the
  owner daily-driver path.

### 2. Pylon has no local supervised dangerous Codex mode

The current Codex assignment executor intentionally never expands beyond
`read-only` / `workspace-write`. That is the correct safety boundary for
public assignment and provider lanes.

For the supervised owner daily-driver MVP, the required mode is different:
Pylon should be able to invoke the local Codex CLI with
`--dangerously-bypass-approvals-and-sandbox`, or an equivalent explicit
`danger-full-access`/approval-never configuration, while the owner is watching.

Required fix:

- Add a local-only execution mode such as `local_supervised_danger`.
- Require explicit opt-in.
- Show the active mode in the TUI/context projection.
- Reject this mode for assignments, paid work, market/provider lanes, and
  public execution claims.

### 3. `pylon work submit` does not pin the real repo state

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
not acceptable for the work-order lane. It does not block the fastest local
supervised path if Pylon runs Codex directly in the active checkout.

Required fix:

- Add `--commit <sha>` to `pylon work submit`, or resolve `--repo` + `--branch`
  to a pinned SHA through a safe GitHub API / git ls-remote boundary before
  submission.
- Reject placeholder SHAs in CLI-generated real work.
- Record the pinned commit in the command output so the user knows what code
  the agent worked on.

### 4. Codex-primary preference is not exposed for work orders

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

Current open issues after the correction:

- #4839 P0: make Pylon composer run Codex in the current repo.
- #4840 P0: add local-only dangerous Codex mode for supervised Pylon dev.
- #4841 P0: add `pylon dev doctor` for repo, instruction, and account context.
- #4842 P0: add Pylon dev check/apply/reload loop for supervised Codex changes.
- #4843 P1: make `pylon work submit` pin real commits and carry adapter intent.
- #4838: add the TUI repo/account/instruction context pane.
- #4786 parent epic: Autopilot MVP issue ladder.
- #4768 M10: overnight unattended proof, both lanes, both surfaces.
- #4772 M14: MVP exit review / door-open decision.
- #4777 P1: first live negotiated labor job on a real backlog issue.
- #4781 P5: backlog faucet for the open market.
- #4782 P6: spare-capacity provider mode.
- #4783 P7: Lane C fanout.
- #4749 W3: separate Tassadar/Psion research sweep, not an MVP dependency.

The issue tail now has two lanes:

1. **Owner supervised daily-driver lane:** #4839, #4840, #4841, #4842, plus
   #4838 for the visible context pane. This is the ASAP switch path.
2. **Autopilot/public readiness lane:** #4843, #4768, #4772, #4777, #4781,
   #4782, and #4783. These are still real, but they are not required for the
   owner to sit at Pylon and supervise Codex.

The terminal-agent operational roadmap is still correct that Pack A, Pack B,
Pack C, and #4836/#4837 are implemented and closed, and Pack D should not be
filed yet. The correction is that "missing live evidence" is not one category:
overnight unattended and market settlement evidence are public/autonomous
readiness evidence, not local supervised daily-driver evidence.

## Minimal Supervised Daily-Driver Path

This is the narrow path to switch ASAP. It is local, owner-supervised, and
source-checkout based.

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
  "dev": {
    "defaultAdapter": "codex",
    "codexExecutionMode": "local_supervised_danger"
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

At this exact commit, that starts the TUI but does not yet make the composer
Codex-backed. The required product delta is #4839 and #4840:

```sh
pylon dev --codex-danger
pylon dev fix "make this repo change"
```

The direct Codex invocation Pylon should wrap for the first MVP is:

```sh
codex exec --json -C <active-repo> --dangerously-bypass-approvals-and-sandbox <prompt>
```

This path should not require:

- `PYLON_OPENAGENTS_BASE_URL`;
- `OPENAGENTS_AGENT_TOKEN`;
- wallet readiness;
- Pylon worker registration;
- `pylon work submit`;
- market/provider quotes;
- overnight unattended proof.

Those become relevant only when using the assignment/work-order path. For
headless owner-assignment dogfood, the existing path remains:

```sh
PYLON_OPENAGENTS_BASE_URL=https://openagents.com \
OPENAGENTS_AGENT_TOKEN=oa_agent_... \
PYLON_ASSIGNMENT_WORKER=1 \
bun apps/pylon/src/index.ts node
```

Do not confuse the two paths. The ASAP daily-driver path is local direct Codex
inside Pylon. The assignment path is useful for proving Autopilot and worker
behavior, but it is not the shortest route to the owner switching today.

## P0 Fix List Before Supervised Daily-Driver Switch

These are the small, direct fixes before replacing the owner's normal coding
workflow while the owner is watching.

1. **Wire the composer/current repo to Codex (#4839).**
   - Use Codex as the default daily-driver backend.
   - Drive the active repo with `codex exec --json -C <repo> ...`.
   - Stream output into the TUI feed.
   - Stop requiring OpenCode for the owner path.

2. **Add explicit local-only dangerous Codex mode (#4840).**
   - Wrap `--dangerously-bypass-approvals-and-sandbox` or equivalent
     `danger-full-access`/approval-never behavior.
   - Require explicit opt-in.
   - Show the mode in Pylon.
   - Reject it for public assignment/provider/market lanes.

3. **Add dev doctor/context projection (#4841) and TUI pane (#4838).**
   - Show repo, branch/commit, dirty state, instruction refs, Codex/OpenAI
     readiness, Claude/Fable readiness, and current execution mode.
   - Keep it typed and redacted.

4. **Add dev check/apply/reload loop (#4842).**
   - Show change summary and changed file refs.
   - Run focused checks.
   - Restart/reload Pylon explicitly after acceptance.
   - Do not commit, push, branch switch, or destructive-clean without a
     separate command.

5. **Run one retained supervised daily-driver proof.**
   - Use a real repo task while the owner watches.
   - Submit from the Pylon composer or `pylon dev fix`, not `pylon work submit`.
   - Codex edits in the active checkout.
   - Focused checks pass.
   - Pylon shows the patch/check/reload state.
   - Fable review is optional, not blocking.

6. **Install-pin v0.3 for the owner.**
   - Either publish `@openagentsinc/pylon@0.3.0` after release gates and npm
     credential repair, or define an owner-only install pin from the source
     checkout so the daily command does not depend on unpublished package
     semantics.

Work-order lane follow-up:

- Fix `pylon work submit` commit pinning and adapter intent (#4843).
- Run M10 (#4768) before relying on "start at night, review in the morning."
- Complete M14/market/provider issues before calling Pylon broadly/publicly
  ready.

## Daily-Driver Decision

At this exact commit, use Pylon today for:

- supervised Codex dogfood;
- Codex and Claude/Fable bridge smokes;
- owner-Pylon no-spend assignment testing;
- validating the work-order spine and receipt discipline;
- terminal/TUI/headless-node workflow testing.

Do not yet use Pylon as the only daily-driver surface until #4839 and #4840
land, because the current composer still invokes OpenCode and the local
dangerous Codex mode does not exist in Pylon.

Once #4839/#4840/#4841/#4842 land and one supervised proof is retained, the
decision can become:

- **yes** for owner-supervised local Pylon + Codex daily work;
- **still no** for unattended overnight runs until #4768;
- **still no** for market/provider/paid public capacity until #4777/#4781/
  #4782/#4783 and #4772 complete;
- **still no** for arbitrary `pylon work submit` repo tasks until #4843.

Do not yet use Pylon as the default for:

- arbitrary repo tasks from `pylon work submit`;
- unattended overnight coding without operator watch;
- paid work or market-provider work;
- automatic PR writeback/merge;
- broad public "Codex primary with Fable fallback" routing.

The shortest honest path to "yes, switch" is not a new architecture pack. It
is a focused P0 pass on the local Pylon dev/composer loop, followed by one
real owner-watched proof. After that pass, the answer can change from
"controlled dogfood only" to "minimally usable for the owner's supervised
day-to-day repo tasks, with work-order, PR/writeback, unattended, and
paid-market still gated."

## Proposed Addendum: Pylon Dev Mode

The owner should add a first-class **Pylon Dev Mode** before trying to make
Pylon the broad daily-driver surface. The purpose is narrow: make Pylon a good
tool for improving Pylon itself.

Dev Mode should be explicitly local, owner-only, and source-checkout based. It
should not be a market lane, paid-work lane, public promise, or autonomous
writeback path. It is the shortest bridge between "source-checkout dogfood" and
"I can actually work in Pylon all day."

This is no longer merely an addendum. It is the ASAP path:

- #4839 implements the Codex composer/current-repo lane.
- #4840 implements the explicit local supervised dangerous Codex mode.
- #4841 implements the redacted dev doctor/context projection.
- #4842 implements the check/apply/reload loop.

### Shape

Command surface:

```sh
pylon dev
pylon dev --codex-danger
pylon dev doctor --json
pylon dev fix "the assignment view is not refreshing"
pylon dev fix --codex-danger "fix the TUI composer"
pylon dev fix --from-last-error
pylon dev check
pylon dev reload
pylon dev review --adapter fable
```

TUI surface:

- A Dev pane that shows the active source checkout, branch, dirty-state
  summary, Pylon version, node status, adapter readiness, and last check
  result.
- A "Fix this" action on error rows, failed command rows, and stale assignment
  rows.
- A "Run checks" action that runs the relevant focused checks before broader
  release gates.
- A "Review with Fable" action when the Claude Agent lane is ready with
  `claude-fable-5`.
- Explicit "Apply", "Reload", and "Discard" actions. No silent commit, push,
  branch switch, or destructive cleanup.

### Dev Task Contract

Dev Mode should create a local-only Pylon dev task, not a normal public
Autopilot work order.

Minimum task fields:

- objective summary;
- source checkout identity;
- current commit ref;
- dirty-state summary;
- failing command ref;
- last error summary ref;
- targeted file refs;
- adapter preference, defaulting to Codex;
- optional reviewer adapter, usually Fable through Claude Agent;
- expected check command refs;
- redaction receipt refs;
- local-only evidence refs.

The task can use the same Agent Runtime Kernel and workspace authority
contracts as normal Pylon assignments, but the projection boundary must be
different: private local diagnostics may stay local, while anything written to
OpenAgents or GitHub must be ref-only and public-safe.

### Built-In Debug Context

`pylon dev doctor --json` should assemble one redacted context bundle:

- `pylon status --json`;
- local package version and source commit;
- adapter readiness for Codex and Claude/Fable;
- `PYLON_HOME` layout health without raw paths in public refs;
- current node/control server state;
- recent Pylon log summaries, not raw logs;
- last failed command ref and exit code;
- current dirty-state summary;
- relevant test files and command refs;
- open assignment refs when present;
- environment capability refs, never raw env values.

That bundle is what Codex receives by default for `pylon dev fix`. Raw tokens,
provider payloads, wallet material, local absolute paths, private repo content,
and raw shell logs must be stripped unless the user explicitly keeps the task
purely local and unexported.

### Codex Main, Fable Review

Dev Mode should match the owner's desired operating pattern:

- Codex is the default implementer.
- Fable is an optional reviewer, planner, or second-pass debugger through the
  Claude Agent lane.
- The UI should say "Fable review" only when the configured Claude Agent model
  is `claude-fable-5` and readiness is green.
- If Fable is not ready, the command should degrade to a typed blocker, not
  silently choose another model.

This is cleaner than trying to make Fable a separate adapter today. The system
already has a proven Claude Agent bridge; Dev Mode should consume it with a
model preference.

### Check And Reload Loop

The core loop should be:

1. User hits "Fix this" or runs `pylon dev fix`.
2. Dev Mode creates a local dev task with a redacted diagnostic bundle.
3. Codex edits the source checkout in a bounded workspace policy.
4. Dev Mode runs focused checks first.
5. If focused checks pass, Dev Mode can run a broader Pylon check subset.
6. The TUI shows a patch summary, changed file refs, check refs, and blockers.
7. User explicitly applies or discards.
8. If applied, Dev Mode restarts or reloads the Pylon node/TUI.

Initial check ladder:

- touched-file formatting;
- targeted unit tests;
- relevant Pylon smoke, for example `smoke:default-start`,
  `smoke:codex-agent-task`, or `smoke:claude-agent-task`;
- only then `bun run --cwd apps/pylon release:gate` when release-bearing.

Reload should be process supervision, not hot mutation. A safe first version is
"restart the local Pylon node and reattach the TUI after checks pass."

### Guardrails

Dev Mode should be stricter than normal coding, because it edits the tool the
owner is standing on:

- Require an explicit `--allow-dirty` if the checkout is dirty.
- Snapshot dirty-state before edits.
- Never switch branches in this workspace.
- Never commit or push unless the user asks in a separate explicit command.
- Never run destructive Git commands.
- Keep command execution on an allowlist.
- Keep file edits under the source checkout.
- Do not route through market/provider lanes.
- Do not spend money by default.
- Do not turn local diagnostics into public evidence without a redaction scan.

### Minimal Milestones

Recommended issue slice:

1. **DM1: Dev doctor.** Add `pylon dev doctor --json` with redacted local
   diagnostics and adapter readiness.
2. **DM2: Dev fix.** Add `pylon dev fix` that creates a local Codex-backed dev
   task, edits in place, and runs targeted checks.
3. **DM3: Dev reload.** Add a safe restart-and-reattach loop for the node/TUI.
4. **DM4: Fable review.** Add optional Claude/Fable review of the local patch
   summary and check refs.
5. **DM5: Retained proof.** Use Dev Mode to fix one real Pylon bug, retain the
   local refs, checks, patch summary, and reload evidence.

### Effect On This Audit

Dev Mode does not make Pylon broadly daily-driver ready by itself. It does make
the immediate dogfood recommendation much stronger:

- It gives the owner a built-in way to repair Pylon while using it.
- It turns failures into bounded local work instead of context pasted into an
  unrelated coding tool.
- It makes Codex-primary plus occasional Fable review a product workflow rather
  than a config workaround.
- It creates the retained proof needed to graduate from "controlled dogfood" to
  "minimally usable for the owner's day-to-day Pylon work."

The recommendation is to file Dev Mode as a new P0 owner-dogfood issue before
or alongside the `pylon work submit` commit-pin and adapter-preference fixes.
