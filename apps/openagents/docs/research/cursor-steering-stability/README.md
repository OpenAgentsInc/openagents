# Cursor steering and recent-message retention audit

This follow-up to PR #454 uses the production engine, production Cursor adapter,
SDK 1.0.31, and authenticated `muse-spark-1.3` sessions. Test conversations and
workspaces are disposable. No real user's conversation is resumed or modified.

## Two independently identified message-loss paths

1. **Remote command batches:** `evaluate_command` treated newer pending Steer
   commands as replacements for earlier ones. A synchronized batch could discard
   distinct user messages before the adapter saw them. Steer commands now all
   execute; command-ID deduplication and interrupt supersession remain intact.
   The fix applies to every harness using this shared command path.
2. **Interrupted startup:** Cursor can stop before committing the current user
   message to its conversation checkpoint. Zeron's transcript already contains
   that message, but a plain SDK resume cannot recover it. A live engine test
   interrupted immediately after `SessionStarted`, asked for a random token
   from that message, and received: “No INTERRUPTED token was in your previous
   message.” The same test passes with the fix.

The shim now atomically writes a user-message receipt in its exclusively owned
store before publishing readiness. Before a subsequent explicit user request,
it reads Cursor's saved conversation and carries forward text absent from the
checkpoint, labeled as interrupted history. It does not resend an interrupted
request as a separate turn or automatically retry tools. Repeated interruptions
retain a flat, ordered list; text already checkpointed is not duplicated.
If startup stops before an SDK-side receipt can be written, the engine bridges
the preceding user-message tail that has no assistant content yet. With no SDK
session ID, it supplies all preceding user messages from its durable transcript.
It excludes the current and later queued messages and does not alter the
user-visible transcript. This conservative startup bridge can repeat historical
context already saved by Cursor; it never creates an additional executable turn.
The receipt uses mode 0600 beside the existing conversation store. Invalid
receipts or unexpected history schemas produce an explicit error instead of
silently discarding context. Legacy SDK-default stores without a Zeron-owned
store have no receipt; the new protection cannot reconstruct previously lost
messages from those stores.

Live validation caught a mistake in the first receipt implementation: SDK
conversation objects use protobuf `turn.case/value` fields rather than their
printed JSON shape. That intermediate implementation failed ordinary follow-up
checks and was corrected before publication. The fixture now exercises both
representations. Final results are distinguished from this failed development
iteration in [results.json](results.json).

Heavy parallel recovery testing also reached Cursor's `get_models` limit of
30 requests/minute during SDK startup. The shim now retries only that specific
pre-send discovery failure with bounded exponential backoff (at most seven
attempts, 63 seconds of delay). Authentication and arbitrary run errors are not
retried. A regression verifies one transient startup limit succeeds without
resending a prompt, while authentication fails immediately. The initial
rate-limited live recovery attempt is recorded as failed, not passed.

The broader engine suite also exposed a cancellation-order race: waking a
blocked harness before publishing the engine's cancellation signal could let
EOF be classified as an error. Cancellation intent is now published first.
This ordering fix applies across harnesses and is covered by the existing
awaiting-input cancellation regression plus repeated runs of that test.
The suite's RPC reset expectation was also updated for the `replayBaseline`
field already emitted by main; this changes a stale test assertion, not the RPC.

## What the checks establish

- `cursor_live.rs` exercises the **real engine command and pending-message
  paths**, including remote batches, startup interruption, interruption during a
  tool, and repeated startup interruptions in one conversation.
- The `history` adapter probe introduces a different random token in each
  message. Each response must recall both the immediately preceding token and
  the new one. A fresh-process resume must list every token in order. No later
  prompt repeats an earlier token, and tokens are never written to workspace
  files. This is stronger than the earlier tests that recalled only one seed.
- The recovery probe now checks the **latest interrupted message**, as well as
  an older checkpoint, after kill, cancel, and dropped-stream faults. Exactly
  one tool append per injected fault is allowed; recovery must not repeat it.
- Native CLI comparison uses the updated installed CLI, eight unique messages,
  per-message recall, and final all-message recall across native CLI resumes.
- Credential-free regressions cover 200 queued steers, 100 cancelled queued
  steers, 100 shim failure/recovery cycles, 20 consecutive missing checkpoints,
  corrupted receipts, duplicate command drains, clock skew, model discovery
  failures, and token refresh/stream-auth failures in the actual pinned SDK.

## Reproduce

These live checks require a logged-in Cursor account with Muse Spark access and
consume provider quota. Always set a fresh isolated state directory.

```sh
cargo test -p zeron-doc
cargo test -p zeron-engine --test message_queue
cargo test -p zeron-harness

ZERON_CURSOR_STATE_DIR=$(mktemp -d) ZERON_CURSOR_EARLY_ROUNDS=10 \
  cargo test -p zeron-engine --test cursor_live -- --ignored --nocapture --test-threads=1

ZERON_CURSOR_STATE_DIR=$(mktemp -d) ZERON_CURSOR_TEST_MODEL=muse-spark-1.3 \
  cargo run -p zeron-harness --example cursor_stability_probe -- history 40
ZERON_CURSOR_STATE_DIR=$(mktemp -d) ZERON_CURSOR_TEST_MODEL=muse-spark-1.3 \
  cargo run -p zeron-harness --example cursor_stability_probe -- sessions 12
ZERON_CURSOR_STATE_DIR=$(mktemp -d) ZERON_CURSOR_TEST_MODEL=muse-spark-1.3 \
  cargo run -p zeron-harness --example cursor_stability_probe -- cancel-burst 40

cursor-agent update
python3 scripts/cursor-cli-history-probe.py
```

For accelerated auth expiry, use the clock-preload command in the
[auth investigation](../cursor-auth-incident/README.md), with
`ZERON_CURSOR_TEST_MODEL=muse-spark-1.3` and `parked 8`.

## Limits

This is a finite regression matrix, not proof of every possible model, tool,
network failure, operating system, or provider behavior. Live runs here use
Linux and Muse Spark. Auth clock advancement tests refresh behavior, not a
wall-clock hour-long soak. Authenticated tests remain opt-in; CI runs the
credential-free regressions, including the shared engine queue tests.

The adapter still queues ordinary follow-ups at turn boundaries. The SDK's
native in-turn steering API was checked separately with Muse and persisted both
messages, but this change does not switch the adapter to that API. “Send now”
continues to interrupt/resume; the receipt repairs its missing-message case.
Missing text is supplied as conversation context, not inserted by rewriting
Cursor's private checkpoint format. Context retention does not guarantee that
an LLM will always answer correctly or never choose an unwanted tool call.
