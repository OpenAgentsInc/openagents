---
status: "accepted"
date: 2026-06-29
decision-makers: OpenAgents maintainers
consulted: AGENTS.md, docs/adr/0011-ship-khala-mobile-as-native-swiftui-with-local-apple-tooling.md, docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md, docs/desktop/2026-06-28-khala-desktop-spec.md, clients/khala-macos/src/launch-plan.ts, clients/khala-code-desktop/src/shared/on-device-decider.ts
informed: OpenAgents contributors, Khala Code desktop operators, Khala macOS/iOS operators, and Pylon operators
---

# Adopt an optional Khala Code on-device decider

## Context and Problem Statement

Khala Code needs a small local decision layer for low-risk routing choices such
as picking between native tool actions, classifying simple coding intents, or
deciding when a request should stay in the normal Khala/Codex path. Separate
backend work is adding Apple Foundation Models and self-hosted GPT-OSS lanes.
This ADR records the decision boundary for using those lanes as an optional
on-device decider without making Khala Code depend on either backend.

This decision is only about the decider contract and platform selection. It
does not implement Apple FM serving, GPT-OSS serving, model downloads,
evaluation claims, or coding-agent parity.

## Decision Drivers

* Keep Khala Code usable when no local small-model backend is installed.
* Preserve Apple FM as the local Apple-platform backend without claiming Codex
  parity.
* Preserve self-hosted GPT-OSS as the non-Mac local backend without making it a
  public model alias or spend path.
* Make platform selection testable and explicit.
* Keep backend implementations pluggable behind a narrow interface.
* Fail soft to the normal Khala Code path when the decider is disabled,
  unsupported, unavailable, or unhealthy.

## Considered Options

* Optional on-device decider with platform-selected pluggable backends.
* Always require a local decider before Khala Code can run.
* Use one backend family on every platform.
* Let each backend invent its own platform detection and request interface.

## Decision Outcome

Chosen option: "Optional on-device decider with platform-selected pluggable
backends", because it lets Khala Code use local small-model help where it is
available while keeping the product path independent from Apple FM and GPT-OSS
availability.

The accepted contract is:

* The decider is off by default. Operators must explicitly opt in before any
  local small-model backend is selected.
* Apple platforms select the Apple FM decider backend when Apple FM is
  available. Bun/Node `darwin` normalizes to `macos`; native iOS clients pass
  `ios` explicitly.
* Non-Mac desktop platforms select a self-hosted GPT-OSS decider backend when
  the local GPT-OSS service is available.
* Unsupported platforms, disabled flags, missing helpers, failed health checks,
  and backend errors are soft unavailability states. They must fall back to the
  normal Khala Code route.
* Backend adapters implement the pluggable
  `khala-code-on-device-decider-v1` interface. The selector returns a backend
  identity and kind; it does not construct or call backend services.

### Consequences

* Good, because Khala Code can opportunistically use local compute without a
  hard dependency.
* Good, because Apple FM and GPT-OSS backend PRs can implement adapters behind
  the same interface.
* Good, because platform behavior is covered by focused tests before any model
  serving code is wired in.
* Bad, because an opt-in decider adds another readiness state operators must
  surface and diagnose.
* Bad, because decider quality remains bounded by the backend and eval gates;
  this ADR does not prove model quality.

### Confirmation

Compliance is confirmed by code review and focused tests for
`clients/khala-code-desktop/src/shared/on-device-decider.ts`. Future backend PRs
must preserve the same default-off, platform-selection, and fail-soft behavior
when adding Apple FM or GPT-OSS adapters.

## Pros and Cons of the Options

### Optional on-device decider with platform-selected pluggable backends

* Good, because it matches the repository posture for local Apple FM readiness
  and native Khala tooling.
* Good, because it lets each platform use the local backend that naturally fits
  that environment.
* Good, because the selector is pure and testable without model spend.
* Bad, because it requires the UI/control plane to represent disabled,
  unsupported, unavailable, and ready as separate states.

### Always require a local decider before Khala Code can run

* Good, because every run would have one uniform local preflight path.
* Bad, because it would break Khala Code on machines without Apple FM or a
  self-hosted GPT-OSS service.
* Bad, because it would turn an optimization into a product dependency.

### Use one backend family on every platform

* Good, because the routing table would be smaller.
* Bad, because Apple FM is the native Apple-platform path and self-hosted
  GPT-OSS is the portable non-Mac path.
* Bad, because forcing either backend everywhere would increase setup burden
  and make unsupported states less truthful.

### Let each backend invent its own platform detection and request interface

* Good, because backend implementers could move quickly in isolation.
* Bad, because Khala Code would have duplicated detection, inconsistent
  fallback behavior, and more ways to accidentally create a hard dependency.

## More Information

* `clients/khala-code-desktop/src/shared/on-device-decider.ts`
* `clients/khala-code-desktop/tests/on-device-decider.test.ts`
* `clients/khala-macos/src/launch-plan.ts`
* `docs/desktop/2026-06-28-khala-desktop-spec.md`
* `docs/adr/0011-ship-khala-mobile-as-native-swiftui-with-local-apple-tooling.md`
* `docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md`
