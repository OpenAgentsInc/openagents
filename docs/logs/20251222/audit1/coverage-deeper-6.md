# Deeper Audit Coverage (Pass 6)

Additional systems reviewed in this pass:
- Recorder conversion/validation/CLI: `crates/recorder/src/convert.rs`, `crates/recorder/src/lib.rs`, `crates/recorder/src/main.rs`
- Nostr client relay + queue: `crates/nostr/client/src/relay.rs`, `crates/nostr/client/src/queue.rs`
- Nostr client pool/subscriptions/cache/outbox: `crates/nostr/client/src/pool.rs`, `crates/nostr/client/src/subscription.rs`, `crates/nostr/client/src/cache.rs`, `crates/nostr/client/src/outbox.rs`
