/**
 * Tunables for the Loro session-doc pipeline (design: loro-sessions §1.2, §3,
 * §5, §6, §10.3). Every constant here is a starting point to be re-measured
 * against a real converted heavy session; keep them in one place so the
 * measurement pass has a single file to touch.
 */

/** Segment split cap: a single `messages` entry never exceeds this many bytes
 * of JSON-encoded parts. Oversized turns split into continuation entries —
 * not a size reducer, a spike smoother (bounds the largest single op/poke a
 * client must apply). */
export const MSG_INLINE_MAX = 256 * 1024;

/** Shallow-compaction retention: the DO re-exports a shallow snapshot at the
 * `now − RETAIN_DAYS` frontier when the update log grows past
 * {@link COMPACT_LOG_BYTES}. Trimmed op history is discarded permanently;
 * state is fully preserved. A peer offline longer than this re-syncs fresh and
 * re-submits its unacked entries at the app layer (idempotent by entry id).
 *
 * 30 days proved fatal in practice (2026-08-04): the ws3-era rooms were ~12
 * days old, so no frontier checkpoint was ever past retention and HISTORY
 * TRIM had never run ANYWHERE — every doc carried full op history from birth.
 * A high-churn workspace room (6 devices flipping session rows all day) plus
 * a dozen chat docs co-materialized in one isolate's loro-wasm heap (which
 * only ever grows) hit the DO memory limit, poisoning every byte-exporting
 * wasm call and silently wedging joins fleet-wide. Presence/status rows do
 * not need weeks of op history; 3 days keeps materialized docs small while a
 * briefly-offline device still diff-syncs. */
export const RETAIN_DAYS = 3;

/** Update-log size that triggers a compaction pass in the session DO.
 *
 * Cold-start cost (`ensureDoc` after a hibernation or a CPU-limit reset)
 * replays the snapshot plus every logged update, so this cap is really a
 * cold-start budget. 8 MiB was too generous for a high-churn room like the
 * per-user workspace doc (3 devices writing device/session rows continuously):
 * the log grew large enough that a cold replay exceeded the DO's per-invocation
 * CPU limit, the runtime reset the DO, every client reconnected into another
 * cold start, and the memory-only presence store was wiped each time — so
 * devices flapped permanently offline. 2 MiB keeps the replay cheap. */
export const COMPACT_LOG_BYTES = 2 * 1024 * 1024;

/** Update-log ROW count that also triggers a compaction pass. Cold-start cost
 * scales with the NUMBER of `doc.import()` calls (one WASM boundary crossing
 * each), not just their total bytes — a workspace room accumulates thousands
 * of tiny row/presence-adjacent updates well under {@link COMPACT_LOG_BYTES}.
 * Fold on row count too so many-small-updates can't blow the cold-start
 * budget the byte cap alone would miss. 1500 proved too generous: a per-user
 * workspace room wedged its cold replay at ~1400 rows (Jul 2026), under the
 * trigger — folding is cheap while the doc is hot (one snapshot export), so
 * fold early and keep cold replays trivially affordable. */
export const COMPACT_LOG_ROWS = 400;

/** Soft ceiling: past this much doc state the UI nudges toward a fresh
 * session. No enforcement machinery — a product stance, not a limit. */
export const SOFT_CEILING_BYTES = 25 * 1024 * 1024;

/** Host stream batching: token/part deltas are committed to the doc on this
 * cadence while a run is streaming (single-peer appends RLE-merge in Loro). */
export const STREAM_COMMIT_MS = 120;

/** DO durability batching during active streams: buffered updates are flushed
 * to SQLite on this cadence. A crash losing the buffer is healed by normal
 * CRDT resync from the host on reconnect. */
export const DO_FLUSH_MS = 5_000;

/** Mobile doc LRU budget — bytes of resident doc *state*, not doc count.
 * Eviction drops the Mirror + doc; state stays on disk. */
export const DOC_LRU_BYTE_BUDGET = 80 * 1024 * 1024;

/** How many trailing messages the DO materializes into its `tail` slot for the
 * L2 instant-open path (mirrors today's RECENT_MESSAGES_LIMIT). */
export const TAIL_MESSAGE_COUNT = 64;

/** Default TTL for durable commands that should not run on a host that wakes
 * up much later (staleness guard §7). Composers may override per command. */
export const COMMAND_DEFAULT_TTL_MS = 24 * 60 * 60 * 1000;

/** Current session-doc schema version, written to `meta.schemaVersion`. */
export const SESSION_SCHEMA_VERSION = 1;

/** Terminal output batching over the device DO (§8). */
export const TERMINAL_OUTPUT_BATCH_MS = 12;
