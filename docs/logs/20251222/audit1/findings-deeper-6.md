# Deeper Findings (Pass 6)

## High
- D6-H-1 Relay queue background task dequeues pending messages but never marks them sent/failed or publishes them; since `dequeue` is read-only, the loop can spin on the same item and the offline queue never drains. Evidence: `crates/nostr/client/src/relay.rs:607`, `crates/nostr/client/src/relay.rs:631`, `crates/nostr/client/src/relay.rs:656`, `crates/nostr/client/src/queue.rs:170`, `crates/nostr/client/src/queue.rs:208`.

## Medium
- D6-M-1 Recorder conversion drops user text when message content is block-structured: it only extracts ToolResult blocks, ignoring ContentBlock::Text for user messages, so user inputs become empty in the .rlog. Evidence: `crates/recorder/src/convert.rs:103`, `crates/recorder/src/convert.rs:266`.
- D6-M-2 Recorder conversion defaults to embedding raw Claude JSONL events as comments, which can leak secrets/PII when logs are shared (the default is enabled in both options and CLI flags). Evidence: `crates/recorder/src/convert.rs:26`, `crates/recorder/src/convert.rs:244`, `crates/recorder/src/main.rs:121`.
- D6-M-3 Relay pool can return Ok with fewer than `min_write_confirmations` when publish tasks error (join errors are logged but not surfaced), so callers may treat a failed publish as success. Evidence: `crates/nostr/client/src/pool.rs:351`, `crates/nostr/client/src/pool.rs:367`.

## Low
- D6-L-1 Recorder header validation says repo_sha should be hex, but only checks length, so non-hex strings like "unknown" pass validation. Evidence: `crates/recorder/src/lib.rs:834`.
- D6-L-2 Recorder parse/convert paths read entire files into memory (`read_to_string` + `lines().collect()`), which can spike memory for large sessions; streaming would be safer. Evidence: `crates/recorder/src/convert.rs:210`, `crates/recorder/src/lib.rs:411`.
- D6-L-3 Nostr subscriptions use `mpsc::unbounded_channel` for event delivery, which can grow unbounded if consumers are slow or stalled. Evidence: `crates/nostr/client/src/subscription.rs:50`, `crates/nostr/client/src/pool.rs:388`.
