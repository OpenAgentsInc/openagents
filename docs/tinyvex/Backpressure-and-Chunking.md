# Backpressure and Chunking

Goals

- Keep slow clients from stalling the server.
- Avoid sending redundant intermediate states when updates arrive faster than the client consumes them.

Per-Connection Outbound Queue

- Bounded queue (e.g., 100 messages). When full:
  - Prefer coalescing subscription updates into a single latest state per `subId` (single‑flight).
  - Drop or compact older `tinyvex/data` messages for the same `subId` in favor of the newest.
  - Always preserve control frames (`pong`, `mutate_result`, `error`).
  - For ACP `session/update` streams, coalescing MUST NOT violate ordering within a session. If coalescing is applied, only drop superseded intermediate chunks of the same kind (e.g., multiple agent_thought_chunk updates), never cross‑reorder or merge different discriminators improperly.

Chunking Large Messages

- Apply when serialized payload exceeds `maxChunkBytes` (advertised in handshake).
- Split into `tinyvex/transitionChunk` notifications with fields `{ subId, seq, chunk, part, total, transitionId }`.
- Clients reassemble by `transitionId` and validate `part/total` before publishing the combined value.

Observability

- Track queue depth, dropped/coalesced counts, chunked size and counts.
- Emit `tinyvex/error` with code `TVX005` (BackpressureDrop) when coalescing occurs, at most once per interval per `subId`.
 - Consider ACP‑aware metrics (e.g., per `session_id`) to observe coalescing impact on session streams.
