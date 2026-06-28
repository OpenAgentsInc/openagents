# Khala macOS

Bounded macOS client contracts for turning an Apple-Silicon Mac into a
self-contained Khala node.

This first slice is intentionally pure and testable. It mirrors the existing
Autopilot Desktop/Pylon Apple FM bridge shape:

- Apple FM bridge: `foundation-bridge` on loopback `http://127.0.0.1:11435`.
- Packaged helper path: `Resources/app/apple-fm-bridge/foundation-bridge`.
- Packaged Pylon entry path: `Resources/app/pylon-node/index.js`.
- One-launch UX: adopt an already-running Pylon when present, otherwise launch
  the embedded Pylon node with Apple FM bridge supervision enabled.
- Demand attribution: local Apple FM turns are marked as owner capacity and keep
  estimated usage truth instead of pretending token counts are exact.

The package does not spawn processes yet. Native app wiring should execute the
launch plan produced by `buildKhalaMacosLaunchPlan`.
