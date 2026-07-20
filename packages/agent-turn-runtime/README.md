# @openagentsinc/agent-turn-runtime

Root package for the shared, UI-neutral Effect turn kernel. It owns the turn
policy and turn state machines. Packet AFS-01 adds the scoped `TurnService`, the
`TurnPolicy`, `ProviderRegistry`, `TurnJournal`, `ThreadRepository`,
`ArtifactResolver`, `ContextSource`, and `ActionBroker` ports, and the
deterministic state transitions. AFS-00 reserves the package graph, the port
type surface, and the import boundary.

- It must not own providers, storage, UI, or platform APIs.
- Apple FM implements the provider interface here. This package must not import
  `@openagentsinc/apple-fm-runtime`.
- Root export only. No app, Electron, React, React Native, Node file or process
  API, provider SDK, SQL driver, or cloud client import.
