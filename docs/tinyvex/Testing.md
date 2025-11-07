# Testing Plan — Tinyvex

Unit Tests

- SubscriptionManager
  - Normalization and dedupe: same (name, params) → single observation.
  - Seq increment behavior and journal window trimming.
  - Coalescing policy when outbound queue is full.
- DbLayer
  - Insert/update/delete transactions; WAL behavior; busy retries.
  - Snapshot reads at specific `ts`; tombstone handling.
- Protocol encoding/decoding
  - JSON‑RPC envelopes, error mapping, chunking reassembly.

Integration Tests

- Two simulated clients subscribing to the same query; mutation triggers updates to both.
- Client reconnects and resumes with `lastSeq` within the journal horizon.
- Large payload chunking: send chunked data; client reassembles and publishes.
- Backpressure: throttle client read; ensure coalescing preserves latest state and control frames.

Load/Soak Tests (later)

- 50–200 concurrent lightweight clients; mutation rate 1–10/sec; observe queue depths and dropped/coalesced metrics.

Tooling

- SwiftPM test targets for unit/integration.
- Optional local harness in OpenAgents app to spin up server and attach simulated clients.

