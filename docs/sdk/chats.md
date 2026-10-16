# Cloud chat replies

`openagents::chats::{send, stream, stop}` continues the same account chat that
openagents.com uses. It runs the existing server turn; it starts no local
harness and creates no second conversation. Read saved pages through
[history](chat-history.md).

```rust,ignore
let message = openagents::chats::Send::new(chat, "Continue the explanation".into())?;
openagents::chats::send(&client, &session, &message, &cancel).await?;
let mut events = openagents::chats::stream(
    &client, &session, &message.turn(), &cancel,
).await?;
while let Some(event) = events.next().await? {
    // Update the view from text, tool, card, done, or error events.
}
```

Keep `Send` after an uncertain response and retry it unchanged. For explicit
persistence, retain its chat, request ID, and text, then restore with `with_id`.
Account and deployment checks apply before sending. Text is bounded to 4,000
characters; this initial SDK composer sends words. Existing file-upload routes
keep their separate contract.

`current` finds the latest turn, including one sent on the website. `stream`
starts with replacements of that request's messages, then text suffixes. Apply
by message index and honor `replace`; reconnecting must not duplicate content.
Cards contain display data only. The apps show a result and a link to the chat
for detailed Lab controls. Private model-routing evidence is not exported.

`Event::Error` preserves typed refusals such as `QuotaExhausted` and unknown
future codes. Errors are terminal except `stream_timeout`, which asks the
caller to reconnect. Connections last at most five minutes, with keepalives;
the SDK bounds a frame to 17 MiB and supports fragmented UTF-8 and CRLF.
Canceling observation closes the client stream while the server continues.

To stop, call `stop` with the named `Turn`. The response confirms its saved
state. A late stop cannot cancel a newer turn. Partial text stays; the existing
runtime drops its model call after it observes the stopped state. The shared
native controller keeps retry identity when observation pauses and clears its
views on account/selection changes.

Existing remote-execution selections and background work keep their website
controls. An unsupported pending turn returns `409 unsupported_turn`; it never
claims that unfinished work stopped.

The [public reference](https://api.openagents.com/v1/openapi.json) describes
`/v1/threads/{id}/events` and `/stop`. Both require an account session or sign-in
key. Streaming rechecks access; a shared member can stop only their own pending
turn. Browser HTML events and forms, older aliases, and Coder 1.0 remain
compatible. [Qualification evidence](chat-replies-evidence.json) records checks.
