# Roadmap — Tinyvex MVP → V1

MVP (Week 1–2)

- Boot SwiftNIO WS server inside macOS app; single connection tested via Simulator.
- Implement `tinyvex/connect`, `tinyvex/subscribe`, `tinyvex/unsubscribe`, `tinyvex/mutation`.
- GRDB setup with WAL, migrations, and a single demo table mapped to documents.
- SubscriptionManager with one query key and broadcast to multiple clients.
- Client library: subscribe + mutation stubs, reconnect + resubscribe.
- Tests: two simulated clients receiving the same stream; mutation triggers update.

Beta (Week 3–4)

- Add chunking and single‑flight backpressure in ConnectionManager.
- Add ValueObservation query normalization and deduplication across clients.
- Implement journaling/resume (`lastSeq` / `journal` fields).
- Bonjour discovery integration on iOS.
- Error taxonomy + robust decoding; no `try!`.

V1 (Week 5+)

- Auth token plumbing for future remote/cloud, but optional locally.
- Extended query library and index helpers.
- Retention and compaction tasks; snapshot validation.
- Security hardening: TLS (wss), pairing tokens, audit logging.

Nice‑to‑Have Later

- Background indexing tasks; rate‑limited invalidation.
- Metrics dashboard; timing and queue depth histograms.
- Cloud/headless variant using the same protocol.

