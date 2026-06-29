---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: INVARIANTS.md, clients/khala-desktop/src/bun/apple-fm-sidecar.ts, clients/khala-desktop/src/shared/apple-fm-readiness.ts, packages/probe/packages/runtime/src/backends/apple-fm/contract.ts, docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md
informed: Khala Code desktop operators, OpenAgents contributors, Pylon operators
---

# Adopt an optional dual-backend on-device decider

## Context and Problem Statement

Khala Code desktop needs a small, fast local decider for tool and model
selection hints. This decider must not become the main coding model, must not
spend hosted tokens, and must fail soft when local model support is missing.

Apple platforms already have a local Apple Foundation Models bridge contract in
the OpenAgents workspace. Non-Apple hosts need the same decider abstraction so a
GPT-OSS local backend can slot in without changing the UI or caller contract.

## Decision Drivers

* Keep the decider optional and off by default.
* Preserve no-spend behavior by allowing only local loopback backends.
* Use Apple Foundation Models on admitted Apple platforms when explicitly
  enabled.
* Keep GPT-OSS as the non-Mac backend slot without hard-coding a provider.
* Model the request, status, and decision boundary with Effect Schema.
* Avoid any Apple FM coding-parity claim.
* Fail soft when the local bridge, hardware, model, or GPT-OSS endpoint is not
  available.

## Considered Options

* Dual-backend local decider interface: Apple FM on Apple platforms, GPT-OSS
  local HTTP on non-Apple platforms.
* Apple FM-only decider.
* Hosted model decider.
* Keyword or rules-based local router.

## Decision Outcome

Chosen option: "Dual-backend local decider interface", because it keeps the
desktop contract backend-neutral while still using the best local platform path
on Apple hardware. The decider is activated only by
`OPENAGENTS_DESKTOP_ON_DEVICE_DECIDER`; without that opt-in it returns a
disabled status and never initializes a backend.

Apple FM is reached through the existing local Foundation Models bridge shape:
health on `/health` and compact JSON decisions through `/v1/chat/completions`.
GPT-OSS uses the same local OpenAI-compatible chat-completions shape behind a
loopback URL. Both paths return advisory decisions only.

### Consequences

* Good, because the UI and RPC contract do not need to know which local backend
  is used.
* Good, because Apple FM and GPT-OSS are both no-spend, local-only slots.
* Good, because unsupported or unconfigured hosts show typed blockers instead
  of crashing or falling through to hosted inference.
* Bad, because GPT-OSS readiness is only a local endpoint contract until a
  specific runtime is selected and packaged.
* Bad, because Apple FM packaged-helper availability still depends on the
  existing Foundation Models bridge being present or explicitly configured.

### Confirmation

Compliance is confirmed by:

* `clients/openagents-desktop/src/shared/on-device-decider.ts`
* `clients/openagents-desktop/src/bun/on-device-decider.ts`
* `clients/openagents-desktop/tests/on-device-decider.test.ts`
* `clients/openagents-desktop/tests/on-device-decider-host.test.ts`
* `clients/openagents-desktop/tests/app-shell.test.ts`
* `bun run --cwd clients/openagents-desktop verify`

## Pros and Cons of the Options

### Dual-backend local decider interface

* Good, because it keeps backend selection platform-aware and pluggable.
* Good, because it can use Apple FM where Foundation Models exists while leaving
  non-Mac hosts a GPT-OSS path.
* Good, because the local-only URL checks preserve the no-spend boundary.
* Bad, because it adds a backend abstraction before the GPT-OSS runtime is
  fully chosen.

### Apple FM-only decider

* Good, because it reuses current local Apple FM bridge work.
* Bad, because it leaves non-Mac desktop hosts without a compatible on-device
  slot.
* Bad, because it would couple the decider contract to one platform.

### Hosted model decider

* Good, because it could work on every host immediately.
* Bad, because it violates the no-spend requirement.
* Bad, because it would send decision context off device.

### Keyword or rules-based local router

* Good, because it is simple and deterministic.
* Bad, because repository invariants reject ad hoc keyword routing for tool
  selection.
* Bad, because it would not be a small-model decider and would be difficult to
  evolve into a learned local decision layer.

## More Information

* `INVARIANTS.md`
* `clients/khala-desktop/src/bun/apple-fm-sidecar.ts`
* `clients/khala-desktop/src/shared/apple-fm-readiness.ts`
* `packages/probe/packages/runtime/src/backends/apple-fm/contract.ts`
* `docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md`
