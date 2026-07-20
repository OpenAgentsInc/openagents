# @openagentsinc/apple-fm-runtime

Neutral root package for the Apple Foundation Models provider. It owns the Apple
FM wire contract, the local inference provider adapter, the helper supervisor
contract, the capability probe, fixtures, and the Swift `foundation-bridge`
source. Packet AFS-02 moves the reusable behavior here from the nested Pylon
runtime and makes Desktop and Pylon consumers.

Exports:

- `.` — portable, bundle-safe. Wire schemas, provider contract, and the frozen
  wire-version source. No Node import.
- `./node` — helper discovery, signature checks, spawn, readiness, and shutdown.
  Node host authority lives only here.
- `./testing` — fixtures and a fake transport.

The subpath split keeps a browser or mobile bundle from importing the Node host
by accident. This package must not import Pylon, Desktop, Blueprint, assignment,
fleet, token, wallet, or cloud code.
