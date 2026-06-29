# Pylon Claude Support Audit — Parity With Codex For Day-To-Day Coding

Date: 2026-06-12

## Question

The owner directive: add Claude support to Pylon equivalent to Codex. We have
some code in place. This audit describes the current system in full, names the
exact gaps between the Claude lane and the Codex lane, proposes the issue set
and end states, and decides to what extent terminal-agent-systems audit
material should be operationalized at the same time.

"Claude" here means the Claude Agent lane built on
`@anthropic-ai/claude-agent-sdk`. "Fable" means the `claude-fable-5` model
selected through that lane; there is no separate Fable adapter and this audit
recommends keeping it that way.

## Source Set

Read in this pass:

- GitHub issue #4786 (epic) with all comments, plus the open Codex/Pylon
  day-to-day issues #4838, #4841, #4842, #4843 and closed #4839/#4840.
- `docs/autopilot-coder/README.md`.
- `docs/autopilot-coder/2026-06-10-claude-agent-sdk-local-claude-pylon-audit.md`.
- `docs/autopilot-coder/2026-06-12-pylon-codex-day-to-day-readiness-audit.md`.
- `docs/autopilot-coder/terminal-agent-systems/2026-06-11-terminal-agent-systems-operationalization-roadmap.md`.
- `apps/pylon/docs/claude-agent-bridge.md`.
- Code-level parity sweep across `apps/pylon/src/claude-agent.ts`,
  `claude-agent-executor.ts`, `codex-agent.ts`, `codex-agent-executor.ts`,
  `codex-composer.ts`, `dev-doctor.ts`, `index.ts`, `tui/app.tsx`, the
  matching tests/smokes, and
  `apps/openagents.com/workers/api/src/autopilot-work-adapter-selection.ts` /
  `autopilot-work-pylon-assignment-synthesizer.ts`.

This is a docs-only audit. It does not create or change a product promise.

## Verdict

**The Claude lane is already the platform peer of Codex on the
assignment/work-order spine — and is in fact the dual-capability default. CL1
closed the largest local supervised gap by adding a Claude Agent SDK composer
backend; CL2 (#4845) added the local-only `bypassPermissions` supervised mode
with the same opt-in/public-path-rejection guardrails as Codex, and CL3
(#4846) projected the Claude execution mode through the dev doctor and the
`Repo & AI Context` pane. The remaining parity work is the retained
supervised proof (CL4 #4847), which needs an owner-watched run.**

Concretely:

- Assignment execution: full parity. Both lanes have a readiness probe,
  capability declaration, bounded executor, workspace guard, `git_checkout`
  support, CI-safe and live smokes, and production receipts. The Claude lane
  shipped first (#4717-#4720, #4755, #4756) and the Codex lane (CX1-CX5,
  #4788-#4792) was deliberately built as its peer behind the same gate.
- Server-side adapter selection: `DEFAULT_CODING_ADAPTER = claude_agent`. A
  dual-capability Pylon runs Claude unless requester intent says otherwise.
- Local supervised surface: no longer Codex-only for normal composer use.
  #4844 adds `claude-composer.ts`, `dev.defaultAdapter`, `--adapter claude`,
  Claude/Fable model labels, and local session resume. The remaining Codex-only
  parts are the `local_supervised_danger` execution mode, explicit danger flag,
  and doctor/context execution-mode projection.

So "add Claude support equivalent to Codex" is not a new bridge build. It is a
bounded parity pass on the local dev/composer loop. The first slice (#4844) is
implemented; the remaining slices are small follow-ups on permissive mode,
visibility, and proof, not new architecture or a new operationalization pack.

**Status (2026-06-12):** CL1-CL3 (#4844, #4845, #4846) are closed in source.
The Codex successor promise went green receipt-first on 2026-06-12 (transition
receipt `promise_transition_c2328369-09d0-4972-9a3b-8d530440cade`). The Claude
bridge remains yellow on the packaged-binary repeat (#4858/#4859).

## Current System

### Shared adapter architecture

Both lanes follow the same pattern, by design (the #4717 design audit and the
CX addendum to #4786 both required peer adapters behind one execution gate):

| Layer                | Claude                                                                 | Codex                                                                        |
| -------------------- | ---------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| SDK (optional, lazy) | `@anthropic-ai/claude-agent-sdk` (^0.3.172)                            | `@openai/codex-sdk` (^0.139.0)                                               |
| Readiness probe      | `probeClaudeAgentReadiness()` in `claude-agent.ts`                     | `probeCodexAgentReadiness()` in `codex-agent.ts`                             |
| Capability ref       | `capability.pylon.local_claude_agent`                                  | `capability.pylon.local_codex`                                               |
| Work class           | `claude_agent_task` (`openagents.pylon.claude_agent_task.v0.3`)        | `codex_agent_task` (`openagents.pylon.codex_agent_task.v0.3`)                |
| Agent kind           | `claude_agent_sdk`                                                     | `codex_sdk`                                                                  |
| Executor             | `claude-agent-executor.ts`                                             | `codex-agent-executor.ts`                                                    |
| Boundary enforcement | `allowedTools` + `PreToolUse` hook (`toolInputEscapesWorkspace`)       | SDK sandbox (`read-only`/`workspace-write`) + post-hoc file-change validator |
| Config section       | `claudeAgent: { enabled, model, maxTurns, timeoutSeconds }`            | `codex: { enabled, model, maxTurns, timeoutSeconds, sandboxMode }`           |
| Smokes               | `smoke:claude-agent-task`, `tests/claude-agent-task-smoke.test.ts`     | `smoke:codex-agent-task`, `tests/codex-agent-task-smoke.test.ts`             |
| Docs                 | `apps/pylon/docs/claude-agent-bridge.md`, `claude-agent-task-smoke.md` | `apps/pylon/docs/codex-bridge.md`, `codex-agent-task-smoke.md`               |
| Promise              | `pylon.local_claude_agent_bridge.v1`                                   | `autopilot.codex_probe_pylon_successor.v1` (via CX4)                         |

Both executors share the adapter-neutral workspace materializer, the same
fixture-repair and `git_checkout_verified` task shapes, the same
closeout/result ref naming convention, and the same typed refusal arms
(`blocker.assignment.<lane>_unavailable`, `..._workspace_escape_blocked`,
`..._test_failed`). The Agent Runtime Kernel (#4804-#4809) projects both
lanes' events through one versioned contract.

### Where the Claude lane is actually ahead

- **Credential sources: 6 vs 3.** Claude probes `ANTHROPIC_API_KEY`, Bedrock,
  Vertex, Azure Foundry, Anthropic-on-AWS, and — most important for the owner
  path — the local Claude Code session
  (`credential.source.claude_agent.local_claude_session`, presence-only via
  `~/.claude/.credentials.json` or the macOS keychain). A machine logged into
  Claude needs zero setup. Codex probes `CODEX_API_KEY`, `OPENAI_API_KEY`, and
  `codex login` state.
- **Finer tool control.** The Claude executor maps assignment
  `allowedToolKinds` onto an explicit `allowedTools` list (default
  `Read, Edit, Write, Bash, Glob, Grep`), excludes user settings via
  `settingSources: []`, and enforces the workspace boundary _before_ each tool
  call through a `PreToolUse` hook. Codex relies on the SDK sandbox plus
  post-hoc file-change validation.
- **Platform default.** `autopilot-work-adapter-selection.ts` declares
  `DEFAULT_CODING_ADAPTER = CLAUDE_AGENT_ADAPTER`; a dual-capability Pylon is
  assigned `claude_agent_task` with reason ref
  `adapter_selection.dual_capability_default`. Requester intent wins when
  present, but `pylon work submit` does not yet expose it (#4843).
- **Receipts.** Live local-session run #4755 (closeout
  `assignment.closeout.ae84ca67ada1584130b823d5`) and the API-submitted
  `git_checkout` proof #4756 (work order
  `autopilot_work_order.46dc8c38-04c5-4f1c-9814-f35bfc00e7c3`, closeout
  `assignment.closeout.2dc83bdc0d8481ebba14621e`, pinned commit
  `1745cd4b54b8a12a50922f80b5d345314c91d70d`).

### Where the Codex lane pulled ahead (the gap)

The #4839/#4840/#4841 P0 pass initially built a local supervised daily-driver
surface for Codex only. #4844 now adds the normal Claude/Fable composer path,
leaving the danger/permission and proof slices open:

1. **Composer backend. Closed by #4844.** `apps/pylon/src/claude-composer.ts`
   streams Claude Agent SDK `query()` messages into the TUI feed.
   `dev.defaultAdapter` and `--adapter claude` select that backend while Codex
   remains the default. The backend runs in the active repo cwd, preflights
   `probeClaudeAgentReadiness()`, labels `Claude` / `Claude (<model>)`, and
   keeps raw SDK session ids local for resume.
2. **Supervised dangerous mode.** `CodexComposerExecutionMode` is
   `"local_bounded" | "local_supervised_danger"`; the danger mode maps to SDK
   `sandboxMode: "danger-full-access"` + `approvalPolicy: "never"`, is gated
   behind `--codex-danger` or `dev.codexExecutionMode`, labels the TUI
   `Codex DANGER`, and is rejected on every public path with
   `blocker.codex.local_supervised_danger_public_path`. The Claude lane has no
   equivalent: `ClaudeAgentConfig` has no execution-mode or permission-mode
   field, and there is no `--claude-danger`.
3. **Dev doctor depth.** `dev-doctor.ts`
   (`openagents.pylon.dev_doctor.v0.3`) does include a `claudeAgent` section —
   readiness, configured model, and a `fableReviewAvailable` flag — but only
   the `codex` section carries `executionMode` and `sandboxMode`. The doctor
   can say "Fable review available"; it cannot say what mode a Claude dev run
   would execute in, because no such mode exists.
4. **Dev loop direction.** #4842 (check/apply/reload) and #4838 (TUI context
   pane) are written with Codex as the implementer and Claude/Fable as
   "optional reviewer". That is the owner's stated preference for work
   ordering, but the _mechanism_ in those issues should be adapter-agnostic so
   that a Claude-primary day works identically.

### The asymmetry table

| Surface                              | Codex                                               | Claude                                                    |
| ------------------------------------ | --------------------------------------------------- | --------------------------------------------------------- |
| Assignment executor                  | Built, live-proven                                  | Built, live-proven (first)                                |
| Capability declaration on go-online  | Yes                                                 | Yes                                                       |
| TUI composer backend                 | Yes (`codex-composer.ts`)                           | Yes (`claude-composer.ts`, #4844)                         |
| Supervised dangerous/permissive mode | Yes (`local_supervised_danger`)                     | **No**                                                    |
| Danger flag / dev config key         | `--codex-danger`, `dev.codexExecutionMode`          | **No**                                                    |
| Public-path rejection of danger mode | `blocker.codex.local_supervised_danger_public_path` | n/a (no mode exists)                                      |
| Dev doctor readiness                 | Yes + executionMode/sandboxMode                     | Readiness/model/Fable flag only                           |
| TUI backend label                    | `Codex` / `Codex DANGER`                            | `Claude` / `Claude (<model>)`; danger label still pending |
| Dual-capability work-order default   | —                                                   | **Yes (Claude wins)**                                     |
| CLI adapter intent on work submit    | Missing (#4843)                                     | Missing (#4843)                                           |
| Credential sources probed            | 3                                                   | 6 (incl. local Claude session)                            |

The punchline after #4844: the network lane defaults to Claude and the local
lane can now run Claude/Fable, but the local lane still lacks the
`bypassPermissions` equivalent of Codex danger mode and the retained
owner-watched proof.

## What "Equivalent To Codex" Requires

### 1. Claude composer backend behind a shared backend seam (implemented by #4844)

#4844 generalizes the composer factory so the dashboard/attach composer can be
backed by either lane:

- The `ComposerBackend` contract that `tui/app.tsx` already consumes is now
  selected through `makeComposerBackend()`.
- `claude-composer.ts` mirrors `codex-composer.ts`: stream
  `query()` SDK messages (assistant text, tool use, results) into the feed,
  run in the current working directory (`PYLON_CODEX_CWD` or
  `PYLON_ACTIVE_REPO` can override it), and surface typed SDK/auth readiness
  blockers from `probeClaudeAgentReadiness()` before any session starts.
- The backend is selected from `dev.defaultAdapter` (`codex` | `claude_agent`,
  default `codex` per the owner's stated working mode) with a per-launch
  override (`--adapter claude`, or a command-palette/TUI toggle).
- The pane labels `Claude` — by branding law, never `Claude Code` — and shows
  the configured model so a Fable session visibly reads `Claude (fable-5)` or
  equivalent.
- Multi-turn: the Claude Agent SDK has a real `resume`/session primitive; the
  composer keeps the session id locally (ref-only off-device) so
  follow-up prompts continue the conversation rather than starting cold. This
  is something the Codex composer thread model gets implicitly.

### 2. Local-only supervised Claude permissive mode (implemented by #4845)

The Codex danger mode maps to an OS-sandbox concept. The Claude Agent SDK's
equivalent control is the permission system, so the mapping is:

- `local_bounded` (default): current executor behavior — `allowedTools`
  allowlist, `PreToolUse` workspace guard, `settingSources: []`.
- `local_supervised_danger`: SDK `permissionMode: "bypassPermissions"`, no
  workspace-guard hook, full tool set, network-capable tools permitted. Same
  semantics as Codex `danger-full-access` + `approvalPolicy: "never"`: the
  owner is watching, the agent is unrestricted.

Guardrails must be byte-for-byte the same shape as #4840:

- Explicit opt-in only: `--claude-danger` (or a shared `--danger` once both
  lanes exist) and/or `dev.claudeExecutionMode: "local_supervised_danger"`.
- Visible TUI state: `Claude DANGER` label plus a feed status line
  (`mode: local_supervised_danger | permissions: bypassPermissions`).
- Rejected on every public path — `pylon work`, `pylon assignment`,
  `pylon provider`, `pylon node`, `pylon attach` — with a typed blocker
  (`blocker.claude.local_supervised_danger_public_path`).
- The assignment executor stays bounded; `loadClaudeAgentConfig()` must keep
  rejecting any permissive mode in the `claudeAgent` config section, exactly
  as `loadCodexAgentConfig()` rejects `danger-full-access`. Only a
  `loadClaudeDevConfig()` (or a unified dev-config loader) reads the local
  override.

One deliberate design decision to make here: in dev mode it is probably
_correct_ to load the repo's own `CLAUDE.md`/`.claude` settings
(`settingSources` including project) because the owner is operating on their
own checkout and wants their own instruction layers active — unlike the
delegated assignment lane, which rightly isolates with `settingSources: []`.
That decision should be recorded in the implementation issue rather than
inherited silently from the executor.

**As implemented (#4845):** the bounded composer keeps the executor-style
isolation it shipped with in CL1 (`allowedTools` allowlist, `acceptEdits`,
`settingSources: []`); the supervised danger mode loads
`settingSources: ["project"]` with no tool allowlist. The split is
deliberate: project instruction layers activate exactly when the owner has
explicitly opted into watching an unrestricted session on their own
checkout.

### 3. Dev doctor parity (implemented by #4846)

Extend the `claudeAgent` section of the dev-doctor projection to match the
`codex` section: active execution mode, effective permission posture, and
blocker refs for permissive-mode misuse. Keep the existing redaction law: no
keys, auth paths, instruction text, or absolute paths.

### 4. Make the dev loop adapter-agnostic (amend #4842, do not duplicate)

`pylon dev check/apply/reload` operates on the working tree after an agent
edit; nothing in it is Codex-specific. The issue should be amended (comment,
not rewrite) so acceptance reads "after a dev composer/fix run by any local
adapter" and the recorded dev-run metadata carries the adapter that produced
the patch. `pylon dev fix --adapter claude` then falls out naturally, and
`pylon dev review --adapter fable` (the DM4 Fable-review milestone from the
Codex readiness audit) consumes the same change-capture surface.

### 5. Adapter intent end to end (already filed as #4843)

#4843 already specifies `--adapter codex|fable|claude_agent` on
`pylon work submit`, carried through request body, validation/planning,
assignment synthesis, and runner selection, with Fable as a Claude Agent
model profile (`claude-fable-5`). Nothing to add except one clarification
worth posting on the issue: the server-side `selectCodingAdapter` already
honors requester intent and already defaults dual-capability placements to
Claude, so #4843 is purely a transport/CLI gap, not a policy gap. The owner's
"Codex primary" preference should be expressed as the owner's local default
(`dev.defaultAdapter: "codex"` and/or explicit `--adapter codex`), not by
flipping `DEFAULT_CODING_ADAPTER` — other requesters may reasonably want the
platform default to remain Claude.

### 6. TUI context pane (#4838 closed; Claude mode added by #4846)

#4838 already requires Claude/Fable readiness, credential source refs, model
config (`claude-fable-5`), selected primary adapter, and fallback adapter in
the `Repo & AI Context` pane, using `probeClaudeAgentReadiness()`. The only
amendment needed once item 2 lands: the pane must show the Claude execution
mode with the same prominence as the Codex one (a `Claude DANGER` state must
be impossible to miss).

### 7. One retained supervised Claude proof

Mirror of the Codex switch criterion: the owner types a real repo request
into Pylon with the Claude backend selected (ideally model `claude-fable-5`),
Claude edits the active checkout, focused checks pass, Pylon shows the
patch/check summary, and the owner reloads or continues without leaving
Pylon. Retain refs locally; public claims stay ref-only. Until this exists,
copy must say the Claude composer lane is built, not proven.

## Suggested Issues

New issues (CL lane, mirroring the CX convention):

| ID  | Priority | Title                                                                                         | Acceptance sketch                                                                                                                                                                                                                                                                                                             |
| --- | -------- | --------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CL1 | P0       | Make the Pylon composer run Claude in the current repo behind an adapter-neutral backend seam | Implemented in source by #4844: `claude-composer.ts` streams SDK messages into the TUI; `dev.defaultAdapter`/`--adapter` selects backend; readiness blockers pre-session; session continuity across prompts; `Claude` label + model shown; tests mirror the Codex composer coverage                                           |
| CL2 | P0       | Add local-only supervised permissive Claude mode                                              | Implemented in source by #4845: `--claude-danger`/`dev.claudeExecutionMode` maps to `permissionMode: "bypassPermissions"` on the local composer path only; `Claude DANGER` label; all five public command paths reject with `blocker.claude.local_supervised_danger_public_path`; assignment config never reads permissive modes; tests prove accept-local/reject-public and the settingSources split |
| CL3 | P0       | Dev doctor and context pane show Claude execution mode                                        | Implemented in source by #4846: `claudeAgent` doctor section gains `executionMode`, `permissionMode`, and `dangerPublicPathBlockerRef`; `pylonConfig.claudeDevOverlayRef` names the dev overlay; the context pane shows `Claude DANGER` with Codex-equal prominence and error-colored text; redaction/fixture tests extended |
| CL4 | P1       | Retained supervised Claude/Fable daily-driver proof                                           | One real owner-watched repo task through the Claude composer (model `claude-fable-5`), focused checks pass, patch/check/reload state shown in Pylon; local refs retained; copy updated from "built" to "proven"                                                                                                               |

Amendments to existing open issues (comments, not new issues):

- **#4842**: make the check/apply/reload contract adapter-agnostic; record
  the producing adapter on each dev run; add `pylon dev review --adapter
fable` as the consumer of the same change capture.
- **#4843**: note that server-side requester-intent and the Claude
  dual-default already exist; the issue is CLI/transport only; the owner's
  Codex-primary preference lands as local default config, not as a
  `DEFAULT_CODING_ADAPTER` flip.
- **#4838**: after CL2, require the Claude execution mode in the pane with
  the same prominence as Codex.

Deliberately **not** proposed:

- No `fable_agent_task` work class or separate Fable adapter. Fable remains a
  model profile on the Claude lane (`claudeAgent.model: "claude-fable-5"`),
  exactly as both prior audits concluded. A dedicated adapter would duplicate
  the bridge for zero capability gain and complicate the capability/promise
  story.
- No change to the dual-capability default or to capability refs.
- No new promise until the CL4 proof exists; the existing
  `pylon.local_claude_agent_bridge.v1` copy boundary already covers the
  bridge.

## End States

- **E1 — Adapter-symmetric supervised daily driver.** The owner sits at
  Pylon and chooses per-prompt or per-session between Codex and Claude/Fable
  in the same composer, with the same danger opt-in semantics, the same
  doctor/context visibility, and the same check/apply/reload loop. Codex
  primary and Fable review is a configuration, not an architecture.
- **E2 — Adapter intent end to end on the work-order lane.** `pylon work
submit --adapter ...` (with real commit pinning, #4843) flows requester
  intent through synthesis to runner selection; the dual-capability default
  remains Claude for intent-less requests.
- **E3 — Fable as first-class reviewer.** `pylon dev review --adapter fable`
  reviews the local patch summary and check refs through the Claude lane when
  `claude-fable-5` readiness is green, degrading to a typed blocker —
  never a silent model substitution.
- **E4 — Unattended parity (post-#4768).** Once the overnight unattended
  proof gate closes, both lanes run under the same Pack A
  supervision/receipt contracts; nothing in this parity pass should create a
  Codex-only or Claude-only operational surface that M10 evidence would then
  have to special-case.
- **E5 — Market capacity (post-#4777/#4781-#4783).** A provider Pylon
  declaring `capability.pylon.local_claude_agent` serves market work under
  Pack B credential/policy evidence. Already structurally true; no new work
  in this pass.

## How Much Terminal-Agent-Systems Material To Build Simultaneously

Short answer: **consume, don't expand.** Do not file a new operationalization
pack for Claude parity.

The operationalization roadmap's standing decision (reaffirmed three times on
2026-06-12) is that Pack A/B/C are implemented and closed, Pack D waits for
the #4768/#4772 proof gates, and the missing ingredient across the program is
live evidence, not issue decomposition. Claude parity work changes none of
that calculus: every slice above lands inside contracts those packs already
hardened. The right relationship is citation:

- **Pack B (#4825-#4830)** — CL1/CL2 touch credentials (six Claude credential
  sources, including a local session probed via keychain). New surfaces must
  represent credentials as source refs through the existing credential
  boundary; the security-review record (#4827,
  `2026-06-11-provider-peer-security-review.md`) and the ToS review apply
  directly to the Anthropic lane (BYOK only; no claude.ai login brokering;
  the 2026-06-15 Agent SDK subscription-credit change belongs in cost-honesty
  copy).
- **Pack A (#4813-#4823)** — the dev loop and any scheduled/background Claude
  runs must emit the same task/notification/smoke/usage receipts. The CL4
  proof should be shaped as a Pack A-style smoke with a stated boundary, not
  prose.
- **Pack C (#4831-#4835)** — `pylon dev check`'s change capture and any
  delivery claims cite repo/worktree identity and change-capture refs rather
  than inventing parallel records.

Specific subsystem audits worth reading during implementation (thin-slice
consumption, per the roadmap's Pack E exception rule):

- Model Provider Abstraction — the composer backend seam is exactly this
  audit's boundary; adapter labels and model/config refs should follow it.
- Permission And Approval — `local_supervised_danger` for Claude is a
  permission-posture change and must land as a typed policy decision, not a
  boolean buried in config.
- Authentication And Credential Storage — local-session/keychain detection
  stays presence-only; ref vocabulary already exists.
- Settings And Configuration — `dev.defaultAdapter`, `dev.claudeExecutionMode`
  and the `settingSources` decision belong in the effective-config snapshot
  surface (PB2).
- Conversation/Query and Structured Event Log — Claude composer streaming
  should project through the Agent Runtime Kernel contract like the
  assignment lane already does, so the TUI feed is a projection, not a
  bespoke pipe.

The roadmap's operationalization rule binds every slice here: if it changes
work state, touches credentials, mutates files, asks for approval, or
supports public copy, it lands as a typed event, policy decision, artifact,
receipt, or projection — terminal text is not operationalized. That rule, not
a new pack, is the simultaneous build.

## Boundaries Restated

These are inherited, not new, but every CL issue must carry them:

- **BYOK always.** The user's Claude credentials or local session; no
  platform-supplied or brokered access. Env key takes precedence over local
  session when both exist.
- **Branding/copy law.** "Claude Agent", "your local Claude", "Powered by
  Claude" — never "Claude Code" in product copy. Fable surfaces as the
  configured model name.
- **Cost honesty.** The user pays for their own inference; subscription Agent
  SDK usage draws from the separate monthly Agent SDK credit (effective
  2026-06-15).
- **Redaction law.** Raw SDK messages, prompts, session JSONL, file contents,
  provider payloads, and local paths never leave the device;
  `assertPublicProjectionSafe` guards everything that crosses the boundary.
- **Authority unchanged.** Composer/dev output is not accepted work; no
  settlement, payout, deploy, spend, or Forum publication authority; the
  danger mode is local-only and rejected on all public paths.

## One-Sentence Truth

Pylon's Claude lane already matches Codex on the delegated work-order spine
and is the platform's dual-capability default; #4844 adds the missing local
Claude/Fable composer, so the remaining equivalence work is an explicit
`bypassPermissions` dev mode with the same opt-in/rejection guardrails, dev
doctor/context-pane visibility, adapter-agnostic review ergonomics, and one
retained owner-watched proof, built by citing the already-closed Pack A/B/C
contracts rather than filing any new operationalization pack.
