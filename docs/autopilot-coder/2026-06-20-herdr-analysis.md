# herdr analysis — what (if anything) to use/adapt for pylon/autopilot/agent delegation

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-20
Author: reference-repo audit (read-only on `projects/repos/herdr`)
Verdict (one line): **Adapt the ideas, port one pattern, skip the binary.** Steal
herdr's declarative screen-detection manifests and its agent-driven socket
vocabulary; do not adopt the AGPL Rust TUI itself.

---

## 1. What herdr actually is

`herdr` (reference clone at `/Users/christopherdavid/work/projects/repos/herdr`,
upstream `github.com/ogulcancelik/herdr`, site `herdr.dev`) is an **agent-aware
terminal multiplexer** — "tmux, but it knows which of your coding agents are
blocked, working, or done." It is a single Rust binary, not a GUI/Electron/web
app. You run `herdr` in a repo, it starts (or attaches to) a background session
server, and you get tmux-style workspaces/tabs/panes where each pane is a real
terminal running a real agent (Claude Code, Codex, droid, amp, opencode, etc.).

Confirmed from source, not the name:

- **Stack / size.** Rust 2021, `ratatui` 0.30 TUI, `tokio`, `portable-pty`,
  `interprocess` (Unix domain + Windows named-pipe sockets), a vendored
  `libghostty-vt` terminal parser. ~155k LOC across `src/`
  (`find src -name '*.rs' | xargs wc -l` → 154,690). Mature and active: 897
  commits since 2026-03-27, current `Cargo.toml` version `0.7.0`, last commit
  2026-06-20.
- **Architecture (from `AGENTS.md` + `src/`).** Clean server/client split. The
  server runs headless (`src/server/headless.rs`) with no TTY and owns all PTYs;
  clients attach over a binary wire protocol (`src/protocol/wire.rs`,
  `PROTOCOL_VERSION = 14`, 2 MB frame cap). Pure-data `AppState` is separated
  from `PaneRuntime`, so workspace/tab/pane logic is testable without real
  terminals. Detach closes only the client; panes keep running. Remote attach is
  just SSH (`src/remote/unix.rs`, `herdr --remote ssh://...`).
- **The interesting part — agent state detection.** A decoupled detector
  (`src/detect/`) reads a *screen snapshot* of each pane's bottom buffer and OSC
  title/progress strings, and classifies the agent as `idle` / `working` /
  `blocked` / `done` / `unknown`. Rules are **declarative TOML manifests**, one
  per agent, in `src/detect/manifests/` (18 of them: `claude.toml`, `codex.toml`,
  `amp.toml`, `opencode.toml`, `droid.toml`, `gemini.toml`, `hermes.toml`, etc.).
  Each rule has a `region` (`osc_title`, `prompt_box_body`,
  `after_last_horizontal_rule`, `bottom_non_empty_lines(3)`, `whole_recent`, …),
  a target `state`, a `priority`, and `contains` / `regex` / `any` / `all` / `not`
  gates. Manifests hot-reload (`herdr server reload-agent-manifests`) and can be
  fetched/overridden remotely (`ManifestSource::{Bundled,Remote,Override}` in
  `src/detect/manifest.rs`). Detection is zero-config: it works on process name +
  screen text with no hooks required. Example: `claude.toml`'s
  `bash_permission_prompt` rule fires `blocked` when the bottom buffer contains
  `"do you want to proceed?"` plus a `bash command` marker plus a `1. yes / 2. no`
  line — that is exactly the "agent needs an approval" signal.
- **Two-tier integration model.** On top of screen detection, official
  integrations (`src/integration/`, assets under `src/integration/assets/<agent>/`)
  install agent-native hooks that report *semantic* state and *session identity*
  over the socket. The Claude one
  (`src/integration/assets/claude/herdr-agent-state.sh`) is a Claude Code hook
  that posts `session` / `working` / `blocked` / `idle` events plus the
  `session_id` and `transcript_path` to `$HERDR_SOCKET_PATH` for the current
  `$HERDR_PANE_ID`. Kimi's hook maps the full Claude-style hook event set
  (`UserPromptSubmit`→working, `PermissionRequest`→blocked, `Stop`→idle,
  `SessionEnd`→release; see `KIMI_HOOK_EVENTS` in `src/integration/mod.rs`).
  Session identity feeds **restore**: after a server restart, supported agent
  panes relaunch via their native resume (`src/agent_resume.rs` builds an
  `AgentResumePlan` argv like `claude --resume <id>` / `devin --resume <id>`).
- **Agents can drive it (the orchestration angle).** A local Unix socket exposes
  a typed JSON method API (`src/api/`, `src/api/schema/`). `SKILL.md` documents
  the agent-facing CLI that wraps it: `herdr workspace/tab/pane list|create`,
  `herdr pane split --direction right --no-focus`, `herdr pane run <id> "<cmd>"`,
  `herdr pane read <id> --source recent`, `herdr wait output <id> --match ... `,
  and crucially `herdr wait agent-status <id> --status done --timeout`. So an
  agent inside a pane can spawn sibling agents, hand them a task, block until
  they reach `done`, and read their transcript. Methods include
  `AgentStart`, `PaneSplit`, `PaneReportAgent`, `PaneReportAgentSession`,
  `WorktreeCreate` (`src/api/mod.rs`, `src/api/schema/agents.rs`).
- **Worktree-native.** First-class `prefix+shift+g` "new worktree" and
  `WorktreeCreate` API (`src/worktree.rs`) — one git worktree + branch per
  parallel agent, the same isolation pattern OpenAgents already uses.
- **License.** Dual: **AGPL-3.0-or-later** OR a paid commercial license
  (`LICENSE`, `Cargo.toml`). This is the load-bearing constraint below.

## 2. Where this maps onto OpenAgents' needs

The relevant OpenAgents surfaces today (verified against `probe`, `openagents`,
`autopilot-omega`):

- **Pylon already is the multi-session supervisor.** `apps/pylon/src/coordinator/
  coordinator-runtime.ts` runs an autonomous background loop that fans one intent
  into multiple parallel-worktree sessions and tracks
  `received→planning→fanning_out→shipping→shipped/failed`.
  `apps/pylon/src/node/control-sessions.ts` is the per-session state machine
  (`queued→running→completed/failed/cancelled`), and
  `apps/pylon/src/node/approval-queue.ts` is the exactly-once approval queue for
  blocked states.
- **A control API already exists, and it is better-fitted than herdr's.**
  `apps/pylon/src/node/control-server.ts` (loopback HTTP, bearer-gated) plus the
  capability-scoped `/bridge` protocol in
  `packages/autopilot-control-protocol/src/bridge.ts`
  (`session.list/spawn/cancel/subscribe`, `decision.resolve`, `turn.steer`,
  `coordinator.pause/resume`, projection levels `public_safe`/`public`/`private`)
  is what the mobile operator app
  (`clients/khala-ios/AutopilotRemoteControl/src/control/control-client.ts`) speaks.
  This is a capability- and identity-scoped, network-friendly, cursor-resumable
  control plane — strictly more than herdr's local-only Unix socket.
- **Probe is the runtime; Omega is the product surface.** `probe` owns durable
  session/tool/approval contracts; `autopilot-omega` owns team/billing/onboarding
  and ingests runner events. Neither needs a terminal multiplexer.

So OpenAgents already has the *coordination server*, *async/overnight loop*,
*approval queue*, *worktree-per-session isolation*, and *remote control plane*
that herdr provides for the single-operator-terminal case. herdr is a
single-machine, terminal-attached, human-in-the-loop tool; Pylon/Autopilot is a
headless, capability-gated, mobile-and-cloud-reachable fleet. **They solve the
same shape of problem at different altitudes.** That means herdr is not a drop-in
component for us — but a few of its sub-systems are genuinely ahead of ours.

## 3. Honest assessment, capability by capability

| OpenAgents need | herdr offer | Already have? | Useful delta |
|---|---|---|---|
| (a) background/async agent execution | headless server, detached panes survive client exit | **Yes** — Pylon coordinator loop + warm-session sync | None worth adopting |
| (b) supervise/herd many agents/sessions | tmux-style pane roster + sidebar rollup of blocked/working/done | **Yes** — Pylon session roster, mobile app view | None at the orchestration layer; the *state taxonomy* is a small win |
| (c) delegation/orchestration patterns | `wait agent-status`, agent-spawns-agent over socket, `SKILL.md` | **Partial** — Pylon coordinator spawns; no agent↔agent RPC primitive | The `wait agent-status …` + `pane read --source recent` *idiom* is worth copying into our agent skill |
| (d) pylon node management | server/client split, remote-over-SSH attach, restore/handoff | **Yes** — Pylon node + bridge pairing | None; our bridge is more capable |
| (e) Autopilot session/approval/delegation model | screen-scraped `blocked` approval detection + native hooks | **Partial** — we depend on Probe/agent self-reporting | **This is the real prize** (see below) |

### The one thing herdr does materially better than us

**Provider-agnostic agent-state detection that works without the agent's
cooperation.** OpenAgents' state today comes from *inside* the runtime (Probe
self-reports, or a Claude hook fires). That is clean when we own the runtime, but
it breaks the moment we delegate to a third-party CLI we don't control — `codex`,
`droid`, `cursor-agent`, `gemini`, a future tool — running headless in a Pylon
worktree or an Autopilot pane. herdr's `src/detect/` engine answers exactly that:
"is this arbitrary coding-agent CLI currently *blocked waiting for me*,
*working*, or *done*?" purely from its terminal output, with the rules expressed
as data (`src/detect/manifests/*.toml`), hot-reloadable and remotely
updatable. The 18 hand-tuned manifests encode real, fragile knowledge about what
"Claude is asking for bash approval" vs "Codex hit `allow command?`" actually
looks like on screen — knowledge we would otherwise have to rediscover.

## 4. Recommendations

1. **SKIP adopting the herdr binary / vendoring its code.** Two decisive reasons:
   (1) **License** — it is AGPL-3.0-or-later. Linking or deriving from it in our
   closed-source Pylon/`cloud`/Autopilot surfaces is a non-starter without buying
   the commercial license, and even then it is a hard dependency on a one-person
   project. (2) **Altitude mismatch** — it is a human's single-terminal tool; our
   need is a headless, capability-scoped, mobile/cloud-reachable fleet we already
   have in Pylon + `autopilot-control-protocol`. Wrapping or running herdr in the
   background "for us" would add a TUI server we'd then have to drive
   programmatically — strictly worse than our own bridge.

2. **ADAPT the idea: a declarative, data-driven agent-state detection layer for
   delegated CLIs.** Build an OpenAgents-owned detector that, given a pane/PTY
   screen snapshot of a third-party coding agent, classifies
   idle/working/blocked/done. Model the rule format on herdr's manifest schema
   (`region` + `priority` + `contains/regex/any/all/not`) so detection rules are
   *config, not code* and can be tuned/shipped without redeploys — the same way
   herdr hot-reloads `src/detect/manifests/`. This belongs in `probe` (runtime
   truth) or as a small shared package consumed by Pylon. Clean-room from the
   documented behavior; **do not copy AGPL Rust source.** The manifests
   themselves are useful *reference data* for what the on-screen signals are, but
   re-encode them in our own format.

3. **PORT one concrete pattern into our agent skill: `wait agent-status` +
   `read recent`.** herdr's `SKILL.md` codifies a clean delegation idiom —
   *spawn a sibling agent, block on `herdr wait agent-status <id> --status done`,
   then `pane read --source recent`*. We already have the transport
   (`session.subscribe` is cursor-resumable in
   `packages/autopilot-control-protocol/src/bridge.ts`); we lack the
   ergonomic, documented "delegate and await" recipe for an agent driving our
   bridge. Add the equivalent verbs/recipe to whatever skill the coding agent
   loads (the analog of `herdr wait agent-status`) so an Autopilot/Pylon agent
   can fan out to sub-agents and await `done`/`blocked` the same way.

4. **ADOPT the state taxonomy `{idle, working, blocked, done}` with the "done =
   finished but unseen" nuance.** Our Pylon states are lifecycle-oriented
   (`queued/running/completed/failed/cancelled`); herdr's are
   *operator-attention*-oriented, and the `done` vs `idle` ("seen") distinction is
   a genuinely good UX primitive for "what needs my eyes right now" across a fleet
   and on the mobile app. Cheap to add as a derived/attention layer over our
   existing lifecycle states.

## 5. Integration cost + risks

- **Adopting the binary:** high cost, high risk. AGPL contamination of closed
  repos, commercial-license negotiation, single-maintainer dependency, and a
  redundant TUI server. Not recommended.
- **Adapting the detection idea (rec #2):** medium cost. A manifest schema +
  evaluator + region-extraction (bottom-buffer/OSC-title parsing) is real work,
  but bounded and self-contained, and herdr's `src/detect/manifest.rs` is an
  excellent *design reference* for the region kinds and rule-precedence semantics.
  Main risk is the inherent fragility of screen-scraping third-party CLIs (their
  output changes between versions) — herdr manages this with hot-reload +
  remote manifest updates + evidence-based tuning (`herdr agent explain --json`),
  a discipline we should copy. Prefer the native-hook path (like
  `herdr integration install claude`) wherever we control or can configure the
  agent; fall back to screen detection only for CLIs we can't instrument.
- **Porting the delegation recipe (rec #3) + taxonomy (rec #4):** low cost,
  docs-and-thin-code. No license exposure (clean-room from public docs/behavior).

## 6. What to do next (if we adopt anything)

1. File an issue in `probe` (or a shared detection package): "data-driven
   agent-state detection for delegated/third-party coding-agent CLIs," referencing
   herdr's manifest schema (`src/detect/manifest.rs`, `src/detect/manifests/*.toml`)
   as the *design* reference and explicitly noting clean-room / no-AGPL-copy.
2. Add a `{idle, working, blocked, done(seen?)}` attention projection over Pylon's
   existing lifecycle states (`apps/pylon/src/node/control-sessions.ts`), surfaced
   in the mobile control client
   (`clients/khala-ios/AutopilotRemoteControl/src/control/session-view-model.ts`).
3. Extend the coding-agent skill / `autopilot-control-protocol` recipes with a
   documented "delegate to sub-agent and await `done`/`blocked`" pattern built on
   the existing `session.subscribe` cursor stream — our analog of
   `herdr wait agent-status`.
4. Prefer agent-native hooks over screen-scraping wherever we can instrument the
   agent (mirror herdr's `integration install` model for the CLIs we delegate to).

## 7. References (herdr, read-only)

- `README.md`, `AGENTS.md`, `SKILL.md` — product thesis, architecture principles, agent CLI.
- `src/detect/manifest.rs`, `src/detect/manifests/{claude,codex,...}.toml` — declarative detection engine + 18 agent manifests.
- `src/integration/mod.rs`, `src/integration/assets/claude/herdr-agent-state.sh` — native-hook semantic state + session identity reporting.
- `src/agent_resume.rs` — session-identity-driven restore (`<agent> --resume <id>`).
- `src/api/`, `src/api/schema/agents.rs`, `src/api/mod.rs` — typed socket method API.
- `src/protocol/wire.rs` — binary wire protocol (v14), server/client split.
- `src/server/headless.rs` — headless background server.
- `src/worktree.rs` — git-worktree-per-agent isolation.
- `Cargo.toml`, `LICENSE` — Rust stack; AGPL-3.0-or-later / commercial dual license.

OpenAgents surfaces compared (read-only):
`probe/`, `openagents/apps/pylon/src/coordinator/coordinator-runtime.ts`,
`openagents/apps/pylon/src/node/{control-server,control-sessions,approval-queue}.ts`,
`openagents/packages/autopilot-control-protocol/src/bridge.ts`,
`openagents/clients/khala-ios/AutopilotRemoteControl/src/control/control-client.ts`,
`autopilot-omega/`.
