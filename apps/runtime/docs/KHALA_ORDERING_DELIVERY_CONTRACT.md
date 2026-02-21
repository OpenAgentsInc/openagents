# Khala Ordering and Delivery Contract (Rust Runtime)

Status: Active  
Owner: Runtime/Khala  
Issue: OA-RUST-089 (`#1924`)

## Purpose

Define the authoritative ordering and delivery semantics for Khala streams in Rust runtime, including multi-node transport caveats and client idempotency rules.

## Ordering Oracle

1. Runtime sequence (`seq`) is allocated by runtime authority writes and is the single logical ordering oracle per topic.
2. Logical ordering is defined by `(topic, seq)`, not by transport arrival time.
3. Multi-node transport may deliver frames out-of-order; consumers must reorder/ignore by `seq`.

## Delivery Semantics

1. Delivery is at-least-once.
2. Duplicate frames are allowed.
3. Exactly-once delivery is not provided.
4. If replay floor is missed, server returns deterministic `stale_cursor` recovery metadata and clients must rebootstrap.

## Client Apply Rules (Mandatory)

1. Persist `last_applied_seq` per topic.
2. Discard any frame where `seq <= last_applied_seq`.
3. Apply only frames with `seq > last_applied_seq`.
4. Treat gaps as replay/bootstrap concern, not as implicit transport failure.

## Runtime Enforcement in Rust

1. Fanout polling enforces logical ordering by sorting outbound frames by `seq` before response emission.
2. Topic windows/stale-cursor checks use sequence bounds from queue content, not insertion order.
3. Projection apply is idempotent by checkpoint: `incoming_seq <= checkpoint.last_seq` is a no-op.

## Conformance Tests

The following tests verify the contract:

1. `fanout::tests::memory_fanout_returns_logical_seq_order_when_transport_order_is_mixed`
2. `projectors::tests::duplicate_and_out_of_order_delivery_is_idempotent_by_topic_seq`
3. Existing stale-cursor + slow-consumer tests in `server::tests::*` remain part of delivery conformance.

## Non-Goals

1. Exactly-once network delivery.
2. Transport-level global total ordering across topics.
3. Treating websocket arrival order as authority.
