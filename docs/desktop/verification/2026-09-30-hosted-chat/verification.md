# Hosted desktop chat

Issue #10007 uses the same `openagents-chat` relay, authenticated encrypted
requests, router metadata, lifecycle, and context as the phones. The host owns
the service; the desktop receives typed snapshots over its private control
socket. Pending sends belong to their conversation, so a delayed acknowledgement
cannot clear another conversation's draft or prevent its send. Working remains
visible below a partial reply until its terminal outcome arrives.

## Stop and retry

The conversation profile in NIP-CJ has no remote cancellation or durable
retransmission identity. Dropping observation does not stop remote work. The UI
therefore says **Stopped receiving this reply. The hosted worker may still
finish.** It retains the received partial with a persisted stopped marker.
Stopping before any words retains the same marker on the unanswered user turn.
A terminal result observed before Stop remains completed.

**Retry reply** removes the stopped partial from the request context and invokes
a new reply for the existing user message. It appends no duplicate user message.
This is a new backend invocation, rather than a claim that the remote worker
resumed or cancelled an old one. Exact local send retries retain their stable
send ID. Late results from a stopped observation cannot overwrite the new reply
or reach another conversation.

The working and retry presentation reimplements Zeron's designs in Rust Native.
No Zeron code or agent harness is copied.

## Live evidence

On September 30, 2026, the opt-in desktop fixture used a fresh random identity,
a temporary encrypted store, and the public OpenAgents hosted worker. It sent
only a synthetic question about Nostr relays. The real reply reached the desktop
controller and painted incremental native transcript frames. First words arrived
in 0.755 seconds; completion took 10.313 seconds. Two different partial states
were observed and painted. Switching to a new chat during the stream showed
only that chat's welcome; returning restored the first conversation's stream.
A contextual follow-up completed in 4.702 seconds with four saved turns total.

[Streaming frame](streaming.png) and [completed frame](completed.png) show the
shared desktop painter's output. These captures use the native adapter's
headless capture path, rather than a claim about compositor scanout. The actual
native window, GPU upload, and presentation path is measured in the
[long-content matrix](../2026-09-30-transcript/verification.md). A repeated
2x default-window run with Verse and the persistent working row has scrolling
p99 4.142 ms and streaming p99 7.422 ms; its [raw samples](native.json) are retained.

The test host fixture creates its identity outside window sources. Window state
contains no signing key. Temporary conversations disappear when the fixture
ends; the owner's home, keychain, computers, and chat lists are untouched.

```sh
OPENAGENTS_CHAT_CAPTURE_DIR=/tmp/openagents-chat-captures \
  cargo test --locked -p openagents-desktop --bin openagents-desktop \
  live_hosted_reply_reaches_desktop_painter --target-dir target \
  -- --ignored --nocapture
```

This command intentionally makes two public hosted inference calls. Ordinary
Cargo checks do not contact the public worker.

## Checks

On pinned Rust 1.97.1:

- Shared chat: 26 pass; one public relay test remains opt-in. Controlled streams
  verify context, independent conversations, stop, retry without duplicate user
  turns, and late-result isolation.
- Desktop: 107 pass; four opt-in timing or network fixtures are ignored in the
  ordinary run. Delayed send acknowledgements preserve the selected draft.
- The opt-in live desktop fixture passes separately.
- Targeted formatting and Clippy with warnings denied pass.
- The separate phone workspace passes `cargo check --offline --locked`.
- The repeated native long-content case remains below 8.3 ms scrolling p99.

The general chat flow is implemented. Router cards, richer text interactions,
and attachments continue in #10008 through #10011 under epic #10003.
