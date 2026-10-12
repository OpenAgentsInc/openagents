# DeepSeek Harness plugins, foreign harnesses and model choice, against Coder's harness plan

Source: the read-only reference clone at
`~/work/projects/repos/deepseek-harness`, read at `d743267388`
(2026-10-09, release `dsh-0.2.1-alpha.2`). All `path:line` citations are
relative to that clone. Nothing was installed or run.

This builds on [`docs/coder-new/deepseek-harness-lessons.md`](../coder-new/deepseek-harness-lessons.md)
(snapshot `5badb15`, October 6), which already covers Cordis services and
lifecycle, installed/enabled/active states, durable events and projections,
the inbox controls, deny-only guards, tool concurrency, compaction, skills,
subagent depth accounting, and the plugin manager. None of that is repeated
here. This report reads the clone for the questions
[#11451](https://github.com/OpenAgentsInc/openagents/issues/11451) raises:
how DSH treats Claude Code, Codex and other foreign agents; how it handles
permissions and models across them; and whether anything routes work
between them. Two packages discussed here, the Claude Code and Codex hook
bridges, landed after the earlier snapshot.

Short answer: DSH has no router. It has a good, honest contract for
foreign harnesses and model routes, and Coder should copy that contract
for #11452. The routing, logging and learning that #11451 plans are ours
alone.

## 1. How DeepSeek Harness structures it

### What a plugin is and how it is loaded

A plugin is a row in a Cordis profile: `id`, package `name`, optional
`config`, optional `disabled`. A package that wants to be switchable from
the Plugins page ships a `cordis.patch.yml` layer that inserts its rows.
The Claude Code bundle is the clearest example. Its whole layer is two rows:
the provider, and a generic delegation tool configured to point at it
(`packages/subagent/subagent-claude-code/cordis.patch.yml:1-9`). Enabling it
installs the package and selects the layer. Switching it off deselects the
layer, and Remove is a separate operation. Live agents see the change on
their next request, and the installer says when a restart is needed
(`packages/subagent/subagent-claude-code/README.md:32-34`).

### Two planes: host and agent presets

The Web profile splits rows into a host plane, with registries, the job
registry, the skill registry and the shell environment, and an agent plane:
the tools, prompt sections and delegation backends that one agent
contributes. Agent-plane rows are disabled in the host profile, and each
session mounts an **agent preset** instead
(`packages/bundle/web-app/cordis.patch.yml:448-500`). The shipped presets
are `minimal`, `standard`, `ptc` and `cordis`
(`packages/bundle/web-app/presets/`). A preset is ordinary Cordis YAML
listing child plugins: persona, instructions, tools, and a plan-mode group
(`packages/bundle/web-app/presets/cordis.patch.yml:1-60`). Bundle rows such
as `subagent_claude_code` are inserted as host rows, so they are global
tools in every preset. A preset tool with the same name shadows the global
one (`packages/subagent/subagent-claude-code/README.md:59`).

Plan mode is a group inside the preset, not a different tool set. The
preset's prompt says outright that "the tool catalog stays the same across
modes for request-cache stability" and that mutation tools stay listed and
are refused by the rules instead
(`packages/bundle/web-app/presets/standard.patch.yml:59`).

### Tools, skills, instructions and hooks

- **Tools** register into a scoped registry with deny-only final guards.
  The earlier lessons doc covers this.
- **Instructions:** `agent-instructions` reads `AGENTS.md` and `CLAUDE.md`
  by default (`packages/context/agent-instructions/src/config.ts:12`) under
  a byte budget that drops broader files first.
- **Skills:** filesystem skills come from project roots, custom roots and a
  shared `~/.agents` home (`packages/skill/skill-filesystem/src/index.ts:51-71`).
- **Hooks (new since the earlier survey):** two experimental bridges run
  the person's *existing* Claude Code or Codex `hooks.json` inside DSH's own
  loop. The Claude Code bridge maps `SessionStart`, `UserPromptSubmit`,
  `PreToolUse`, `PostToolUse`, `Stop`, `SubagentStart` and `SubagentStop`
  onto DSH decision points. Hooks can block a prompt or tool with a reason
  the model sees, add context, or force another step
  (`packages/experimental/hooks-claude-code/README.md:56-75`,
  `src/index.ts:223-301`). The Codex bridge supports five of these points
  (`packages/experimental/hooks-codex/README.md`). Both bridges fail soft:
  an unreadable config logs a warning and runs no hooks, and a crashing hook
  is logged while the agent continues.

### Foreign agents as subagent providers

Claude Code, Codex, any ACP agent and DSH itself are all **providers** on
one subagent seam (`ctx.subagents`). The provider contract
(`packages/subagent/subagent/src/types.ts:323-365`) has a name and a
**capability record** with five booleans: `agentOptions` (provider, model
and effort overrides), `outputSchema`, `depthLimit`, `toolFilter` and
`persona` (`types.ts:123-129`). It also has a descriptive
`inheritsParentContext` flag, optional static `agentRouteDefaults`, and
`start()`, plus an optional `prepareContinuable()` whose presence marks a
provider whose children can continue.

The seam enforces honesty at three points:

1. At start, `assertCapabilities` rejects the first requested feature the
   provider lacks, with `UNSUPPORTED_CAPABILITY`
   (`packages/subagent/subagent/src/index.ts:626-645`).
2. Out-of-process backends advertise `NO_START_CAPABILITIES`, so a child
   in another process is "never accepted-then-ignored"
   (`packages/subagent/subagent/src/out-of-process.ts:51-61`).
3. At load, the delegation tool refuses a configuration that needs a
   capability its provider lacks, such as a depth cap or model selection
   (`packages/subagent/tool-subagent/src/index.ts:176-193`). It also derives
   its model-facing wording from `inheritsParentContext` (`:212-220`).

The Claude Code and Codex providers share one design:

- **One shot.** Each run is a fresh session, thread and turn with no
  continuation or resume. Only the final answer, or a safe diagnostic,
  enters the parent. Reasoning, tool traffic, stderr and diffs stay out
  (`packages/subagent/subagent-claude-code/README.md:12`,
  `packages/subagent/subagent-codex/README.md:12`).
- **Native configuration is authoritative.** Claude Code reads the host's
  normal settings. The provider overrides only `model`, `permissionMode`
  and `env`.
- **Pinned runtime.** The package ships a platform-pinned runtime and
  "never falls back to the host `claude` executable" (Claude README:12).
  A missing payload leaves the provider dormant, and the first delegation
  fails at startup with a fixed-category diagnostic.
- **Credentials are scrubbed.** Credential-shaped environment variables are
  removed before an explicit `env` overlay, so a key meant for the child
  must be given on purpose (`subagent-claude-code/src/index.ts:44-47,62`).
- **Permissions are named per harness.** Claude Code's `permissionMode`
  defaults to `dontAsk` and can be `acceptEdits`, `auto`, `plan` or
  `bypassPermissions` (Claude README:45-56). Codex has `never`,
  `approve-for-me`, which sends requests to Codex's automatic reviewer, and
  `dangerously-bypass-approvals-and-sandbox` (Codex README:51-53). The
  bypass values must be chosen explicitly.
- **Diagnostics are safe.** A failure becomes one fixed line naming the
  provider, stage and category. It never includes stderr, paths, task text
  or credentials, and the shared result boundary caps it at 4096 bytes
  (`packages/subagent/subagent-acp/README.md`, "Safe diagnostics").

### Permissions

`permission-presets` offers named modes, `read-only`, `workspace-write`
and `danger-full-access`. Each mode sets the sandbox and approval together,
while each enforcement service keeps its own value
(`packages/bundle/base/cordis.patch.yml:254-272`). If the person adjusts
the knobs so they match no preset, the state shows as `custom`, which can
be left but not selected
(`packages/interaction/permission-presets/src/index.ts:79`, README:12).
Approval fails closed when no answerer is available
(`packages/interaction/user-approval/README.md`).

### Models and providers

`ctx.llm` is an adapter registry. `registerAdapter(providers, adapter)`
refuses duplicates (`packages/llm/llm/src/index.ts:389-431`). There are
also configurable-provider declarations that may be registered or dormant
(`:483`) and per-provider model discovery (`:557`). `agent-default-model`
supplies the default provider, model and effort for new sessions.

A child model is chosen through the delegation tool. A setting holds an
**exact allowlist of `(provider, model)` routes**
(`packages/subagent/tool-subagent/src/model-selection.ts:9-26`,
`model-selection-settings.ts:23-39`). Any explicit model choice by the
parent must resolve to an allowed route (`model-selection.ts:139`). The
route is then preflighted against the live adapter before the child exists
(`:167-176`). Pure inheritance needs no allowlist entry.

### Routing

There is no automatic router. A search of the packages for router or
classifier logic finds only HTTP routing and provider "routes." Work is
chosen in two ways:

- **By the person:** the default model, the session's agent preset and the
  permission preset.
- **By the parent model:** it calls a named delegation tool
  (`subagent_claude_code`, `subagent_codex`, or `subagent` with a provider),
  optionally with an allowlisted model route.

Nothing estimates cost, speed or the chance of success, and nothing logs a
choice against its outcome.

## 2. What is distinctive or better than what we do

1. **The capability record and refusal.** Every provider declares what it
   can honor, and a request for anything else is refused at start or at
   load, never silently dropped. Coder has no equivalent. `ToolBinding` and
   `AcpAgent` say what exists, not what each engine can do
   (`crates/coder-new/src/plugin_definition.rs:25-45`,
   `bundled_runtime.rs:95-107`). #11452 asks for "what it can do," and this
   is the shape to use.
2. **A cache-stable tool catalog.** DSH keeps the catalog identical across
   modes and refuses calls instead of hiding tools. Coder's catalog changes
   per request: `ExecutionSettings::for_request` trims agents and Microcoder
   by request text (`crates/coder-new/src/plugin_tools.rs:122-162`), and the
   harness filter retains a subset (`:270`). Owner decision 1 in #11451 is
   "don't bust the prompt cache"; DSH shows how to meet that at the tool
   layer.
3. **No keyword routing.** Coder's `requested_agents`
   (`plugin_tools.rs:765+`) splits the request into words and matches agent
   names against a list of directive verbs ("ask", "use", "run",
   "delegate"…). That is the ad hoc string routing the workspace rules
   forbid for tool selection. DSH sidesteps it entirely: a named agent is a
   named tool the model calls.
4. **Hook bridges as portable policy.** Running the person's existing
   `.claude` or `.codex` hooks inside a different loop means a
   `PreToolUse` gate follows the work whichever engine runs it. Today a
   Claude Code harness turn in Coder loads the person's hooks natively
   (`crates/coder-new/src/claude.rs:1-16`), but the Coder loop and Codex
   ignore them. That is exactly the gap #11451 phase 5 ("policy floors
   across harnesses") has to close.
5. **One permission name mapped per harness.** One preset name drives the
   sandbox and approval knobs, each foreign harness has an explicit table
   from DSH's mode to its native mode, and drifted knobs show as `custom`.
   Coder hard-codes bypass in each engine
   (`bundled_runtime.rs:827`, `claude.rs:670-671`) and states it in prose
   instructions (`plugin_tools.rs:279+`).
6. **An exact route allowlist checked at dispatch.** A model choice by the
   parent, or by a preference, is valid only if it resolves to an exact
   allowed pair, and it is preflighted before any process starts.
7. **Fixed-category, size-capped diagnostics.** These are safe to log and
   show, which suits routing records.

Where Coder is ahead, and should stay ahead:

- **Learned selection.** `harness.rs` picks a per-run configuration from
  typed Jev judgments, explores a bounded share of runs, and promotes a
  change only on a paired win (`crates/coder-new/src/harness.rs:1-21`).
- **Preference-ordered fallback.** `route.rs` orders model doors by the
  person's cost, speed and intelligence favor and explains refusals
  (`crates/coder-new/src/route.rs:1-128`).
- **A planned router.** #11451 adds System One ranking with logged
  probabilities and outcomes.

DSH has none of these. Its foreign harnesses are also one-shot and
output-only, whereas Coder runs Claude Code as the *primary* turn with full
streaming and continuation. That is a harder contract, and the right one
for owner decision 1.

## 3. Comparison

| Concern | DeepSeek Harness | Coder today | #11451 plan |
| --- | --- | --- | --- |
| Unit of extension | Cordis row; bundles ship a profile patch layer | Static `PluginDefinition` table of host bindings (`plugin_definition.rs:57-183`); installed packages listed from the registry (`plugin_catalog.rs`) | Harness plugins declared next to `plugin_definition.rs` (#11452) |
| Composition per session | Agent presets (`minimal`, `standard`, `ptc`, `cordis`), chosen per session | One `ExecutionSettings` snapshot per turn, including the harness config (`plugin_tools.rs:20-78`) | One composer and no modes (#11453) |
| Foreign engines | Subagent providers (Claude Code, Codex, ACP, DSH SDK); one-shot, final answer only | Claude Code as a mode with full turns (`claude.rs`); Codex and ACP agents through `acp_subagent` | Harness plugins as turn executors; follow-ups continue the session |
| Capability declaration | `SubagentCapabilities` with five booleans plus `inheritsParentContext` and continuable-by-presence; refusal at start and load | None per engine | "What it can do" list in the contract (#11452) |
| Health | Dormant until first use; fails at startup with a fixed category | Sign-in checks per plugin (`claude.rs:45-52`); credit refusals in `route.rs` | Cheap health check makes a candidate (#11452) |
| Who chooses the engine | The person (preset or default model) or the parent model (delegation tool) | The person (Shift+Tab mode); keyword match on agent names (`plugin_tools.rs:765`); `route.rs` door order for `auto` | Code filters, System One ranks, an explicit pin wins |
| Model choice | Exact `(provider, model)` allowlist, preflighted | `/models` pick; favor-weighted door order | Sliders set λ; preferences doc (#11455) |
| Permissions | Named presets mapped per harness; `custom` derived; approval fails closed | Bypass everywhere, stated in prose | Bypass now; one policy with floors later (phase 5) |
| Hooks | Native guards plus Claude Code and Codex `hooks.json` bridges | Claude Code's hooks only inside the Claude Code harness | Not yet planned |
| Prompt cache | Tool catalog fixed across modes | Catalog varies per request and per harness | Route at work boundaries |
| Instructions and skills | `AGENTS.md` and `CLAUDE.md` chain; skill roots including `~/.agents` | Memory and project instructions (`memory.rs`); Claude Code's own | System One selects context per harness (phase 3) |
| Records of choices | None | Harness config in ATIF `extra.harness` | Routing record per choice with probabilities and outcome (#11454) |
| Credential handling for children | Scrub credential-shaped variables, then an explicit overlay | `env_remove` lists (`bundled_runtime.rs:556-589`, `claude.rs:700`) | Unspecified |

## 4. Recommendations

### Adopt

1. **#11452: put a capability record in the harness contract, and refuse
   requests a harness cannot honor.** Use typed booleans or enums, not
   prose:
   - continues a session;
   - streams a transcript or returns only a final answer;
   - accepts a model override, and which models;
   - honors a tool filter;
   - returns structured output;
   - takes image input;
   - the permission modes it supports;
   - placement (this computer, a linked computer, or the gateway).

   The router's candidate filter reads this record, and a pin to a harness
   that lacks a needed capability fails with a reason instead of degrading
   quietly. Model it on `types.ts:123-129` and `index.ts:626-645`. Make
   continuation known by declaration, as DSH does by the presence of
   `prepareContinuable`.
2. **#11453: keep one stable tool catalog across harnesses and turns, and
   delete the keyword matcher.** `for_request` and `requested_agents`
   (`plugin_tools.rs:122-162,765+`) should go. An explicit agent name in the
   request becomes a typed pin, extracted by the router's System One
   question set or by the model calling a named delegation tool. Tools that
   are off should be refused at execution with a plain reason, not removed
   from the catalog mid-chat. This closes a workspace-rule violation and
   protects the prompt cache that owner decision 1 asks for.
3. **#11452 and #11451 phase 5: one Coder permission name, mapped to each
   harness's native modes in a table.** Each harness plugin declares a
   table like DSH's (Claude Code `dontAsk`, `acceptEdits`, `auto`, `plan`,
   `bypassPermissions`; Codex `never`, `approve-for-me`, `bypass`). Today
   every entry maps to bypass, per the owner's decision. Phase 5 then
   changes one policy value instead of editing engines. Drifted settings
   show as "custom," and a missing answerer fails closed.
4. **New issue: Claude Code and Codex hook bridges for the Coder loop and
   the Codex harness.** Read the person's `.claude/settings.json` hooks and
   `.codex/hooks.json`. Run them at the Coder loop's matching points
   (prompt submit, before and after a tool, stop) and, where Codex's
   app-server exposes them, in the Codex harness. A `PreToolUse` block then
   holds whichever harness the router picks. Phase 5 floors (production
   deploy, merge, secrets, payments) can ship as host-owned hooks on the
   same points, deny-only, so a later hook cannot override a floor. Done
   when the same `hooks.json` blocks the same command under all three
   harnesses. This belongs in #11451 phase 5; file it now because phase 1
   harness work decides where the hook points live.
5. **#11455: compile "never send this repo to X" into an exact allow or
   deny list of `(harness, model)` routes checked at dispatch.** Do not
   pass it to the router as a soft input. Use `model-selection.ts:9-26,139`
   as the shape: an exact pair, validated when the preferences doc is
   written, and preflighted before a process starts. Softer preferences
   ("use Claude less") stay router inputs that adjust λ or the priors.
6. **#11454: routing records carry fixed-category exclusion reasons and
   capped, scrubbed diagnostics.** For each excluded or failed candidate,
   record `{harness, stage, category}` from a closed set: signed-out, over
   budget, capability missing, placement offline, startup failed, limit,
   access policy. Never record raw stderr or paths, and cap each diagnostic
   at a few KB. Also record each harness's runtime version, so outcome
   drift after a Claude Code or Codex upgrade shows up in the Gym table.

### Avoid

- **Making foreign harnesses one-shot and output-only.** DSH does this
  because continuation across someone else's runtime is hard. It conflicts
  with owner decision 1, where follow-ups continue the harness session.
  Keep DSH's output-only shape only for *delegation* (Solver, background
  agents, `acp_subagent`) and say which shape applies in the capability
  record (#11452).
- **Visible presets and permission pickers.** DSH's agent-preset and
  permission-preset selectors are modes under another name. Under #11451,
  presets are plugin configuration that the router and policy read, and
  the person never picks one (#11453).
- **The parent model choosing the engine as the main routing mechanism.**
  That is DSH's only mechanism, and it spends a frontier turn to decide,
  with no logged probabilities. Keep delegation tools for work the model
  hands off, and let System One choose the turn's harness (#11451 phase 2).
- **Pinning bundled Claude Code and Codex runtimes for now.** DSH ships
  pinned payloads and never uses the host CLI. Coder's harnesses
  deliberately run on the person's own logins. Record versions instead
  (recommendation 6) and revisit if upgrades cause measured regressions.
- **Host-process JavaScript plugins.** This repeats the earlier lessons
  doc: keep built-ins as Rust host bindings and third-party code behind
  the Wasm/NIP-EXT boundary.

### Top five

1. A capability record in the #11452 harness contract, with refusal
   instead of silent degradation.
2. A stable tool catalog with no keyword agent matching
   (`for_request`/`requested_agents`), folded into #11453.
3. One Coder permission name mapped per harness, bypass for now
   (#11452, then #11451 phase 5).
4. A new issue for Claude Code and Codex hook bridges, so hooks and
   floors follow the work across harnesses.
5. Hard exact-route allow and deny lists from the preferences doc
   (#11455), with fixed-category exclusion reasons in routing records
   (#11454).
