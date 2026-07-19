# Agent Runtime Schema

`@openagentsinc/agent-runtime-schema` is the RK1 schema-only Agent Runtime Kernel
contract. It defines the durable OpenAgents runtime run and event log shape used
by native, external, hosted, and fixture loops.

The package intentionally contains no executor, provider SDK, or Vercel AI SDK
dependency. Adapter-specific loops project into this contract at the boundary.
worker and UI surfaces consume event logs and projections derived from this
contract.

Day 1 of the Fast Follow thread-fabric program adds
`openagents.runtime_control_intent.v2` and
`openagents.runtime_control_outcome.v1`. The provider-neutral intent keeps
`turn.queue`, `turn.steer`, and `turn.interrupt` distinct, binds stable command,
message, thread, turn, generation, ordering, expiry, and origin identity, and
contains refs rather than raw prompt content. The outcome keeps admission,
delivery, and terminal observation separate so a queued or accepted command is
never misreported as applied or completed.

The same program's rebuildable canonical search projection consumes only
owner-local `openagents.thread_export_artifact.v1` bundles. It returns exact
original thread/event refs with their accepted, superseded, or reverted state,
uses bounded deterministic text filtering only after the search route is
selected, and reports index/result truncation explicitly. It is not transcript,
acceptance, persistence, disclosure, transport, or renderer authority.

For Khala Code's AI SDK-shaped runtime work, the package also defines:

- `openagents.khala_runtime_event.v1` for canonical text, reasoning, step,
  tool, usage, provider metadata, file change, compaction, interruption, and
  raw-sidecar events, plus thread-scoped `writeback.recorded` metadata when a
  coding turn publishes a branch or pull request.
- `openagents.khala_runtime_control_intent.v1` for mobile/desktop/server
  control intents such as append message, start/interrupt/continue turn, close
  turn, and tool approve/deny.
- `openagents.runtime_interaction.v1` for private, provider-neutral questions,
  tool approvals, and plan reviews. One pending/resolved/expired/revoked
  lifecycle carries exact thread/turn/interaction identity, a server deadline,
  bounded display-safe choices, and kind-matched decisions. Exact retries are
  duplicates. Conflicting reuse and late or revoked decisions fail closed.
- Structural mappers from existing `AgentRuntimeEvent` records and dependency
  free AI SDK `TextStreamPart`-shaped objects into Khala runtime events.

Raw provider chunks, local paths, tool inputs/results, and raw prompts should
stay behind private refs. Tool events require an explicit authority record
before execution, so Codex, Claude/Pylon, AI SDK Core, AI SDK harness sandbox,
and Khala Sync mobile-control lanes can share one transcript/control contract
without giving any adapter permission by implication.

RK5 also adds a small shared surface presenter:
`projectAgentRuntimeSurfaceStatus`. Workroom and TUI views use it to render the
same public-safe run truth from kernel projections without reading raw adapter
transcripts.

`openagents.agent_definition.v1` is the harness-agnostic background-agent
definition contract. It stores the durable workflow object separately from the
runtime harness: name, goal, harness hint, lane, triggers, budget, escalation,
and an enforced toolset with `allow`, `deny`, and `ask` lists. Runtime runs can
link back to `agentDefinitionId`, so fulfillment loops can prove they were
definition-backed without baking Codex, Claude Code, or any other harness into
the durable record.

The pure helper `decideAgentDefinitionToolAuthority` is the shared
tool-authority boundary. It is deny-by-default, gives `deny` precedence over
`ask` and `allow`, and converts `ask` matches into an operator escalation record
instead of authorizing the tool invocation.

The `./webhooks` subpath owns provider-specific webhook normalization for
background-agent triggers. GitHub deliveries are converted into bounded
`openagents.agent_definition_webhook_event.v1` records before trigger
conditions run, so Worker ingress never passes raw provider payloads into model
or run context.
