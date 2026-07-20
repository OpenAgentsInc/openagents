# @openagentsinc/agent-turn-store

Root package for the driver-neutral turn journal state and migrations. Packet
AFS-01 adds the store port and an in-memory test adapter. Platform drivers
(Node, Expo, browser) live only in platform subpaths or app composition roots,
never in this root export.

- It must not own platform drivers in its root export.
- The turn state machine must not import a concrete store.
- Root export only. No app, Electron, React, React Native, Node file or process
  API, provider SDK, SQL driver, or cloud client import.
