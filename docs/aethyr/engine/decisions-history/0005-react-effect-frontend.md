> Deprecated (owner, 2026-10-24): history only. Architecture decisions live in [`docs/adr/`](../../../adr/README.md).

# 0005: React and Effect frontend with an independent frame loop

Status: selected, 2026-10-22.

## Context

The initial design selected TypeScript, DOM controls, and Effect but left the
component library and reactive UI layer unspecified. The editor needs accessible
contextual tools, typed asynchronous services, and a clear boundary between UI
state and a continuously running world.

## Decision

Use React DOM and TypeScript components, Effect Atom with `@effect/atom-react`
for reactive UI state, and Effect 4 for application services. Own the components,
state adapter, command interfaces, and design system. Pin compatible library
versions together at implementation; Atom API changes remain inside the adapter.

The runtime worker owns simulation and direct WebGPU frame scheduling independently
of React renders, hooks, and atom notifications. Atoms expose selected views and
bounded results; they do not mirror every entity or become world authority.
Effect services own explicit preview lifetimes. Component subscriptions do not
create an engine, advance a frame, or determine when the engine stops. See the
normative [frontend contract](../frontend.md).
UI-only hot refresh retains the isolated preview browsing context; genuine context
reload follows checkpoint recovery. Bounded UI streams expose gaps and recover
command outcomes through durable status reads instead of blocking the frame loop.

## Alternatives and consequences

- **Custom DOM renderer:** replaceable later, but adds a separate UI framework to
  maintain without evidence that it improves the engine's independent hot path.
- **Another component framework:** viable behind the same command/service
  interfaces; adopting one requires measured editor or accessibility benefits.
- **Component-driven world state:** couples entity updates and frame timing to
  UI work. The selected architecture keeps those in the owned runtime instead.
- **A second application-wide state store:** adds competing caches and lifecycle
  ownership. Use Effect Atom for shared UI state and Effect 4 for service work;
  component-local presentation state remains permitted.

The design adds explicit registry disposal, subscription bounds, lifecycle
idempotency, and dependency compatibility tests. UI work can still contend for
browser resources; independence is a scheduling contract, not a benchmark result.

## Revisit

Revisit the UI adapter or component framework if pinned profiles show material
accessibility, interaction latency, memory, or maintenance problems. Preserve
typed commands, trust boundaries, project ownership, and independent engine
scheduling through any replacement. [Verification](../verification.md) owns
integration evidence.
