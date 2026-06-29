# Khala macOS

Bounded macOS client contracts for turning an Apple-Silicon Mac into a
self-contained Khala node.

This first slice is intentionally pure and testable. It mirrors the existing
Autopilot Desktop/Pylon Apple FM bridge shape:

- Apple FM bridge: `foundation-bridge` on loopback `http://127.0.0.1:11435`.
- Packaged helper path: `Resources/app/apple-fm-bridge/foundation-bridge`.
- Packaged Pylon entry path: `Resources/app/pylon-node/index.js`.
- One-launch UX: adopt an already-running Pylon control endpoint when present,
  otherwise launch the embedded Pylon node with Apple FM bridge supervision
  enabled and an app-managed `PYLON_HOME` under Application Support.
- Demand attribution: local Apple FM turns are marked as owner capacity and keep
  estimated usage truth instead of pretending token counts are exact.

The native app mirrors this contract in `PylonSupervisor`: it probes the
loopback control endpoint, authenticates with the local control token, surfaces
accounts/capacity/assignments in the inspector, and only owns stop/restart for a
child it launched itself.
