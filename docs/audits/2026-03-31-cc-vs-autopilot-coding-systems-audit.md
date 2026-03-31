# 2026-03-31 `cc` vs Autopilot Coding Systems Audit

## Scope

This audit compares the imported `cc` coding-agent system against the current
Autopilot coding stack in `openagents`.

The question is:

- what `cc` owns itself
- what Autopilot owns today for coding workflows
- where Autopilot is stronger
- where Autopilot is still only a shell around Codex
- what is worth importing into the current roadmap

Audit basis:

- `cc` reference docs prepared in `/Users/christopherdavid/work/docs/cc/`
- `cc` source snapshot at `/Users/christopherdavid/work/competition/repos/cc`
  - audited snapshot commit: `813c06acfa2d705076df6193b405c81eb11a18d1`
- `openagents`
  - branch audited: `main`
  - HEAD audited: `5238387587dfd265d455bc1735a07277d0a5c61b`

Per repo contract, the OpenAgents authority docs reviewed first:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`

Primary OpenAgents coding surfaces reviewed:

- `docs/codex/README.md`
- `docs/codex/EXEC.md`
- `docs/codex/REMOTE.md`
- `docs/codex/ROADMAP_CODEX.md`
- `docs/headless-compute.md`
- `docs/audits/2026-02-27-codex-app-server-full-integration-audit.md`
- `docs/audits/2026-02-27-codex-chat-skills-integration-audit.md`
- `docs/audits/2026-03-10-t3code-codex-wrapper-gap-audit.md`
- `docs/audits/2026-03-26-tailnet-codex-remote-companion-audit.md`
- `crates/codex-client/src/*`
- `apps/autopilot-desktop/src/codex_lane.rs`
- `apps/autopilot-desktop/src/codex_lane/{session,router,types}.rs`
- `apps/autopilot-desktop/src/codex_exec.rs`
- `apps/autopilot-desktop/src/codex_remote.rs`
- `apps/autopilot-desktop/src/openagents_dynamic_tools.rs`
- `apps/autopilot-desktop/src/panes/codex.rs`
- `apps/autopilot-desktop/src/desktop_control.rs`
- `apps/autopilot-desktop/src/bin/autopilotctl.rs`

Related `psionic` Hermes/Qwen surfaces reviewed for this extension:

- `../psionic/README.md`
- `../psionic/docs/hermes/README.md`
- `../psionic/docs/HERMES_QWEN35_COMPATIBILITY.md`
- `../psionic/docs/HERMES_QWEN35_PARALLEL_ATTRIBUTION.md`
- `../psionic/docs/HERMES_BACKEND_BENCHMARK.md`
- `../psionic/crates/psionic-models/src/lib.rs`
- `../psionic/crates/psionic-serve/src/openai_http.rs`

## Bottom Line

`cc` is a native coding-agent runtime. Autopilot is not.

`cc` owns its own:

- process startup
- session lifecycle
- query loop
- tool registry
- permission engine
- compaction and memory logic
- MCP and plugin loading
- background task model
- teammate and remote-session machinery

Autopilot owns a different layer:

- a desktop product shell around Codex app-server
- app-owned thread, plan, diff, approval, and workspace projections
- OpenAgents-specific dynamic tools for panes, CAD, data market, wallet,
  provider, treasury, and labor workflows
- a one-shot `autopilot-codex-exec` surface
- a local-first remote companion
- an app-owned control plane for the desktop runtime

So the practical split is:

- `cc` is closer to "build your own Claude Code/Codex-class runtime"
- Autopilot is closer to "build a desktop product and operator shell around
  Codex, then inject OpenAgents-native capabilities into it"

For current `openagents`, the right near-term move is not to clone `cc`
wholesale. The right move is to keep Codex as the engine and import selected
`cc` ideas around app-owned state, tasking, visibility, and operator control.

If OpenAgents later wants independence from Codex app-server, `cc` becomes a
much more direct architectural reference.

## Comparison Matrix

| Dimension | `cc` | Autopilot now | Implication |
| --- | --- | --- | --- |
| Runtime ownership | full native runtime | wraps Codex app-server | Autopilot is downstream of Codex behavior |
| Desktop product integration | not the focus of audited snapshot | strong | this is Autopilot's current advantage |
| Generic coding shell completeness | high at runtime level | medium | Autopilot still needs more shell workflows |
| OpenAgents domain tooling | none | strong | panes, wallet, provider, CAD, data-market, labor |
| Permission engine ownership | native | delegated/proxied | Autopilot does not own the core policy engine |
| MCP/plugin ownership | native | proxied from Codex plus app tools | Autopilot can inspect and surface, but not replace |
| Task/subagent ownership | native | limited app-side projection | major gap if we want deeper orchestration |
| Remote coding runtime | broad | narrower local-first companion | Autopilot is useful but not a full session taxonomy |
| Protocol drift controls | unclear from imported snapshot | strong | `crates/codex-client` and parity gates are real strengths |

## Detailed Comparison

## 1. Runtime ownership and startup

`cc` owns the coding runtime from the first process boundary:

- `main.tsx` is the actual orchestrator
- `entrypoints/init.ts` owns trust-sensitive startup
- `setup.ts` owns cwd, worktree, file watcher, and session-memory setup
- `QueryEngine.ts` and `query.ts` own turn execution

Autopilot does not own the same layer.

Autopilot today owns:

- `crates/codex-client` as the protocol wrapper
- `apps/autopilot-desktop/src/codex_lane.rs` as the lane thread and
  app-server request/notification projection
- desktop state and pane behavior around that lane

This means Autopilot can strongly control product behavior, but it does not
control the underlying query engine, compaction engine, or built-in tool
runtime.

That is the first and most important difference.

## 2. Session model and UI state

`cc` has one broad AppState that includes:

- settings
- permission context
- MCP state
- plugin state
- task state
- remote state
- prompt suggestion/speculation
- team and companion state

Autopilot has a different app-state shape:

- thread projections
- latest plan/review/diff artifacts
- pending command/file approvals
- workspace and project identity
- skill attachments and draft state
- pane state across many non-coding surfaces
- wallet, provider, data market, and kernel-adjacent UI truth

Autopilot already does real app-owned Codex productization here. `docs/codex/ROADMAP_CODEX.md`
records shipped work on:

- session controls
- thread lifecycle controls
- plan artifact persistence
- diff/review/compact artifact persistence
- workspace/project identity
- request and approval control flows

So Autopilot is not just a dumb transport wrapper anymore. It has an app-owned
projection layer around Codex threads.

## 3. Query loop and context management

`cc` owns this internally.

It has native implementations for:

- system prompt assembly
- user-input preprocessing
- streaming model loop
- tool-result ordering
- snip/microcompact/autocompact/reactive compact
- `max_output_tokens` recovery
- transcript persistence

Autopilot does not implement those behaviors itself. Codex app-server does.

Autopilot's app-owned role is:

- start or resume a Codex thread
- submit turns and steer follow-ups
- normalize notifications
- store thread/session artifacts that the user needs
- expose those artifacts in desktop and remote surfaces

That is a product shell, not a runtime kernel.

If we keep Codex as engine, this is acceptable.
If we want runtime independence, this is the biggest missing ownership area.

## 4. Tools and execution model

`cc` owns a native tool system:

- `Tool.ts` defines the full tool contract
- `tools.ts` defines the builtin inventory
- tool execution has validation, concurrency control, interrupt behavior,
  hooks, telemetry, and synthetic failure handling
- tool orchestration distinguishes concurrency-safe and serial tool calls

Autopilot does not own generic coding tools at that level.

Autopilot instead injects app-owned dynamic tools into Codex. The current
inventory in `apps/autopilot-desktop/src/openagents_dynamic_tools.rs` is
already substantial: `41` tool specs.

Those tools cover:

- pane control
- CAD intent/action
- data-market draft, preview, publish, payment, delivery, and revocation
- treasury and swap flows
- provider control
- labor evidence and claim flows

This is a real strength. `cc` does not have OpenAgents-native product tools.

But it is also a sign of the boundary:

- `cc` owns the general tool runtime
- Autopilot owns product-specific tools presented to Codex

## 5. Permissions and approvals

`cc` has a native permission engine.

It owns:

- allow/deny/ask rule matching
- multiple permission modes
- auto-mode classifier behavior
- denial tracking
- tool-specific safety logic

Autopilot does not own a general-purpose equivalent.

Autopilot currently owns:

- approval mode selection in chat/session state
- projection of pending command/file approvals into app state
- approval responses through desktop UI and remote companion
- some product flows that intentionally force `AskForApproval::Never`
  for deterministic automation paths

That is a thinner layer. It is enough for a Codex wrapper. It is not enough
to replace a native permission/runtime system like `cc`.

## 6. MCP, plugins, skills, and apps

`cc` natively loads and manages:

- MCP configs and transport connections
- MCP tools, commands, and resources
- plugin manifests, installs, and caches
- bundled and dynamic skills

Autopilot does not own that plane natively.

Autopilot currently does three things instead:

1. `crates/codex-client` exposes broad Codex protocol coverage for skills,
   MCP, config, apps, models, and account methods.
2. desktop panes surface those Codex-backed states as product UIs
3. OpenAgents injects its own dynamic tools into Codex

This is enough to make Codex useful inside Autopilot. It is not enough to say
Autopilot owns the MCP/plugin/skill substrate.

## 7. Memory, tasks, subagents, and swarms

This is one of the clearest gaps.

`cc` has native ownership of:

- file-based persistent memory
- session-memory extraction
- team-memory sync
- background shell tasks
- background agent tasks
- teammate/team state
- dream/consolidation tasks

Autopilot today does not have an equivalent coding-runtime task system.

What it does have is narrower:

- saved plan artifacts
- saved diff/review/compact artifacts
- approval and tool-prompt queues
- remote follow-up and approval handling
- product-specific orchestration around CAD and seller flows

Autopilot has productized Codex session artifacts. It has not built a native
coding-task runtime around them.

If we want richer coding orchestration, this is one of the highest-leverage
areas to borrow from `cc`.

## 8. Remote and automation

`cc` has broad remote/session support:

- connect
- SSH
- assistant viewer
- remote/teleport sessions
- bridge/remote-control
- remote-safe command filtering

Autopilot has a narrower but already useful remote stack:

- `autopilot-codex-exec` for one-shot non-interactive runs
- a local-first remote companion in `apps/autopilot-desktop/src/codex_remote.rs`
- Tailnet-safe bind rules
- remote visibility into:
  - transcript
  - approvals
  - tool prompts
  - plan artifact
  - diff artifact
  - wallet and provider truth
  - workspace/git identity
  - read-only terminal visibility

This is good product work.

The current weakness is consistency of ownership:

- Autopilot has a strong app-owned control plane in `desktop_control.rs` and
  `autopilotctl.rs`
- but Codex remote itself is still more chat- and pane-driven than
  `desktop_control`-driven

That is already called out in
`docs/audits/2026-03-26-tailnet-codex-remote-companion-audit.md`.

Compared to `cc`, Autopilot remote is useful but still incomplete.

## 9. Product integration

This is where Autopilot is stronger.

Autopilot's coding lane lives inside a desktop product that also owns:

- wallet truth
- provider online/offline truth
- data-market state
- CAD state
- labor evidence and claim flows
- Tailnet and desktop-control operator surfaces

`cc` is stronger as a coding runtime. Autopilot is stronger as a product shell
that ties coding behavior into the rest of OpenAgents.

This matters because `docs/MVP.md` is explicit:

- Autopilot is a personal agent and a money-printing provider product
- coding is one important user-facing lane
- coding cannot displace wallet truth, provider truth, or the earn loop

That makes a full `cc`-style in-repo runtime rewrite a worse near-term fit
than continued Codex-based shell productization.

## 10. Testing and drift management

Autopilot has a meaningful strength that `cc` did not expose in the imported
snapshot:

- `crates/codex-client` protocol conformance tests
- `scripts/lint/codex-protocol-parity-gate.sh`
- live harness tooling in `docs/codex/LIVE_HARNESS.md`
- release and debug runbooks for the coding lane

That is important because the wrapper model only works if protocol drift is
kept under control.

Autopilot is currently better than many Codex wrappers on this exact point.

## 11. Related `psionic` Hermes/Qwen track

There is real prior work in the broader OpenAgents stack on Hermes, but it is
important to describe it correctly.

The honest characterization is not "we ported Hermes into OpenAgents" and it
is not "Autopilot already has a native coding runtime somewhere else."

What `psionic` actually explored and implemented was a backend lane for Hermes,
especially around local `qwen3.5` compatibility.

The strongest retained claims in the `psionic` repo are:

- `docs/hermes/README.md`: Psionic can act as a real backend for Hermes on one
  consumer GPU through the normal OpenAI-compatible `chat.completions` path.
  Hermes remains a separate checkout. Psionic does not bundle a full Hermes
  product or CLI.
- `docs/HERMES_QWEN35_COMPATIBILITY.md`: the canonical retained direct proof is
  green `6/6` on `2026-03-29` for native Psionic `qwen35` on the local
  `archlinux` `RTX 4080` lane.
- `docs/HERMES_QWEN35_PARALLEL_ATTRIBUTION.md`: the formerly hard same-turn
  parallel-tool row is now green on Psionic and Ollama across `2b`, `4b`, and
  `9b`.
- `docs/HERMES_BACKEND_BENCHMARK.md`: the repo also kept same-host benchmark
  receipts where Hermes stays fixed and only the backend endpoint and
  backend-specific model identifier change.

So there was real Hermes-related work. But the thing that got owned in
`psionic` was the serving/runtime/tool-contract boundary, not the whole
controller shell that Hermes itself provides.

Concretely, that `psionic` work includes:

- native Qwen-family support in `crates/psionic-models/src/lib.rs`, including
  `Qwen35` tokenizer and prompt-template families, reasoning extraction, think
  block stripping, decoder-family metadata, and Qwen-specific fixture coverage
- OpenAI-compatible tool-call and replay handling in
  `crates/psionic-serve/src/openai_http.rs`, including explicit
  `parallel_tool_calls` handling, `minimum_required_tool_calls`, ordered tool
  batch validation, and replay-safe tool-result behavior
- repo-owned Hermes probes and harnesses under `scripts/release/` plus retained
  receipts under `fixtures/qwen35/hermes/`

That matters because it answers the user's likely question directly:

- no, the repo did not absorb all of Hermes as a native OpenAgents agent shell
- yes, the repo did do serious Hermes compatibility and backend-porting work
  around Qwen, tool calls, replay, and same-turn multi-tool behavior

This gives the broader stack three distinct ownership cuts:

- `cc`: own the coding controller/runtime itself
- Autopilot today: own the desktop product shell around Codex
- `psionic` Hermes/Qwen lane: own the inference/backend contract while leaving
  the controller layer in external Hermes code

That third point changes the comparison in one useful way.

OpenAgents is not starting from zero if it ever wants less dependence on Codex.
The stack already has real prior work on:

- OpenAI-compatible backend serving
- Qwen-family local model support
- tool-loop correctness
- tool-result replay
- strict same-turn parallel tool behavior

But that prior work reduces backend risk, not controller/runtime risk.

It does not mean OpenAgents already has:

- a native coding query loop
- an app-owned tool scheduler
- an app-owned permission engine
- an app-owned coding-task or subagent runtime
- a full coding-session UI/runtime comparable to `cc`

So the right interpretation is:

- `cc` shows what it looks like to own the coding runtime
- the `psionic` Hermes/Qwen lane shows what it looks like to own the serving
  substrate for an external agent/controller
- Autopilot today shows what it looks like to own the product shell around an
  external agent/runtime

Those are different layers, and mixing them together would make the audit less
honest.

## What `cc` Has That Autopilot Still Does Not

The most important missing capabilities are:

1. A native query/tool/permission/memory/task runtime.
2. App-owned background coding tasks and teammate/subagent objects.
3. First-class ownership of MCP/plugin loading instead of mostly proxying
   Codex state.
4. A broader remote session taxonomy.
5. A coding runtime that would still exist if Codex app-server disappeared.

Those are real gaps. They should not be hand-waved away.

## What Autopilot Has That `cc` Does Not

The most important Autopilot-specific strengths are:

1. App-owned desktop integration with wallet, provider, data-market, labor,
   CAD, and operator surfaces.
2. Typed OpenAgents dynamic tool bridging into real product behavior.
3. A desktop control plane and `autopilotctl` operator path for the running app.
4. A remote companion explicitly shaped around the same local machine.
5. Strong Codex protocol parity and debugging discipline.

These are not side details. They are why a pure `cc` clone is not the right
near-term product move for this repo.

## Strategic Conclusion

OpenAgents should currently think about `cc` in two different ways.

There is also a third relevant reference inside the broader stack: the
`psionic` Hermes/Qwen lane. That work proves OpenAgents has already explored
backend ownership for an external agent, especially around Qwen tool-call
correctness and replay behavior. It does not prove that OpenAgents already owns
the controller/runtime layer that `cc` owns.

### 1. Near-term reference

Use `cc` as a reference for:

- app-owned task/state projection around a coding session
- clearer session and runtime ownership boundaries
- stronger visibility into approvals, requests, tools, and background work
- richer memory and artifact treatment
- more complete remote/session continuity

These are improvements we can apply while still using Codex as the engine.

### 2. Long-term runtime reference

Use `cc` as a reference for a future native OpenAgents coding runtime only if
we decide we want to own:

- the model loop
- the tool scheduler
- the permission engine
- MCP/plugin/memory/task infrastructure

That is a much larger decision than "improve Autopilot coding UX."

If that longer-term decision ever happens, the current internal starting points
are different:

- `cc` is the better reference for controller/runtime ownership
- `psionic` is the better reference for owned backend/model/tool-contract
  behavior
- Autopilot is the better reference for the app-owned operator and product
  shell

## Recommended Direction For `openagents`

For the current roadmap, the right move is:

1. Keep Codex as the underlying engine.
2. Continue the `docs/codex/ROADMAP_CODEX.md` plan for desktop coding-shell
   replacement.
3. Import `cc` ideas selectively into the app-owned shell.

Highest-leverage imports from `cc` into the current Autopilot roadmap:

- stronger app-owned task objects for long-running coding activity
- richer projection of tool execution and request state
- broader app-owned session artifact model
- more honest memory/compaction visibility if Codex exposes enough signal
- better remote/session-control continuity

Lower-leverage near-term imports:

- reimplementing the full Codex runtime inside OpenAgents
- cloning `cc`'s Bun/TypeScript architecture
- trying to own plugins/MCP/memory all at once in the desktop app

That would fight both `docs/MVP.md` and `docs/OWNERSHIP.md`.

Related implication from the `psionic` Hermes lane:

- preserve the backend lessons separately from the shell lessons
- treat Hermes-on-Psionic as evidence that backend independence is plausible
- do not confuse that with proof that the Autopilot coding shell can already
  replace Codex
- if OpenAgents ever wants a post-Codex path, the likely sequence is:
  stronger Autopilot shell first, stronger owned backend second, owned
  controller/runtime only after that decision is explicit

## Decision

The honest current description is:

- `cc` is a runtime we can study if we want to own the coding engine.
- `psionic` contains real Hermes/Qwen backend compatibility work, but that work
  owns the backend contract rather than the full agent shell.
- Autopilot today is a serious Codex shell with real product integration, not a
  native coding runtime.
- The right next step is to make that shell stronger, not to pretend we
  already own the engine.
