# OpenCode plugins, OpenCode 2.0, and what Coder should take from them

2026-10-12. The owner asked how OpenCode does plugins and whether Coder should
learn from it. The sources are the reference clone at
`projects/repos/opencode` (HEAD `1c74b9f020`, 2026-10-12, `@opencode/plugin`
2.0.27) and the note [OpenCode 2.0](https://anoma.ly/notes/opencode-2-0/). The
note has no byline or date. Paths below are relative to the clone's
`packages/` unless they start with `crates/`. Line numbers are at that HEAD.

Coder is compared on `origin/main` as of today: `crates/coder-new/src/plugin_definition.rs`,
`plugins.rs`, `bundled_settings.rs`, `plugin_tools.rs`, `plugin_store.rs`,
`plugin_catalog.rs`, `crates/background/src/plugins.rs`, and the plan in
#11451 to #11456. `harness_plugin.rs` has not landed yet.

## 1. How OpenCode's plugins work

### What a plugin is

A plugin is a module whose default export is `{ id, effect }` (the Effect API)
or `{ id, setup }` (the Promise API) (`core/src/plugin/module.ts:78-91`).
The setup function gets a `Context` with one handle per domain: `agent`,
`aisdk`, `command`, `event`, `integration`, `mcp`, `model`, `permission`,
`provider`, `reference`, `rpc`, `session`, `shell`, `skill`, `storage`,
`tool`, `vcs`, `websearch` and `worktree`, plus `options`, `location` and
`generate` (`plugin/src/effect/plugin.ts:26-59`). Setup returns nothing. It
registers behavior imperatively, and the plugin's scope owns every
registration, so unloading the plugin removes all of them
(`plugin/src/effect/README.md:26-30`). The Promise API is an adapter over the
Effect API. It is not a second runtime (`plugin/src/promise/adapter.ts:207-216`).

There are two kinds of registration:

| Kind | What it does | Examples |
| --- | --- | --- |
| **Transform** | A synchronous editor callback that adds, updates or removes entries in a stateful domain. The registry replays every transform in registration order onto a fresh value when something changes, and `reload()` replays one domain. Values read earlier are never mutated. | `agent`, `command`, `integration`, `mcp`, `model`, `provider`, `reference`, `skill`, `tool`, `vcs`, `websearch` (`plugin/src/effect/README.md:32-64`, `:128-163`) |
| **Hook** | Intercepts a live operation. Hooks run in registration order, and later hooks see earlier mutations (`README.md:83-107`). | `tool` `execute.before`/`execute.after`; `permission` `evaluate`; `session` `prompt`, `context`, `compaction`, `generate`, `title`, `model.request`, `http.request`/`http.response`, `experimental.ws.*`, `retry`; `shell` `create.before`; `aisdk` `sdk`/`language` |

The hook surface:

- **Tools.** `execute.before` can rewrite the input or reject the call with a
  `Tool.Error`. `execute.after` can rewrite the result or the error
  (`plugin/src/effect/tool.ts:20-52`).
- **Session.** `context` exposes the system parts, messages, request options
  and the tool map just before provider dispatch. A plugin can rewrite a
  tool's description or delete a tool for that request. `compaction` and
  `title` can supply a result and skip the model call. `retry` decides the
  retry policy. Every request is tagged
  `kind: "primary" | "compaction" | "title" | "generate"`
  (`plugin/src/effect/session.ts:33-36`, `:61`, `:138-151`).
- **Permission.** `evaluate` sees `action`, `resources` and the current
  `effect`, and may change that effect (`plugin/src/effect/permission.ts:7-20`).
  The managed-policy plugin uses it to enforce organization statements
  (`core/src/config/plugin/policy.ts:48`).
- **Providers and auth.** Provider and model transforms edit the catalog,
  and `aisdk` hooks swap in an SDK or a language model. Auth is the
  `integration` domain, which covers API key, OAuth and command connect
  methods with `authorize` and `refresh` (`plugin/src/promise/adapter.ts:354-436`).
  About 35 provider integrations are internal plugins under `core/src/plugin/provider/`.
- **Storage** is per plugin, namespaced `plugin:<id>` in the key-value store
  (`core/src/plugin/host.ts:609-617`).
- **RPC** lets a plugin publish typed methods and events that clients and
  other plugins can call (`plugin/src/promise/adapter.ts:77-162`).

### Loading, configuration, distribution

- **Config.** `plugins` in any config document takes an npm spec, a path, or
  `{ package, options }`. `-name` removes a plugin, and `*` or `opencode.*`
  match by wildcard (`schema/src/config/plugin.ts`,
  `core/src/config/plugin/source.ts:113-120`). Plugins in `plugin/` or
  `plugins/` directories inside config roots are discovered automatically,
  and explicit config is applied last so it can remove discovered ones
  (`core/src/plugin/source-directory.ts:7`, `core/src/config/plugin/source.ts:125-160`).
- **Distribution is npm.** Package plugins are installed through the host's
  npm service. A package may expose `server`, `tui` and `rpc` entrypoints
  (`plugin/src/host.ts:11-44`, `core/src/plugin/module.ts:98-150`). The
  host checks for updates once a day and marks outdated packages
  (`core/src/plugin/supervisor.ts:184-203`, `:222`).
- **Order.** Built-in "pre" plugins load first, then SDK-supplied and
  instance plugins, then user packages, then built-in "post" plugins, most of
  them config readers (`core/src/plugin/supervisor.ts:99-103`,
  `core/src/plugin/internal.ts:216-270`). On a duplicate ID the first one
  wins, and the later one is reported as failed instead of taking down the
  whole set (`supervisor.ts:104-119`). A plugin that fails to reload keeps
  its previous running generation (`supervisor.ts:79-91`).
- **Guarded plugins.** Repository config cannot remove the policy plugin or
  the Console connection (`internal.ts:272-274`, `supervisor.ts:43`). Managed
  policy can block any plugin with `integration.use` on `plugin:<name>`
  (`module.ts:52-59`).
- **Hot reload.** Local plugin sources are fingerprinted by SHA-256 over
  every file the module graph touches. A change on disk triggers a debounced
  reactivation that re-imports only the changed graphs
  (`plugin/src/source.ts:8-46`, `supervisor.ts:205-247`).
- **No isolation.** Plugins run in-process with the host's full authority.
  The only gates are policy (allowed to load) and permission rules (allowed
  to call a tool).

### Agents, modes, subagents

- An agent is a record: `id`, `name`, an optional `model`, `system`,
  `description`, `mode: "subagent" | "primary" | "all"`, `hidden`, `steps`,
  and `permissions` (`schema/src/agent.ts:23-35`). The default ruleset is
  allow everything, but ask before reading `.env` files (`agent.ts:39-51`).
- The built-in agents are contributed by a plugin, `opencode.agent`:
  - Build is primary.
  - General and Explore are subagents. Explore is denied everything except
    reads, search, web and shell.
  - Compaction, Title and Summary are hidden.

  (`core/src/plugin/agent.ts:76-141`)
- Plan "mode" is also just a plugin (`core/src/plugin/plan.ts`). It is a
  primary agent that may edit only its plan directory, plus:
  - a `tool.execute.after` hook that rewrites the permission error;
  - a `session.context` hook that adds or removes a system reminder near the
    tail of the messages, so the cached prefix stays warm.
- **Routing.**
  - The person picks the primary agent: Tab runs `agent.cycle`
    (`tui/src/app.tsx:904`), and clients call `session.switchAgent`.
  - The model picks subagents through the `subagent` tool, reading the
    agents' descriptions.
  - A primary agent cannot be called as a subagent
    (`core/src/tool/plugin/subagent.ts:136`).
  - The tool takes `background: true`: it returns at once and notifies the
    caller when the subagent finishes.
  - A `model` override exists, but the tool's own description says to set it
    only when the person asks (`subagent.ts:31-46`).

  Nothing ranks agents or models for a task. Routing is the person's choice
  plus the model's tool choice.
- **Per-model behavior** is also plugins. `opencode.prompt.openai` and
  `opencode.prompt.anthropic` choose a system prompt by model ID
  (`core/src/plugin/optimize.ts:16-30`). `opencode.tool.input.repair` coerces
  malformed tool arguments against each tool's JSON schema, for example
  string numbers, stringified objects or null placeholders, before execution
  (`core/src/plugin/tool-input-repair.ts:1-30`).

### MCP

MCP servers are a transformable domain (`plugin/src/effect/mcp.ts`). Config
feeds them through `opencode.config.mcp`. In 2.0 every MCP server is
"Code Mode" by default: its tools become functions in a small native
JavaScript interpreter (`codemode/`), and the model writes one script that
chains calls instead of making one tool call per hop. A defaults plugin turns
Code Mode off for servers that already do it themselves
(`core/src/plugin/mcp-codemode-defaults.ts:6-41`).

### Client and server

One background server process per machine owns sessions, plugins and
providers. The TUI, web, desktop and scripts (`@opencode/client`) are all
clients of it. Other devices pair with a one-time code that expires after
five minutes (`server/src/pairing.ts:7-30`). TUI plugins are a separate
entrypoint. They render into named slots such as `session.composer.top`,
`sidebar.content` and `sidebar.footer`, and they read server state
(`plugin/src/tui/context.ts:198-201`).

## 2. What OpenCode 2.0 changes

From the note, checked against the code:

| Change | In the note | In the code |
| --- | --- | --- |
| Built-ins are plugins | 1.0's plugin system "never got the attention it needed". In 2.0 nearly every feature is an internal plugin, "80+ of them", so the API stays maintained. | 83 distinct `opencode.*` plugin IDs. Tools, agents, Plan, providers, prompts, VCS, web search and config readers all register through the public `@opencode/plugin/effect` API (`internal.ts:216-270`). The v2 plan states the goal: "Internal and external plugins use the same public plugin API" (`plugin/src/effect/PLAN.md:9`). |
| Transform and hook model | Not described. | Replaces 1.0's returned-hooks object. Setup "does not return hooks" (`PLAN.md:46`). The catalog design record picked replayable, Location-scoped transforms over whole-config transforms, so one changed domain reloads alone (`specs/v2/catalog-config-plugin-lifecycle.md`). |
| Hot reload | Everything hot-reloads, so OpenCode can write itself a plugin mid-session and use it. | `plugin/src/source.ts`, `supervisor.ts:205-247`. |
| TUI plugins stable | Built on OpenTUI. | `plugin/src/tui/`. |
| One local server | The TUI, web and desktop all talk to one central process. Clients find it automatically. Pair other hosts with `/pair`. | `server/`, `server/src/pairing.ts`. |
| New API | Rebuilt from scratch. OpenCode can call its own API, for example to move the session into a worktree. | `protocol/`, `client/`. The `opencode.tools` plugin exposes the API to the agent. |
| Code Mode | MCP and OpenAPI become callable functions, and every MCP server opts in by default. | `codemode/`, `mcp-codemode-defaults.ts`. |
| Background work | Background subagents, async shell with notification, and Ctrl+B to background a running command or subagent. Can use "a different model". | `subagent.ts:44-61`, `core/src/session/subagent-job.ts`. |
| SDK | `@opencode/sdk` is the CLI's own code, usable as a library and able to run in Durable Objects. | `sdk/`. |
| Breaking | "1.0 plugins will not work in 2.0." The server API is completely different. `tui.json` folds into the main config. | |

The note says nothing about modes or routing. In the code, 2.0 keeps
user-picked primary agents and model-picked subagents. Plan stays a
user-selected agent, now built as a plugin.

## 3. Comparison with Coder and #11451

| Concern | OpenCode 2.0 | Coder today | #11451 plan |
| --- | --- | --- | --- |
| What a plugin is | Executable TS module with `id` + `effect`/`setup` | `PluginDefinition` const: id, name, `default_enabled`, `tools: &[ToolBinding]`, `model_provider`, `composer_rails`. It "names supported host bindings; it does not supply executable code" (`plugin_definition.rs:55-65`). | Adds harness plugins as typed declarations next to it (#11452). |
| Built-ins on the same API | Yes, 83 of them. | Partly. Claude Code and the Coder loop are `DEFINITIONS` entries, but the shell, file, web, memory, plan, fleet and connection tools are hard-wired in `ExecutionSettings::defs` (`plugin_tools.rs:192-276`) and dispatched by name in one `match` (`plugin_tools.rs:383+`). | "Everything is a plugin" says yes. No issue covers moving the hard-wired tools yet. |
| Third-party plugins | npm packages, in-process, full authority | Installed packages under `~/.openagents/extensions/<key>/<slug>/<version>/`, off by default. Today they contribute only background rules, and "the plugin describes; the host executes" (`crates/background/src/plugins.rs:1-17`). | Unchanged. |
| Extension points | Transforms on 11 domains, hooks on tool, session, permission, shell and aisdk | A fixed binding enum per capability, plus composer rail slots (`RailSlot`, `resolve_composer_rails`) | Harness contract: placement, payer, capabilities, health, start and continue a turn (#11452) |
| Config | `plugins` array, `-id` removal, wildcards, guarded IDs | Per-plugin enable plus private settings and keys (`plugins.rs`, `bundled_settings.rs`, `plugin_store.rs`) | Sliders plus a routing-preferences doc (#11455) |
| Modes / agents | Primary agents switched by Tab (Build, Plan), and subagents chosen by the model | `Mode::{Demo, Live, Claude, Solver}`, with Shift+Tab to Claude (`lib.rs:117`) | No modes. Mode::Claude goes away (#11453). |
| Routing | None. The person picks the agent; the model picks the subagent by description. | Per-run harness `Config` chosen by a Jev cell, with promotion on paired wins (`harness.rs`) | Code filters candidates, then System One (Clef/Jev) ranks and picks, invisibly, at work boundaries (#11451 phase 2) |
| Routing evidence | None | ATIF `extra.harness` | Routing records with probabilities and outcomes (#11454) |
| Permissions | One `Ruleset` of `{action, resource, effect}` (last match wins) per agent, plus an `evaluate` hook and managed policy | `approval::tools_allowed` gate; bypass in Claude Code | Bypass now; one policy with floors across harnesses later (phase 5) |
| MCP | Domain plus Code Mode by default | Coder tools are exposed *to* Claude Code over MCP (`claude_tools.rs`). The Coder loop does not consume MCP servers. | #11450 (openagents MCP) |
| Client/server | One local server per machine; TUI, web, desktop and paired devices are clients | Linked computer for phone and web | Gateway fallback when the computer is offline (#11456) |
| Hot reload | Yes, including self-authored plugins | Picker re-scans the installed list once a second (`plugin_catalog.rs`) | — |

Coder is already ahead in one place, and the plan rests on it: choosing per
task, with measured promotion. OpenCode has no router. Its "routing" is a Tab
key and tool descriptions. OpenCode is ahead in a different place: its plugin
API is the system's own internal API, and that keeps it whole.

## 4. Recommendations

**Adopt**

1. **Run every built-in through the plugin contract** (#11452, plus a new
   issue). This is OpenCode 2.0's central lesson. 1.0's API rotted because
   nothing inside used it. Today `ToolBinding` is an enum the host matches
   on, and most tools bypass `DEFINITIONS` altogether. New issue, *"Coder
   built-ins register through the plugin contract"*:
   - each built-in plugin (shell, files, web, memory, plan, workflow,
     fleet, connections, and each harness) supplies its tool definitions,
     instructions and dispatch through one Rust trait object;
   - `ExecutionSettings::defs` and `execute` become a loop over enabled
     registrations;
   - the harness contract in #11452 is the first user of that trait, not a
     parallel one.
2. **Give the candidate set replayable transforms with reasons** (#11452,
   #11454).
   - OpenCode's registry is useful here. Each source contributes a
     transform; reads replay them in order onto a fresh value and cache it
     until a registration changes.
   - For Coder: each harness plugin contributes its candidate entry. Policy,
     placement (#11456), budget and preferences (#11455) are later
     transforms that remove entries, each with a typed reason.
   - The router reads one materialized snapshot. The routing record (#11454)
     stores that snapshot's exclusion reasons as they are, so "why not
     Claude?" can always be answered.
   - Keep the edits typed (`Excluded { harness, reason }`), not free-form.
3. **One permission ruleset, with an evaluate hook and guarded floors**
   (#11451 phase 5; new issue *"Coder policy floors as a guarded rule
   plugin"*).
   - Use OpenCode's shape: `{action, resource, effect: allow|ask|deny}`,
     last match wins, per agent, plus an `evaluate` hook and IDs that
     repository config cannot remove.
   - Coder's floors (production deploy, merge, secrets, payments) become one
     guarded plugin, evaluated in Coder's tool layer and in Claude Code's
     pre-tool hook. Then a harness running in bypass still cannot cross a
     floor.
   - Policy that blocks a whole plugin (`integration.use plugin:<id>`) is
     worth copying too.
4. **Repair tool input against the schema before execution** (new issue,
   *"Coder loop: schema-guided tool-input repair"*).
   - `opencode.tool.input.repair` handles string numbers, stringified
     objects, null placeholders and extra keys on closed objects, only where
     the schema leaves no ambiguity.
   - This helps most on cheap models, the Haiku 5.5 and Luna tiers the
     router will pick for small work. It is pure code: no model call and no
     keyword routing.
   - Measure repaired calls per harness in the routing records.
5. **Route at boundaries, and keep the prefix cached** (#11453, #11451
   decision 1).
   - OpenCode's Plan plugin changes behavior with a system reminder near
     the tail, not by rewriting the system prompt. Background subagents
     return at once and notify on completion. A model override is allowed
     only when the person asks.
   - These match the owner's rules: route each new piece of work, don't
     bust the cache, no user-visible switch.
   - When #11453 removes Mode::Claude, constraints that change during a
     chat should go in as tail reminders. Harness changes should happen only
     at work boundaries.
6. **Tag every request with its kind** (#11454). OpenCode tags auxiliary
   calls `primary | compaction | title | generate`. Routing records and cost
   should carry the same tag, so a title or compaction call on the gateway
   is not counted as the harness's work.
7. **Try Code Mode for MCP, behind a Gym measurement** (new issue, related
   to #11450).
   - Let the Coder loop call MCP/OpenAPI tools from one script, chaining
     calls in one round trip.
   - Adopt it only if the paired runs in `harness.rs` show fewer turns at
     equal pass rate. It is an action-space change, the kind the harness
     study found depends on the model.

**Avoid**

- **Agent cycling as the main control.** Tab-cycling between Build and Plan
  is a mode switch under another name. #11453 removes ours, so it should not
  come back as "agents". If a Plan-like constraint is needed, it is a
  plugin-applied constraint the router or the person turns on, not a mode.
- **Unsandboxed npm plugins with full authority.** OpenCode loads arbitrary
  packages in-process and relies on policy. Coder's installed plugins follow
  "the plugin describes; the host executes" (`background/src/plugins.rs`).
  Keep that boundary. Third-party plugins contribute declarations (harness
  entries, rules, rails, tool schemas); executable code stays in signed
  built-ins or out-of-process MCP/ACP servers.
- **Hot-loading self-written executable plugins.** "OpenCode writes itself a
  plugin and uses it immediately" conflicts with that boundary and with the
  floors in recommendation 3. Hot reload of *declarative* plugin files is
  fine, and the catalog loader already re-scans.
- **Per-model behavior chosen by substring.** `optimize.ts` picks prompts by
  checking whether `model.id` contains "gpt" or "claude". Coder's harness
  cells are typed Jev judgments with paired promotion. Keep that.
- **Model choice by subagent description alone.** OpenCode has no ranker.
  It trusts the calling model to read descriptions. #11451 already decides
  otherwise: code filters, System One ranks, and every choice is logged.

**Top five:**

1. Built-ins through one plugin trait (#11452, plus a new issue).
2. A candidate registry built from replayable transforms with typed
   exclusion reasons (#11452, #11454).
3. One rule set with guarded floor plugins enforced in every harness
   (#11451 phase 5, new issue).
4. Schema-guided tool-input repair (new issue).
5. Tail reminders and boundary-only harness switches instead of modes
   (#11453).
