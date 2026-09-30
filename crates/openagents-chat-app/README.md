# Shared chat application

`openagents-chat-app` owns the OpenAgents chat controller and its portable
presentation: conversation lists, retained Coder transcripts, suggestions,
router offers, Coder target selection, command outbox, queue and question state,
and Gym cards. It carries the existing Rust implementation from the phone into
one shared library. The phone preserves its imports through re-exports.
The desktop consumes the shared client session and transcript projection,
leaving native editing, focus, clipboard callbacks, and painting in its adapter.

The crate calls the existing Coder client and authority checks. A conversation
remains hosted OpenAgents chat until an explicit offered action dispatches Coder
to a connected computer. Engine execution stays on the Coder host. The adapters
provide credentials, connections, storage, native controls, and navigation.
Wallet and Playtest destinations are typed navigation values; the phone adapter
owns their implementations. Portable playtest event codes are handed back to
that adapter, which owns logging and report transmission.

## Dependency boundary

The resident host uses `openagents-chat` for the hosted transport, lifecycle, and
encrypted records. This application crate sits above that core and the Coder
client. Keeping the client-facing controller here avoids a dependency cycle
through `coder-host`.

Neither crate depends on `openagents-mobile`, Breez, or SQLite. The phone remains
its own Cargo workspace for Breez's SQLite dependency; it consumes both shared
crates by path. Shared chat records, cache keys, opaque composer tokens, and
phone packet serialization remain compatible. The platform wake adapter also
keeps its existing change-counter behavior through the re-export.

The `test-support` feature exposes read-only fixture helpers to tests in the
phone workspace. Normal builds do not enable those helpers.

## Verification

Run the portable application tests in the main workspace:

```sh
cargo test --locked -p openagents-chat-app --target-dir target
cargo clippy --locked -p openagents-chat-app --all-targets --target-dir target -- -D warnings
```

Run phone consumer checks in its separate workspace:

```sh
cargo test --locked --manifest-path crates/openagents-mobile/Cargo.toml --lib --target-dir target/phone-core
```

The retained Gym report used by the shared tests stays in the phone's fixture
archive. Reading that test artifact does not link the mobile application.
