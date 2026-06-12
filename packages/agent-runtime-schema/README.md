# Agent Runtime Schema

`@openagentsinc/agent-runtime-schema` is the RK1 schema-only Agent Runtime Kernel
contract. It defines the durable OpenAgents runtime run and event log shape used
by native, external, hosted, and fixture loops.

The package intentionally contains no executor, provider SDK, or Vercel AI SDK
dependency. Adapter-specific loops project into this contract at the boundary;
worker and UI surfaces consume event logs and projections derived from this
contract.

RK5 also adds a small shared surface presenter:
`projectAgentRuntimeSurfaceStatus`. Workroom and TUI views use it to render the
same public-safe run truth from kernel projections without reading raw adapter
transcripts.
