# Codex Roadmap

> Status: updated 2026-03-10 after auditing `~/code/t3code` against the current
> OpenAgents Codex wrapper, after re-reading `docs/MVP.md` and
> `docs/OWNERSHIP.md`, after re-checking the current desktop lane, pane, and
> turn-input surfaces on `main`, and after landing `CX-6` on `main`.
>
> This is the live roadmap for Codex product work in OpenAgents Desktop. The
> goal is not to clone T3 Code's TypeScript server/web/Electron architecture.
> The goal is to adapt the highest-leverage product ideas around Codex while
> keeping OpenAgents desktop-first, Rust-first, replay-safe, and truthful about
> wallet/provider state.

Agent execution instruction: implement this roadmap one slice at a time in the
recommended dependency order below. Keep protocol types and app-server
compatibility in `crates/codex-client`. Keep Codex product workflows, remote
access control, workspace state, plan artifacts, and diff artifacts in
`apps/autopilot-desktop`. Do not move product-specific orchestration into
`crates/wgpui`.

Roadmap hygiene rule: after each shipped roadmap slice, update this document so
it reflects the new truthful state on `main`. Do not leave the roadmap stale.

Primary reference: the comparison basis for this roadmap is
`docs/audits/2026-03-10-t3code-codex-wrapper-gap-audit.md`.

## Objective

Make Autopilot good enough to replace the user's day-to-day `codex` CLI/TUI
workflow on the same machine, while also borrowing the strongest workbench ideas
from T3 Code.

That means the roadmap must cover three layers, not just one:

- Codex engine truth: auth, readiness, protocol coverage, approvals, config,
  models, skills, MCP, apps
- Codex interactive workflow parity: session lifecycle, composer behavior,
  queued follow-up turns, review/diff/compact/plan flows, status/config
  visibility, and non-interactive `exec`-style automation
- coding-shell product parity: workspace identity, git/worktree/PR workflow,
  terminal workflow, durable artifacts, and remote access into the same desktop
  runtime

This is not a plan to weaken the MVP promise in `docs/MVP.md`. The Codex
surface should make Autopilot more useful and more operable on the same machine;
it must not displace wallet truth, provider truth, or the earn loop.

## Ownership Rules

This roadmap must keep `docs/OWNERSHIP.md` intact:

- `crates/codex-client` owns reusable Codex protocol types, method coverage,
  normalization, and conformance truth.
- `apps/autopilot-desktop` owns Codex product behavior, workspace identity,
  plan/diff persistence, remote access, terminal/git orchestration if added,
  and final UX wiring.
- `crates/wgpui` owns product-agnostic UI primitives only.
- Remote surfaces must consume app-owned state derived from desktop truth; they
  must not become a second source of truth for threads, plans, workspace state,
  provider state, or wallet state.

## Product Rules

- Desktop remains the canonical runtime.
- Remote means "access the same local machine from another device", not "hosted
  OpenAgents in the cloud".
- Remote must be opt-in, disabled by default, authenticated, and safe on LAN or
  Tailnet-style networks.
- Provider and wallet state must remain explicit and authoritative.
- Every new Codex feature must degrade honestly when prerequisites are missing.
- Prefer narrow, verifiable slices over broad architecture expansion.

## Replacement Standard

Autopilot does not replace daily `codex` use until a user can stay inside
OpenAgents for the full normal loop on the same machine:

- start, resume, fork, rename, archive, unarchive, list, and inspect threads
- submit a turn, interrupt it, and queue or steer a follow-up while a turn is
  already running
- choose model, reasoning effort, fast/service tier, personality, collaboration
  mode, approval mode, and sandbox mode
- see current session truth: cwd/workspace, branch, dirty state, model,
  permissions, auth status, and token usage
- mention files, attach images, reuse skills, and recover drafts/history
- run review, diff, compact, and plan flows without dropping to the Codex TUI
- use git/worktree/terminal flows needed for real coding work
- use MCP tools, apps/connectors, approval prompts, tool user-input prompts,
  and auth refresh flows
- run one-shot automated tasks comparable to `codex exec`
- optionally supervise and continue that same local session from another device

If a user still needs to keep the Codex TUI open for core session control,
coding-shell operations, or scripted one-shot tasks, the replacement bar has not
been met.

## Why Remote Belongs

Remote access fits the desktop-first MVP when framed correctly:

- the repo, toolchains, local files, Codex auth, wallet, and provider runtime
  stay on the user's machine
- the remote surface is only a control and observation layer into that machine
- the user can continue a thread, answer approvals, or monitor earning state
  from a phone, tablet, or second laptop without moving execution or custody to
  a hosted service

This is the part of T3 Code's remote story worth adapting. The correct model
for OpenAgents is a personal remote companion, not a public multi-user web app.

## What To Adapt From T3 Code

The strongest ideas to borrow are product ideas, not architecture:

- startup readiness checks for Codex install/auth/version truth
- durable proposed-plan artifacts with an explicit "implement this plan" action
- better diff visibility than a raw transient patch blob
- richer turn input: image attachments and workspace/file mentions
- minimal workspace identity around each Codex thread
- remote access to the same local runtime over an authenticated web surface

## What Not To Copy Blindly

- Do not recreate T3 Code's server + web + Electron split as our default
  architecture.
- Do not add a broad web-first product branch with parity requirements across
  desktop and browser.
- Do not pull project/workspace orchestration into `crates/wgpui`.
- Do not expose wallet send/withdraw or other high-risk actions on remote-v1.
- Do not treat public-internet exposure as an MVP requirement.
- Do not build multi-provider placeholder UX before we have a real second
  provider.

## Shipped On Main

OpenAgents already has real strengths that this roadmap should build on rather
than replace:

- broad Codex app-server method coverage in `crates/codex-client`
- protocol-conformance coverage in `crates/codex-client/tests`
- a wide desktop lane in `apps/autopilot-desktop/src/codex_lane.rs`
- dedicated Codex panes for account, models, config, MCP, apps, labs, and
  diagnostics
- explicit handling for approvals, tool calls, tool user input, and auth
  refresh
- richer OpenAgents-native dynamic tool bridging than T3 Code
- live `turn/plan/updated` and `turn/diff/updated` handling in desktop state
- `CX-1` landed on `main` in `8146e1f09`: install/version probing in
  `crates/codex-client`, lane snapshot propagation, unified readiness refresh,
  and desktop config-constraint summaries derived from `config/read` layers plus
  `configRequirements/read`
- `CX-2` landed on `main`: chat-header session controls now cover
  model/effort/service-tier/personality/collaboration/approval/sandbox, the
  desktop lane now preserves thread-start/resume session truth from Codex
  responses, and Autopilot shows a compact always-visible coding status summary
  with workspace/git/auth/token context
- `CX-3` landed on `main`: the chat rail now exposes refresh/search/filter
  controls plus explicit thread lifecycle actions, thread history rows now show
  preview metadata instead of raw ids alone, and desktop caches/restores
  per-thread transcripts so resume/read/copy flows stop feeling lossy
- `CX-4` landed on `main`: the composer now assembles text, mentions, local or
  remote images, and skill attachments in deterministic order, per-thread draft
  state and submission history are restored across thread switches, and sending
  while a turn is active uses `turn/steer` so queued follow-up prompts keep the
  live task moving instead of forcing a second pending turn
- `CX-5` landed on `main`: the desktop now keeps the latest per-thread plan
  artifact with explanation, steps, source turn id, timestamp, and workspace
  context; restores it from `thread/read` plan items after reconnect; renders a
  compact plan summary in chat; and exposes an explicit implement action that
  turns the saved plan into a same-thread follow-up or steer prompt
- `CX-6` landed on `main`: desktop chat now exposes first-class `Review changes`
  and `Compact thread` actions, persists diff/review/compaction artifacts in an
  app-owned local projection, restores review/compaction context from
  `thread/read`, and renders compact latest-review/latest-diff/latest-compact
  sections directly in the main coding transcript
- `CX-7` landed on `main`: Codex threads now carry explicit workspace root,
  project id/name, and honest git branch + dirty/clean state in app-owned
  desktop state; the chat surface shows that identity in thread rows, status,
  and saved plan/diff artifacts; `Open ws` opens the active project workspace;
  and the desktop keeps a minimal per-project defaults registry for new threads
- `CX-10` landed on `main`: desktop chat now exposes app-owned `/skills`,
  `/mcp`, `/apps`, `/requests`, and `/approvals` control flows; remote skills
  are cached in app state instead of being discarded; MCP/apps summaries and
  request-queue responses no longer require falling back to Codex TUI panes for
  the common operator loop; and the chat operator surface now advertises and
  counts pending approvals, tool prompts, and auth-refresh requests more
  honestly

Many of the required primitives already exist in protocol or lane form. The
roadmap is therefore primarily about productizing those primitives into a
coherent desktop coding workflow instead of treating them as hidden capability.

The roadmap below is about product-layer gaps around that existing strength.

## Current Honest Gaps

These are the current gaps that matter most:

- we do not expose many already-supported lane capabilities as a coherent
  operator workflow in desktop chat; this is especially true for coding-shell
  actions beyond the current chat/session/review/compact surface
- we do not yet have the T3 Code-class coding shell: branch/worktree/PR
  controls, thread-scoped PTY terminals, or checkpointed coding artifacts
- we do not yet have an app-owned non-interactive `exec`-style surface for
  one-shot automated tasks
- we do not have personal remote access into the desktop runtime

## Not Gating Initial Replacement

The following Codex/TUI features are useful but should not block the first
"Autopilot replaces Codex CLI for daily coding" milestone:

- realtime voice mode and microphone/speaker settings
- theme picker and status-line customization UI
- feedback/log upload UI
- debug-only commands such as rollout-path printing or test-approval
- Windows-only sandbox elevation setup flows on non-Windows hosts
- memory-debug commands

We still need honest equivalents or explicit omissions for those later, but they
are not the critical path for replacing normal coding sessions.

## Current Execution Queue

The queue below is ordered by dependency and by what is required to replace the
Codex CLI/TUI in practice, not just by protocol completeness.

### Phase A. Codex Session Parity

### CX-1. Codex Readiness, Auth, And Config Truth ([#3357](https://github.com/OpenAgentsInc/openagents/issues/3357))

Status:

- shipped on `main` in `8146e1f09`
- desktop now exposes install presence, resolved invocation, version,
  account/login state, rate limits, effective approval/sandbox config, and
  managed-constraint explanations in one Codex operator surface
- config truth is derived from real `config/read` layers plus
  `configRequirements/read`, so disallowed selections are explained instead of
  just hidden behind the effective fallback

Next:

- `CX-12` is now the first open remote-companion item for Codex replacement
  work

### CX-2. Session Controls And Status Parity ([#3358](https://github.com/OpenAgentsInc/openagents/issues/3358))

Status:

- shipped on `main`
- desktop chat now exposes session controls for model, reasoning effort,
  service tier, personality, collaboration mode, approval mode, and sandbox
  mode directly in the main coding flow
- thread start/resume responses now feed back real Codex session truth into
  desktop state so the header/status rail reflects actual model, permissions,
  cwd, and reasoning state rather than stale local assumptions

Scope:

- expose model selection, reasoning effort, fast/service tier, personality, and
  collaboration mode controls in the main coding flow
- expose approval mode and sandbox mode controls in the same session-control
  layer
- show a compact always-visible status summary with model, effort, cwd/workspace,
  branch, auth, permissions, and recent token usage
- provide a clear desktop equivalent for the core Codex TUI controls behind
  `/model`, `/fast`, `/personality`, `/plan`, `/approvals`, and `/status`

Acceptance:

- a user can change session behavior from desktop without falling back to the
  Codex TUI
- session state changes are persistent, truthful, and reflected in subsequent
  turns

### CX-3. Thread Lifecycle And History Parity ([#3359](https://github.com/OpenAgentsInc/openagents/issues/3359))

Status:

- shipped on `main`
- desktop chat now exposes a real thread history strip with refresh/search,
  archived/source/provider/sort filters, and explicit thread lifecycle actions
  for fork/archive/unarchive/rename/read/copy/rollback/unload
- thread rows now show preview text and thread metadata is visible in the main
  header, including status, load state, and update timestamp
- active thread transcripts are cached per thread so switching around history
  and reloading from `thread/read` no longer blanks the conversation unless the
  thread truly has no transcript yet

Scope:

- productize thread start, resume, fork, rename, archive, unarchive, read, and
  list into a coherent desktop workflow
- provide a resume/history picker that is good enough to replace Codex `/resume`
- support output copy, thread metadata inspection, and clean transcript recovery
- keep thread list behavior fast and legible enough for daily use

Acceptance:

- a user can manage Codex threads in Autopilot instead of the Codex TUI
- resume/fork/rename flows no longer feel like hidden protocol features

### CX-4. Composer Parity And Queued Follow-Up Turns ([#3361](https://github.com/OpenAgentsInc/openagents/issues/3361))

Status:

- shipped on `main`
- desktop chat now parses `/mention PATH [| LABEL]` and `/image PATH|URL`
  directives into Codex `Mention`, `LocalImage`, and `Image` inputs while
  keeping skill attachments deterministic and validated
- composer drafts are tracked per thread plus detached state, restored on
  thread switches, and the last submitted prompt is preserved for recovery when
  a turn-start or steer submission is rejected
- when a turn is already active, sending from the composer now routes through
  `turn/steer` and appends the accepted follow-up prompt into the live
  transcript without creating a second assistant placeholder

Scope:

- extend turn assembly to support local image attachment and mention attachment
- add deterministic ordering and validation for image, mention, and skill inputs
- add draft/history recovery comparable to normal interactive use
- expose queued follow-up turns or steer behavior using `turn/steer` while a
  turn is already in progress
- preserve multiline paste and attachment behavior correctly

Acceptance:

- `TurnStartParams.input` can include text, skills, images, and mentions from
  desktop chat
- a user can continue steering a live task instead of waiting for completion
- this closes the functional gap behind `/mention` and the Codex queued-message
  workflow

### Phase B. Coding Artifact Parity

### CX-5. Durable Proposed-Plan Artifacts And Implement Handoff ([#3362](https://github.com/OpenAgentsInc/openagents/issues/3362))

Status:

- shipped on `main`
- the latest plan is now stored per thread as a desktop-owned artifact with
  explanation, structured steps, source turn id, timestamp, and workspace
  context
- thread selection and `thread/read` restore the latest saved plan so the
  artifact survives reconnects and thread history reloads instead of living only
  in transient turn-stream state
- the chat pane now shows a compact latest-plan summary and exposes an explicit
  `Implement plan` action that converts the saved artifact into a same-thread
  follow-up turn or active-turn steer prompt

Scope:

- persist the latest proposed plan for a thread as a first-class app object
- store explanation, steps, source turn id, timestamp, and workspace context
- show a compact plan card or pane entry in desktop
- add an explicit "implement this plan" action that converts the artifact into a
  follow-up turn on the same thread
- add plan-mode-specific follow-up UX rather than treating plans as transient
  stream text

Acceptance:

- the latest plan survives restart and reconnect
- the user can reopen and act on the plan after the original turn scrolls away

### CX-6. Review, Compact, And Structured Diff Parity ([#3363](https://github.com/OpenAgentsInc/openagents/issues/3363))

Status:

- shipped on `main`
- desktop chat now exposes first-class `Review changes` and `Compact thread`
  actions in the main coding header instead of leaving review behind the Codex
  labs pane
- the latest diff is now a durable per-thread artifact with source turn id,
  file list, added/removed counts, raw diff body, and restart-safe local
  persistence
- review output and compaction events are now durable desktop artifacts,
  `thread/read` restores review/compaction context when Codex can provide it,
  and the transcript renders compact latest-review/latest-diff/latest-compact
  summaries directly above the conversation

Scope:

- add a first-class review workflow equivalent to Codex `/review`
- add manual compact flow equivalent to Codex `/compact`
- persist per-turn diff artifacts instead of only transient text
- derive at least: file list, added/removed line counts, raw diff body, and
  source turn id
- show compact review/diff summaries in desktop

Acceptance:

- the user can run review, compact, and diff workflows without dropping to the
  Codex TUI
- the latest diff and review context survive restart

Later extension within this lane:

- thread-scoped checkpoint capture
- git hidden-ref capture and revert
- full-thread diff rollups

### Phase C. Coding Shell Parity

### CX-7. Workspace And Project Identity ([#3364](https://github.com/OpenAgentsInc/openagents/issues/3364))

Status:

- shipped on `main`
- desktop chat threads now carry explicit workspace root, project id/name, and
  honest git branch + dirty/clean state in app-owned state instead of deriving
  that ad hoc in the view layer
- the desktop now builds a minimal per-workspace project registry with
  project-level defaults, uses that registry when starting new threads in the
  same project, shows project/workspace identity in chat status + thread rows,
  and stamps saved plan/diff artifacts with the same workspace context
- the chat rail now exposes `Open ws`, which opens the active project workspace
  in a configured editor when available and otherwise falls back to the
  platform default opener

Scope:

- attach an explicit workspace root to each Codex thread
- introduce a minimal project registry above raw threads
- expose current branch and dirty/clean status when the workspace is a git repo
- support open-in-editor and project-level defaults where appropriate
- show workspace identity in desktop chat context, diff artifacts, and plan
  artifacts

Acceptance:

- a thread truthfully reports which workspace and project it operates on
- non-git workspaces degrade honestly

### CX-8. Git, Branch, Worktree, And PR Workflow Parity ([#3365](https://github.com/OpenAgentsInc/openagents/issues/3365))

Status:

- shipped on `main`
- desktop chat now recognizes app-owned `/git ...` and `/pr prep` commands for
  status, pull, repo init, branch list/create/checkout, worktree list/add/remove,
  and PR summary generation directly inside the main coding transcript
- worktree creation now updates the active thread workspace context so new
  threads inherit the branch/worktree they were spun up for, and local command
  results are preserved in the thread transcript just like normal coding turns
- PR prep now emits a desktop-owned summary with suggested title/body, compare
  URL when `origin` is GitHub, current status, commit list, and diff stat so
  branch-to-PR handoff stays inside Autopilot instead of bouncing back to the
  Codex CLI or external shell

Scope:

- add git status, pull, branch list/create/checkout, and repo-init workflows
- add worktree create/remove and worktree-aware thread context
- add PR-prep actions comparable to T3 Code's coding shell
- support branch/worktree context preservation when opening new threads in the
  same project

Acceptance:

- the user can do the normal branch/worktree loop in Autopilot instead of
  dropping to external tools for every state transition
- this closes the most important T3 Code workbench gap

### CX-9. Thread-Scoped Terminal And Background Command Lane ([#3366](https://github.com/OpenAgentsInc/openagents/issues/3366))

Status:

- shipped on `main`
- desktop chat now owns a per-thread shell session lane with `/term open`,
  `/term write`, `/term resize`, `/term clear`, `/term restart`, and
  `/term close`, with background output pumped into app state instead of being
  confused with Codex protocol items
- active threads now render a dedicated terminal card above the transcript so
  shell output stays visible and thread-scoped while the user keeps working in
  the same chat surface
- `/ps` now lists terminal sessions and `/clean` removes inactive ones, while
  `Cmd/Ctrl+Shift+T` opens or restarts the active thread terminal without
  leaving the chat composer flow

Scope:

- app-owned PTY session tied to a workspace or thread
- open, write, resize, clear, restart, close
- background terminal list/cleanup equivalents for the Codex `/ps` and `/clean`
  workflow
- desktop-first keybindings and focus rules

Acceptance:

- a thread can own a terminal session without confusing it with Codex protocol
  state
- a user can keep terminal-oriented coding work inside Autopilot

### Phase D. Integrations And Automation Parity

### CX-10. Skills, MCP, Apps, And Request-Flow Parity ([#3367](https://github.com/OpenAgentsInc/openagents/issues/3367))

Status:

- shipped on `main`
- desktop chat now exposes app-owned skills, remote-skill export/list, MCP,
  apps, and request-flow commands directly in the main coding surface instead
  of requiring separate Codex panes as the primary operator path
- remote skills are now retained in desktop state and surfaced in both chat and
  the skill registry, so `skills/remote/list` and `skills/remote/export` stop
  being hidden lane capability
- pending approvals, tool prompts, and auth-refresh requests are now counted
  and advertised more coherently in the chat shell itself

Scope:

- make skills attach/list/config workflows easy in the main coding flow
- keep existing remote-skill support and surface it cleanly
- expose MCP status, OAuth login, reload, and tool visibility clearly
- expose apps/connectors in a way that matches the useful part of Codex `/apps`
- keep approvals, tool user input, auth refresh, and request-permissions-style
  prompts coherent in desktop state

Acceptance:

- a user can rely on skills, MCP, apps, and approval flows from Autopilot
  without needing the Codex TUI as a fallback control panel

### CX-11. Non-Interactive `exec` Replacement ([#3368](https://github.com/OpenAgentsInc/openagents/issues/3368))

Status:

- shipped on `main`
- `autopilot-codex-exec` now provides an app-owned one-shot Codex runner with
  prompt/stdin input, image attachments, thread resume-by-id, sandbox and
  approval selection, optional ephemeral threads, output-schema forwarding, and
  final-message file output
- the binary now emits stable JSONL automation events with familiar Codex event
  names such as `thread.started`, `turn.completed`, and `item.completed`,
  while preserving the underlying app-server item payload in each emitted item
- the public usage contract for local scripts now lives in
  `docs/codex/EXEC.md`

Scope:

- provide an app-owned one-shot Codex execution surface comparable to
  `codex exec`
- support prompt input, optional ephemeral mode, and structured event output
- emit machine-readable event streams for automation, comparable to Codex JSONL
  events such as `thread.started`, `turn.completed`, and item lifecycle events
- document how this surface should be used from local scripts or automation

Acceptance:

- a user can replace normal `codex exec ...` habits with an OpenAgents-owned
  equivalent
- structured outputs are stable enough for local automation and regression tests

### Phase E. Personal Remote Companion

### CX-12. Personal Remote Companion V1 ([#3369](https://github.com/OpenAgentsInc/openagents/issues/3369))

Status:

- shipped on `main`
- desktop now owns an opt-in authenticated Codex remote companion listener in
  `apps/autopilot-desktop`, with safe bind validation for loopback, RFC1918
  LAN, Tailnet-style CGNAT, and IPv6 ULA targets
- `/remote`, `/remote enable [ip:port]`, `/remote disable`, and
  `/remote rotate-token` now manage the listener from the main chat surface,
  while Codex Labs shows the live base URL, pairing URL, and remote token
  preview
- the served remote UI now projects desktop-owned thread, transcript,
  approval/tool-prompt, readiness/session, plan, diff, wallet, and provider
  truth over authenticated snapshot/action endpoints instead of creating a
  second browser-side state authority

Next:

- `CX-13` extends the remote snapshot with workspace/git context and terminal
  visibility for away-from-desk supervision

Scope:

- remote access is disabled by default
- when enabled, the desktop app can bind to loopback, a LAN IP, or a
  Tailnet-style IP
- the desktop generates and rotates an auth token
- the desktop shows a copyable URL and QR-friendly pairing string
- the remote UI can show:
  - thread list and current thread
  - message history for the active thread
  - pending approvals and tool user-input requests
  - Codex readiness and session-status state
  - current plan artifact and latest diff artifact
  - wallet balance and provider online/offline truth
  - a narrow follow-up composer and basic session controls

Remote-v1 safety rules:

- no wallet send/withdraw
- no destructive config writes
- no public unauthenticated exposure
- no second independent state store in the browser

Acceptance:

- a second device on the same trusted network can open the remote UI and resume
  or continue a Codex thread
- approvals, tool user input, and basic session steering can be completed
  remotely
- disabling remote access tears down the listener and invalidates the prior
  token

Implementation note:

- do not block remote-v1 on a large new web-stack commitment
- the first version can be a narrow served web bundle owned by
  `apps/autopilot-desktop`
- if a separate companion web app later becomes justified, it must remain a thin
  client over desktop-owned truth

### CX-13. Personal Remote Companion V2 ([#3370](https://github.com/OpenAgentsInc/openagents/issues/3370))

Scope:

- extend remote to expose workspace and git context
- optionally add remote terminal visibility, then interactive terminal only if
  the safety model is strong enough
- expose branch/worktree context and more of the coding shell once local parity
  exists

Acceptance:

- remote becomes good enough for real supervision and light coding continuation
  away from the desk without pretending to be a hosted IDE

## Remote Setup Direction

The remote setup we should adapt is the local-first version:

- run the desktop app on the main machine
- optionally enable a remote listener from inside the desktop app
- connect from a phone, tablet, or second laptop over LAN or Tailscale-style
  networking
- authenticate with a generated token

This is valuable because it preserves the strongest part of the OpenAgents
thesis:

- the machine with the repo, local tools, wallet, and provider runtime remains
  the machine doing the work
- the remote device is only a window into that machine

This is the opposite of pushing OpenAgents toward a hosted browser product.

## Validation

When implementing roadmap slices, keep the existing repo gates in the loop:

- `scripts/lint/workspace-dependency-drift-check.sh`
- `scripts/lint/ownership-boundary-check.sh`
- `scripts/lint/touched-clippy-gate.sh`
- `cargo test -p autopilot-desktop codex_lane`
- `cargo test -p autopilot-desktop assemble_chat_turn_input`
- `cargo test -p codex-client --test skills_and_user_input`

Add slice-specific tests as the roadmap grows, especially for:

- readiness state transitions
- session-control state transitions
- thread lifecycle and steer flows
- turn-input assembly
- durable plan/review/diff persistence
- non-interactive exec event output
- git/worktree/PTY orchestration
- remote auth and listener lifecycle

## Recommendation

For the current product direction, the best sequence is:

1. `CX-1` readiness/auth/config truth
2. `CX-2` session controls and status parity
3. `CX-3` thread lifecycle and history parity
4. `CX-4` composer parity and queued follow-up turns
5. `CX-5` durable plans
6. `CX-6` review, compact, and structured diffs
7. `CX-7` workspace/project identity
8. `CX-8` git/branch/worktree/PR workflow
9. `CX-9` thread-scoped terminal
10. `CX-10` skills/MCP/apps/request-flow parity
11. `CX-11` non-interactive exec replacement
12. `CX-12` personal remote companion v1

That path turns the roadmap into an actual Codex CLI replacement plan first,
then a T3 Code-class local coding shell, then a safe personal remote surface.
