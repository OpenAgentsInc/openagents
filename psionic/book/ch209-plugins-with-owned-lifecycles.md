# 209. Plugins with owned lifecycles

> **Status: unimplemented — design draft.** Nothing here runs yet.

Extensions to the harness need clear ownership. This chapter composes the
harness from ordered profiles and plugins. Every registration belongs to the
plugin that made it and is revoked when that plugin unloads. Third-party
plugins run in bounded Wasm and are distributed as signed Nostr events.

## What this chapter covers

- Profiles: ordered lists of plugins that make up a harness configuration.
- Capabilities with three roles: a definition, a provider and consumers.
- Owned registrations. Unloading a plugin removes every tool, hook and
  capability it registered.
- Visible states: installed, enabled, admitted, active, failed and draining.
- Third-party plugins in bounded Wasm with admitted capabilities only.
  DeepSeek Harness runs host plugins with the host process's authority;
  Psionic does not.
- Distribution as signed Nostr events, verified before install.

## Dependencies

- [Chapter 203](ch203-tools-as-typed-capabilities.md) and [Chapter 204](ch204-a-sandbox-and-approvals.md): tools and grants that plugins register.
- [Chapter 59](ch059-run-in-the-browser.md): Coax compiled to Wasm.
- `coax/lib/nostr`: signed events (see the Part IV prerequisites in the
  [plan](PLAN.md)).

## Done means

- After a plugin unloads, no registration it made remains.
- A Wasm plugin that calls a capability it was not admitted is refused.
- A plugin event with a bad signature does not install.
- The state of each plugin is visible and changes only through the listed
  transitions.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
