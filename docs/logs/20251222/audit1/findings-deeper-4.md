# Deeper Findings (Pass 4)

## Medium
- D4-M-1 GitAfter caches Nostr events with synchronous rusqlite calls inside async tasks (`tokio::spawn` + `Mutex<EventCache>`), which can block the async runtime under load. Evidence: `crates/gitafter/src/nostr/client.rs:41`, `crates/gitafter/src/nostr/client.rs:153`, `crates/gitafter/src/nostr/cache.rs:10`, `crates/gitafter/src/nostr/cache.rs:95`.
- D4-M-2 GitAfter search endpoints apply `LIMIT` before in-memory filtering, so searches can return empty even when matches exist beyond the first N rows. Evidence: `crates/gitafter/src/nostr/cache.rs:1062`, `crates/gitafter/src/nostr/cache.rs:1096`, `crates/gitafter/src/nostr/cache.rs:1122`.
- D4-M-3 Wallet Bitcoin CLI commands are fully stubbed with `anyhow::bail`, so balance/send/receive/history always fail despite being exposed in the CLI. Evidence: `crates/wallet/src/cli/bitcoin.rs:5`, `crates/wallet/src/cli/bitcoin.rs:29`.

## Low
- D4-L-1 GitAfter cache defines foreign keys but never enables SQLite foreign key enforcement, and cleanup deletes only from `events`, leaving orphan rows in metadata tables. Evidence: `crates/gitafter/src/nostr/cache.rs:52`, `crates/gitafter/src/nostr/cache.rs:546`.
- D4-L-2 PullRequestBuilder docs call `commit` and `clone_url` required, but `build` does not enforce them, allowing invalid NIP-34 PR events. Evidence: `crates/gitafter/src/nostr/events.rs:503`, `crates/gitafter/src/nostr/events.rs:581`.
- D4-L-3 FROSTR wallet CLI is demo-only: keygen stores only a marker in keychain and import/export/sign flows are not implemented, so shares aren't recoverable from storage. Evidence: `crates/wallet/src/cli/frostr.rs:34`, `crates/wallet/src/cli/frostr.rs:75`, `crates/wallet/src/cli/frostr.rs:96`.
