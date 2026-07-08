# Pylon Codex Day-To-Day Readiness Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


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

The honest blocker is now narrower and more operational. After the
#4839/#4840/#4841/#4842 implementation passes, the composer/current-repo Codex
path, explicit local dangerous mode, repo/context projection, and
check/apply/reload loop exist in source:

- The TUI composer now uses `@openai/codex-sdk` through
  `apps/pylon/src/codex-composer.ts`, defaulting to the current working
  directory (`PYLON_CODEX_CWD` / `PYLON_ACTIVE_REPO` override it) and showing
  typed SDK/auth readiness blockers before any thread starts. This closes the
  original OpenCode-composer blocker for source dogfood.
- The local dashboard composer can now opt into
  `local_supervised_danger`, which maps to SDK
  `sandboxMode: "danger-full-access"` with `approvalPolicy: "never"` and is
  visibly labeled `Codex DANGER`. The assignment executor remains deliberately
  bounded to `read-only` / `workspace-write`.
- `pylon dev doctor --json`, `pylon context --json`,
  `pylon dev check --json`, `pylon dev apply --json`, and
  `pylon dev reload --json` now provide the first typed local dev loop around
  the Codex composer. The TUI command palette exposes matching Dev and Context
  actions.
- The TUI now has a `Repo & AI Context` pane on wide dashboards and an `f6`
  context route on narrow terminals. It renders active repo, instruction,
  config, Codex/OpenAI, Claude/Fable, backend, current-job, and blocker refs
  from the typed projection without dumping secrets, auth paths, raw
  instruction text, local absolute paths, or provider account emails.
- `pylon work submit` now requires a real `--commit <40-char-sha>`, rejects
  all-zero/all-one placeholders, preflights the commit against the public
  GitHub repo before submission, and includes the pinned checkout in command
  output.
- "Codex primary, Fable occasionally" now has a first-class work-order intent
  surface: `--adapter codex`, `--adapter claude_agent`, or `--adapter fable`.
  Fable maps to the Claude Agent lane with `profile.claude_agent.fable`.
- The local dashboard composer can now select the Claude Agent SDK as an
  adapter too: `dev.defaultAdapter: "claude_agent"` or `--adapter claude`
  streams `query()` messages from the active repo, labels the backend as
  `Claude` / `Claude (<model>)`, and keeps raw session ids local while showing
  only hashed refs in the feed.

The practical recommendation is now sharper: **use the source checkout for a
retained supervised Pylon task, prove the Codex composer plus dev
check/apply/reload loop end to end, then pin the owner install path.** The
implementation no longer needs to wait for overnight unattended proof, paid
market receipts, PR writeback, or work-order-lane proof before the owner can
dogfood the local path while watching.

Top-priority issues filed from this correction:

- #4839: P0 composer/current-repo Codex mode. Implemented in source with the
  TypeScript SDK stream path.
- #4840: P0 local-only dangerous Codex execution mode. Implemented in source
  with SDK `danger-full-access` behind explicit local opt-in.
- #4841: P0 `pylon dev doctor` repo/instruction/account context projection.
  Implemented in source as a redacted JSON projection.
- #4838: P0 TUI repo/account/instruction context pane. Implemented in source
  with a wide `Repo & AI Context` pane, narrow `f6` route, and
  `pylon context --json`.
- #4842: P0 dev check/apply/reload loop. Implemented in source with typed
  local projections and command-palette actions.
- #4843: P1 work-order commit pinning and adapter intent. Implemented in
  source with explicit commit preflight and SDK/assignment intent plumbing.
- #4844: CL1 P0 Claude composer backend. Implemented in source with
  `@anthropic-ai/claude-agent-sdk` streaming, `dev.defaultAdapter` /
  `--adapter claude` selection, readiness blockers before launch, and local
  session resume.

**Not yet a supported Pylon-only replacement from the packaged install.** The
remaining blockers are productization and proof blockers:

- The only published installable package is still `@openagentsinc/pylon@0.2.5`.
  The v0.3 code with Codex/Claude/Fable paths is `0.3.0-rc2` in source and has
  not been published as stable.
- The work-order path is now source-hardened, but it remains a network/API
  lane. The fastest daily-driver switch is still the local composer/dev path,
  not a packaged `pylon work submit` flow.
- "Codex primary, Fable occasionally" is built for work orders as explicit
  intent and for the local composer as adapter selection. The local context
  pane shows Codex/Fable readiness. The remaining Fable/Claude gaps are the
  explicit local permissive Claude mode, richer Fable review ergonomics, and a
  retained supervised proof before it should replace the owner's normal loop.
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
- Local dashboard composer has an explicit
  `local_supervised_danger` mode (`--codex-danger` or
  `dev.codexExecutionMode`) that maps to SDK `danger-full-access`; public
  work/provider/node/attach command paths reject that flag.
- Network disabled inside the Codex SDK thread.
- Workspace escape detection through post-hoc file-change validation.
- CI-safe and live smoke runbooks.
- Live device receipt for a real Codex SDK fixture task.
- Live API-parity receipt for an API-submitted `git_checkout` task on a
  codex-only Pylon against the public fixture repo, with independent `bun test`
  verification and accepted closeout.

For bounded assignment dogfood, Codex is real.

The gap is not "Codex cannot run." The gap is now "the owner's daily Pylon
workflow does not yet return a convenient repo/context, patch/check/reload
handoff."

The implementation decision for the local lane is to use the TypeScript SDK,
not a raw CLI parser. Pylon should create a Codex SDK thread with the active
repo as `workingDirectory`, then stream `thread.runStreamed(prompt)` events
into the TUI. The local supervised danger path maps to SDK
`sandboxMode: "danger-full-access"` with `approvalPolicy: "never"` on
`pylon dev` / the local composer path, not the public assignment executor.

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
lane when the local Claude/Fable credential/session is ready.** In source,
that now applies to both the work-order lane and the local dashboard composer:
set `claudeAgent.model` to `claude-fable-5` and select
`dev.defaultAdapter: "claude_agent"` or `--adapter claude`. There is no
`fable_agent_task` work class or separate Pylon-native Fable adapter.

## Current Workflow Fit

| Workflow need                                        | Status                         | Notes                                                                                                                                                                                           |
| ---------------------------------------------------- | ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------------------------------------------------------- |
| Start Pylon locally                                  | Source-ready                   | v0.3 runs from source; published stable package is not v0.3.                                                                                                                                    |
| Type a coding prompt into Pylon and run Codex        | Built in source (#4839)        | Composer routes through `@openai/codex-sdk`, uses the current working directory by default, and reports SDK/auth blockers before starting a thread.                                             |
| Type a coding prompt into Pylon and run Claude/Fable | Built in source (#4844)        | `dev.defaultAdapter: "claude_agent"` or `--adapter claude` routes through `@anthropic-ai/claude-agent-sdk`, resumes the local SDK session, and labels the model.                                |
| Run Codex unrestricted while the owner watches       | Built in source (#4840)        | `--codex-danger` or `dev.codexExecutionMode` maps the local dashboard composer to SDK `danger-full-access`; assignment/provider paths reject the flag.                                          |
| Show active repo/instructions/accounts               | Built in source (#4838/#4841)  | `pylon context --json` emits the redacted typed projection; the wide TUI renders `Repo & AI Context`, and narrow terminals can open the `f6` context route.                                     |
| Check/apply/reload after a Codex edit                | Built in source (#4842)        | `pylon dev check/apply/reload --json` emits typed local projections; command-palette Dev actions call the same loop. Reload is explicit and currently no-op unless a controlled process exists. |
| Keep a headless worker online                        | Source-ready                   | `pylon node` and `PYLON_ASSIGNMENT_WORKER=1` exist for no-spend owner assignments, but this is not required for local supervised daily-driver MVP.                                              |
| Register / heartbeat / show status                   | Built                          | Presence and status commands exist; live worker-loop smoke passed. Not required for the fastest local switch.                                                                                   |
| Submit work from Pylon work-order lane               | Source-hardened (#4843)        | `pylon work submit` requires `--commit`, rejects placeholder/unresolvable commits before submission, and prints the pinned checkout in output.                                                  |
| Read work status/events                              | Built                          | `pylon work status <work-order-ref> [--events]` exists for the work-order lane.                                                                                                                 |
| Review delivered work                                | Built                          | `pylon work review <work-order-ref> --action ...` exists for the work-order lane.                                                                                                               |
| Prefer owner Pylon before paid fallback              | Built as policy                | Own-Pylon/free-lane policy is in the #4786 ladder and code. Not required for local supervised dev mode.                                                                                         |
| Codex assignment execution                           | Live-proven                    | Codex SDK task and `git_checkout` parity ran live with receipts.                                                                                                                                |
| Fable execution                                      | Via Claude Agent               | Use Claude Agent lane with `model: "claude-fable-5"`; local composer selection is built in source, but Fable-specific daily-driver review/proof is not recorded.                                |
| Adapter choice per work-order task                   | Source-hardened (#4843)        | `--adapter codex                                                                                                                                                                                | claude_agent | fable` carries requester intent through validation, task records, assignment synthesis, and local runner selection. |
| Codex as default on dual-capability Pylon            | Explicit for work orders       | Intent-less dual-capability work orders still default to Claude by platform policy; owner Codex-primary use should pass `--adapter codex` or use local Dev Mode.                                |
| PR draft/writeback                                   | Contract-ready, not live claim | Delivery readiness exists; live PR writeback and maintainer merge remain separate authority. Not needed for supervised MVP.                                                                     |
| Paid work / settlement                               | Not daily-driver MVP           | Live market/settlement issues remain open, but do not block owner-supervised local use.                                                                                                         |
| Overnight unattended run                             | Not daily-driver MVP           | #4768 is still open, but it gates "start at night, review in the morning," not "sit here watching Codex."                                                                                       |
| Public or external market capacity                   | Not daily-driver MVP           | #4777/#4781/#4782/#4783 remain open for independent provider/settlement receipts.                                                                                                               |

## Most Important Implementation Findings

### 1. The TUI composer now runs Codex in source (#4839)

The original blocker was:

```ts
// --- Composer -> OpenCode interaction --------------------------------------
```

`submitPrompt()` looked up `opencode` and called `runOpencodeStream(...)`.
That meant the first-screen Pylon experience could not be "type request, Codex
edits current repo."

#4839 changes that in source:

- `apps/pylon/src/codex-composer.ts` uses `@openai/codex-sdk` `runStreamed()`
  events, not hand-parsed CLI output.
- `apps/pylon/src/tui/app.tsx` now accepts a composer backend instead of
  importing OpenCode directly.
- `apps/pylon/src/index.ts` wires the default dashboard and attach mode to a
  Codex backend in the current working directory.
- Missing SDK/auth readiness is a visible Codex blocker.
- The assignment executor remains bounded; dangerous mode is local composer
  only.

### 2. Pylon has local supervised dangerous Codex mode in source (#4840)

The current Codex assignment executor intentionally never expands beyond
`read-only` / `workspace-write`. That is the correct safety boundary for
public assignment and provider lanes.

For the supervised owner daily-driver MVP, the local mode is different:
Pylon can now invoke the Codex SDK with
`sandboxMode: "danger-full-access"` and `approvalPolicy: "never"` while the
owner is watching.

#4840 changes that in source:

- `--codex-danger` or `dev.codexExecutionMode:
"local_supervised_danger"` explicitly opts the local dashboard composer into
  the mode.
- The TUI backend label becomes `Codex DANGER` and the feed status line shows
  `mode: local_supervised_danger | sandbox: danger-full-access`.
- `runCodexComposerStream()` refuses `danger-full-access` unless the caller
  also declares `executionMode: "local_supervised_danger"`.
- `pylon work`, `pylon assignment`, `pylon provider`, `pylon node`, and
  `pylon attach` reject `--codex-danger` with
  `blocker.codex.local_supervised_danger_public_path`.
- `loadCodexAgentConfig()` still rejects assignment `codex.sandboxMode:
"danger-full-access"`; only `loadCodexDevConfig()` reads the local dev
  override.

### 3. `pylon work submit` now pins the real repo state in source (#4843)

`apps/pylon/src/work-requester.ts` no longer emits the all-ones fixture SHA for
real Autopilot work submissions. The source implementation now:

- requires `--commit <40-char-sha>` for `pylon work submit`;
- rejects all-zero/all-one placeholder SHAs and non-SHA branch names before any
  API call;
- preflights the SHA against the public GitHub `owner/repo` commits API before
  posting `/api/autopilot/work`;
- records the pinned repository, branch, and commit in `pylonSubmission`.

The server request schema and normalized coding-assignment payload also reject
placeholder commit pins, so the old fixture cannot leak into assignment
materialization.

### 4. Codex-primary preference is now explicit for work orders (#4843)

The adapter-selection policy already said requester intent wins. #4843 wires
that intent from Pylon CLI to assignment synthesis:

- `--adapter codex` sends `requestedAdapter: "codex"` and selects
  `codex_agent_task` on a dual-capability Pylon.
- `--adapter claude_agent` sends `requestedAdapter: "claude_agent"` and selects
  the Claude Agent lane when available.
- `--adapter fable` maps to `requestedAdapter: "claude_agent"` plus
  `requestedAdapterProfileRef: "profile.claude_agent.fable"`.

The platform default remains unchanged: an intent-less dual-capability Pylon
defaults to Claude. That is deliberate so other requesters are not affected by
the owner's Codex-primary preference. Owner daily-driver work should use local
Dev Mode or explicit `--adapter codex`.

Fable remains a profile/model on the Claude Agent lane, not a separate adapter.
When the placed Pylon lacks Claude Agent capability, synthesis refuses to
substitute Codex for a Fable/Claude request.

### 5. The local composer can now run Claude/Fable in the active repo (#4844)

The Codex composer remains the default for the owner's daily-driver path, but
the local TUI is no longer Codex-only. #4844 adds the parallel Claude Agent SDK
composer:

- `apps/pylon/src/claude-composer.ts` streams `query()` SDK messages and
  assistant text into the existing composer feed.
- `dev.defaultAdapter: "claude_agent"` or `--adapter claude` selects the
  Claude backend; the default remains Codex.
- The backend runs in the same active repo cwd as Codex and preflights
  `probeClaudeAgentReadiness()` before starting a session.
- TUI labels are `Claude` or `Claude (<model>)`, so Fable configuration is
  visible as the model instead of a separate adapter.
- Raw Claude SDK session ids stay local for resume; the feed shows only
  hashed session refs.

This does **not** implement the permissive `bypassPermissions` mode. That is
the next CL issue (#4845) and should use the same public-path rejection shape
as Codex dangerous mode.

## Open Issue Tail

Current issue tail after the #4838/#4839/#4840/#4841/#4842/#4843/#4844 implementation pass:

- #4786 parent epic: Autopilot MVP issue ladder.
- #4768 M10: overnight unattended proof, both lanes, both surfaces.
- #4772 M14: MVP exit review / door-open decision.
- #4777 P1: first live negotiated labor job on a real backlog issue.
- #4781 P5: backlog faucet for the open market.
- #4782 P6: spare-capacity provider mode.
- #4783 P7: Lane C fanout.
- #4749 W3: separate Tassadar/Psion research sweep, not an MVP dependency.

The issue tail now has two lanes:

1. **Owner supervised daily-driver lane:** one retained supervised proof using
   the source checkout. #4838, #4839, #4840, #4841, #4842, #4843, and #4844
   are implemented in source. This remains the ASAP switch path.
2. **Autopilot/public readiness lane:** #4768, #4772, #4777, #4781, #4782, and
   #4783. These are still real, but they are not required for the owner to sit
   at Pylon and supervise Codex.

The terminal-agent operational roadmap is still correct that Pack A, Pack B,
Pack C, and #4836/#4837 are implemented and closed, and Pack D should not be
filed yet. The correction is that "missing live evidence" is not one category:
overnight unattended and market settlement evidence are public/autonomous
readiness evidence, not local supervised daily-driver evidence.

2026-06-12 follow-up: #4844, #4845, and #4846 subsequently closed. The Claude
composer backend (#4844), Claude `local_supervised_danger` mode (#4845), and
Claude dev-doctor/context projection (#4846) are implemented and closed in
source. #4847 remains open as CL4, the retained supervised Claude/Fable
daily-driver proof. A Codex twin, #4860, was filed on 2026-06-12 for the same
retained supervised proof on the Codex lane, matching P0 item 7 of this audit.

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
    "enabled": true,
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

At this exact commit, that starts the TUI with a Codex SDK-backed composer in
the current working directory. To switch a session to Claude/Fable without
changing the default, launch with `--adapter claude`. `dev.codexExecutionMode`
in the config or `--codex-danger` makes the local dashboard composer use
`local_supervised_danger`:

```sh
pylon dev --codex-danger
pylon dev doctor --json --codex-danger
pylon dev fix "make this repo change"
```

The SDK path should continue to use `@openai/codex-sdk` directly:

```ts
const thread = await codex.startThread({ workingDirectory: activeRepo });
await thread.runStreamed(prompt);
```

For the supervised danger lane, Pylon should pass SDK options equivalent to
local unrestricted owner-watched execution: `sandboxMode: "danger-full-access"`
and `approvalPolicy: "never"`.

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

1. **Wire the composer/current repo to Codex (#4839). Done in source.**
   - Uses Codex as the default daily-driver backend.
   - Drives the active repo/current working directory through the TypeScript
     SDK.
   - Streams SDK events into the TUI feed.
   - Stops requiring OpenCode for the owner path.

2. **Add explicit local-only dangerous Codex mode (#4840). Done in source.**
   - Maps the supervised behavior to SDK `danger-full-access` /
     `approvalPolicy: "never"`.
   - Requires explicit opt-in through `--codex-danger` or local dev config.
   - Shows the mode in Pylon as `Codex DANGER` plus a feed status line.
   - Rejects the flag for public assignment/provider/market/headless/attach
     lanes.

3. **Add dev doctor/context projection (#4841). Done in source.**
   - Shows repo, branch/commit, dirty count, instruction/config digest refs,
     Codex/OpenAI readiness, Claude/Fable readiness, current execution mode,
     and backend refs.
   - Keeps output typed and redacted: no keys, auth paths, instruction text,
     changed filenames, or local absolute paths.

4. **Render repo/account/instruction context in the TUI (#4838). Done in
   source.**
   - Adds `pylon context --json` as the public-safe projection used by the
     TUI.
   - Adds a wide-dashboard `Repo & AI Context` pane beside telemetry.
   - Adds an `f6` context route and `Context: refresh repo & AI` command for
     narrow terminals.
   - Shows Codex DANGER, OpenAI/Codex source refs, Claude/Fable readiness,
     backend refs, current job refs, required capability refs, and blockers.

5. **Add dev check/apply/reload loop (#4842). Done in source.**
   - `pylon dev check --json [--allow-dirty]` shows a dirty-state summary,
     changed file refs, focused check command refs, exit codes, and output
     digest refs.
   - `pylon dev apply --json` records the current patch summary without
     committing or pushing.
   - `pylon dev reload --json` is explicit and currently returns a safe no-op
     unless a controlled process exists.
   - The TUI command palette exposes the same Dev check/apply/reload actions.

6. **Add Claude/Fable local composer selection (#4844). Done in source.**
   - `dev.defaultAdapter: "claude_agent"` or `--adapter claude` runs the
     Claude Agent SDK in the active repo.
   - The TUI labels the backend as `Claude` / `Claude (<model>)`, so
     `claude-fable-5` is visible when configured.
   - Readiness blockers are reported before SDK session start.
   - Raw session ids stay local for resume; public/feed output uses hashed
     refs.

7. **Run one retained supervised daily-driver proof.**
   - Use a real repo task while the owner watches.
   - Submit from the Pylon composer or `pylon dev fix`, not `pylon work submit`.
   - Codex edits in the active checkout.
   - Focused checks pass.
   - Pylon shows the patch/check/reload state.
   - Fable review is optional, not blocking.

8. **Install-pin v0.3 for the owner.**
   - Either publish `@openagentsinc/pylon@0.3.0` after release gates and npm
     credential repair, or define an owner-only install pin from the source
     checkout so the daily command does not depend on unpublished package
     semantics.

Work-order lane follow-up:

- Use source-built `pylon work submit "<objective>" --commit <sha> --adapter
codex|claude_agent|fable` for explicit work-order dogfood only; it is no
  longer the shortest path for the local daily-driver switch.
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

Do not yet use Pylon as the only daily-driver surface until one supervised
proof is retained and the owner install path is pinned. The composer, local
dangerous mode, doctor projection, and check/apply/reload loop now exist in
source, but they still need a real Pylon-from-Pylon proof run.

Once that proof is retained, the decision can become:

- **yes** for owner-supervised local Pylon + Codex daily work;
- **still no** for unattended overnight runs until #4768;
- **still no** for market/provider/paid public capacity until #4777/#4781/
  #4782/#4783 and #4772 complete;
- **limited yes** for source-built `pylon work submit` repo tasks that provide
  a real commit pin and explicit adapter intent, with the caveat that the
  packaged install and retained owner proof still need pinning.

Do not yet use Pylon as the default for:

- unwatched arbitrary repo tasks from packaged `pylon work submit`;
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

- #4839 implements the Codex composer/current-repo lane in source.
- #4840 implements the explicit local supervised dangerous Codex mode.
- #4841 implements the redacted dev doctor/context projection in source.
- #4842 implements the check/apply/reload loop.
- #4844 implements local Claude/Fable composer selection through the Claude
  Agent SDK.

### Shape

Command surface:

```sh
pylon dev
pylon dev --codex-danger
pylon dev doctor --json
pylon dev fix "the assignment view is not refreshing"
pylon dev fix --codex-danger "fix the TUI composer"
pylon dev fix --from-last-error
pylon dev check --json [--allow-dirty]
pylon dev apply --json [--allow-dirty]
pylon dev reload --json
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

`pylon dev doctor --json` now assembles the core redacted context bundle, and
`pylon context --json` projects the same repo/account/job state for the TUI:

- local package version and source commit;
- active repo provider/name, branch, commit, and dirty count;
- instruction and config digest refs;
- adapter readiness for Codex and Claude/Fable;
- active Codex execution mode and sandbox;
- backend readiness refs;
- current work/request/workspace/verification refs when available.

The visible `Repo & AI Context` pane now renders this projection on wide
dashboards and through `f6` on narrow terminals.

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
- `--adapter claude` or `dev.defaultAdapter: "claude_agent"` now gives the
  local composer a Claude/Fable implementation path; a dedicated
  `pylon dev review --adapter fable` command remains follow-up.
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

1. **DM1: Dev doctor. Done in source.** `pylon dev doctor --json` emits
   redacted local diagnostics and adapter readiness.
2. **DM2: Dev check/apply. Done in source.** `pylon dev check --json` and
   `pylon dev apply --json` record safe changed-file refs, dirty-state counts,
   command refs, exit codes, and output digest refs without committing or
   pushing.
3. **DM3: Dev reload. Source no-op landed.** `pylon dev reload --json` is an
   explicit no-op when there is no controlled process; a real restart-and-reattach
   path remains follow-up.
4. **DM4: Claude/Fable composer. Done in source for the composer path.**
   `dev.defaultAdapter: "claude_agent"` and `--adapter claude` select the
   Claude Agent SDK backend; dedicated review-command ergonomics remain
   follow-up.
5. **DM5: Fable review.** Add optional Claude/Fable review of the local patch
   summary and check refs.
6. **DM6: Retained proof.** Use Dev Mode to fix one real Pylon bug, retain the
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

The recommendation is now to retain one real owner-watched Dev Mode proof and
then decide whether the source checkout is enough for the owner's immediate
daily use, while package publication and the broader Autopilot proof gates
continue separately.

## 2026-06-21 Verse UI Issue Ladder Addendum

The Verse coding overlay audit now treats this Pylon/Codex substrate as the
starting point for UI work, not as future runtime discovery. The sequential
`VCODE-01` through `VCODE-16` ladder in
`2026-06-21-autopilot-verse-coding-agent-pane-overlay-audit.md` should be read
alongside this audit when planning agent-manageability work in Autopilot
Desktop Verse.

Relevant source state this audit feeds into the ladder:

- The desktop app already edits node-local `dev.accounts` entries for Codex
  and Claude Agent, with tests for add/list/remove, priority, duplicate
  rejection, and invalid refs.
- The composer already carries a selected Codex `accountRef` through
  `ClickedComposerSpawn` into the existing session-spawn command contract.
- Pylon already resolves Codex account refs and projects public-safe
  `accountRefHash` values into session evidence.
- The multi-session runbook already models multiple Codex accounts as named
  refs, not raw credential homes.

The main new UI obligation is therefore not "add multi-Codex account support"
from scratch. It is:

1. expose the existing Codex account inventory in Verse code mode;
2. let the owner add, remove, prioritize, and select Codex accounts from a
   focused code-mode pane;
3. keep the selected account visible before every Codex spawn;
4. sync account, session, event stream, transcript, readiness, quota, and
   diagnostic projections into one typed code-mode model;
5. prevent silent fallback when a selected Codex account is blocked; and
6. test the whole loop with multiple Codex accounts before extending the same
   UI contract to Claude Agent or Fable.

That sequencing keeps Codex first, makes multiple Codex accounts a front-line
manageability issue, and preserves the existing redaction boundary: normal UI
shows short labels and public-safe hashes, while raw homes, credentials, local
paths, provider payloads, and full diagnostic detail stay out of public or
default surfaces.
