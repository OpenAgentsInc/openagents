---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: docs/research/terminal-agents/codex.md, docs/research/terminal-agents/gemini-cli.md, docs/research/terminal-agents/hermes-agent.md, docs/research/terminal-agents/opencode.md, docs/research/terminal-agents/pi.md, docs/research/terminal-agents/openagents-current-state.md, INVARIANTS.md, AGENTS.md
informed: OpenAgents contributors, agents, Khala CLI operators, Khala Code desktop operators, and Pylon operators
---

# Adopt a native Khala terminal tool runtime

## Context and Problem Statement

OpenAgents already has substantial terminal-agent infrastructure, but it is
split across Pylon delegated Codex/Claude execution, Probe computer-use tools,
Khala CLI fleet orchestration, and research notes on terminal-agent systems.
Khala Code desktop and Khala CLI need a shared OpenAgents-native tool runtime
that works even when no external coding agent is installed, while preserving
the authority boundaries required by the repository invariants.

The terminal-agent research set compared Codex, Gemini CLI, Hermes Agent,
OpenCode, Pi, and the current OpenAgents implementation. Those studies converge
on the same architectural need: first-party typed tools, a central executor,
scoped approvals, bounded output, structured UI events, and explicit separation
between local private artifacts and public-safe receipts.

## Decision Drivers

* Keep Khala's built-in coding tools OpenAgents-owned and reusable across
  desktop, CLI, and future Pylon fallback lanes.
* Avoid making shell the only file/search/edit interface.
* Preserve Effect and Effect Schema as the shared runtime and contract model.
* Reuse Probe and Pylon implementation slices rather than building a new
  isolated tool system.
* Keep Codex and Claude SDKs as delegated execution lanes, not the only Khala
  tool authority.
* Prevent public requests, MCP tools, plugins, or project-discovered tools from
  smuggling owner-local danger authority.
* Keep model-visible output, UI display, private artifacts, and public-safe
  summaries as separate result lanes.

## Considered Options

* Build a shared `@openagentsinc/khala-tools` runtime with Effect Schema-first
  built-ins and scoped authority services.
* Continue to rely on Codex and Claude SDK tools for all coding work.
* Reuse Probe tools directly as the Khala tool runtime without a new shared
  package.
* Make shell execution the primary native Khala tool and let models use shell
  for read, search, edit, and patch operations.
* Adopt a third-party terminal-agent tool layer wholesale.

## Decision Outcome

Chosen option: "Build a shared `@openagentsinc/khala-tools` runtime with
Effect Schema-first built-ins and scoped authority services", because it gives
Khala a native tool contract while reusing the strongest existing OpenAgents
pieces: Probe for browser, PTY, scoped filesystem, and timeline primitives;
Pylon for workspace materialization, approval queue, assignment closeouts, and
public-safe reporting; `packages/ui` for diff review.

The accepted native core catalog is:

* `read`
* `ls`
* `glob`
* `grep`
* `edit`
* `write`
* `apply_patch`
* `exec_command`
* `write_stdin`
* `ask_user`
* `todo_write`
* `view_image`

`web_fetch`, `web_search`, browser tools, memory, skills, cron, integrations,
MCP, plugins, and project-discovered tools are optional policy-scoped toolsets,
not the default coding catalog.

### Consequences

* Good, because Khala Code desktop and Khala CLI can share one event, approval,
  output, and artifact contract.
* Good, because narrow file/search tools reduce unnecessary shell authority.
* Good, because Codex/Claude delegated lanes can stay productive while the
  native runtime grows as a provider-neutral substrate.
* Good, because public-safe summaries and private local artifacts are separated
  by contract instead of convention.
* Bad, because this adds a new package and consolidation step before native
  Khala tools are fully available.
* Bad, because existing Probe and Pylon primitives need wrapping or promotion
  work before the runtime is cleanly shared.

### Confirmation

Compliance is confirmed by:

* `docs/research/terminal-agents/2026-06-29-openagents-khala-tool-decisions.md`
* future `packages/khala-tools` tests for path escapes, stale edits, output
  bounding, permission denial, unknown/stale tool calls, and redaction of
  public-safe summaries
* Khala Code desktop and Khala CLI consuming the same tool event protocol
* review rejecting public/request-level danger flags or un-namespaced external
  tool shadowing of built-ins

## Pros and Cons of the Options

### Shared `@openagentsinc/khala-tools` runtime

* Good, because it matches OpenAgents' Effect and Effect Schema direction.
* Good, because it can reuse Probe and Pylon without coupling Khala desktop to
  product Worker code.
* Good, because it makes approval, sandbox, output, and event decisions
  explicit.
* Bad, because it requires integration work across existing subsystems.

### Continue to rely on Codex and Claude SDK tools

* Good, because those lanes already work for Pylon delegated coding capacity.
* Good, because they provide mature tool behavior immediately.
* Bad, because Khala would lack a native no-external-agent fallback.
* Bad, because SDK-local tool events, permissions, and artifacts are not one
  OpenAgents-owned contract.

### Reuse Probe tools directly

* Good, because Probe already has scoped filesystem, terminal, browser, tool,
  and timeline primitives.
* Bad, because Probe's default permission handler allows by default and is not
  sufficient as Khala product policy.
* Bad, because Probe does not yet expose the full native coding catalog,
  especially `grep`, exact `edit`, and grammar-backed `apply_patch`.

### Make shell the primary native tool

* Good, because it is simple to implement and flexible.
* Bad, because it broadens authority for routine read/search/edit actions.
* Bad, because shell output and shell safety are harder to bound, approve, and
  render than narrow typed tools.

### Adopt a third-party tool layer wholesale

* Good, because it could accelerate early functionality.
* Bad, because it would conflict with OpenAgents' Effect boundary, workspace
  policy, public-safe receipt model, and Pylon/Probe reuse goals.

## More Information

* `docs/research/terminal-agents/2026-06-29-openagents-khala-tool-decisions.md`
* `docs/research/terminal-agents/openagents-current-state.md`
* `docs/research/terminal-agents/opencode.md`
* `docs/research/terminal-agents/codex.md`
* `docs/research/terminal-agents/gemini-cli.md`
* `docs/research/terminal-agents/hermes-agent.md`
* `docs/research/terminal-agents/pi.md`
* `INVARIANTS.md`
* `AGENTS.md`
