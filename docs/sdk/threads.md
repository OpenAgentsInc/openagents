# Account thread API

The [public reference](https://api.openagents.com/v1/openapi.json) includes
source-backed thread schemas from `thread_openapi.rs` (#11867). Existing
`/coder/sessions*`, `/coder/check-in`, and `/coder/sync` aliases keep their
handlers and deprecation headers. No Coder 1.0 program changes.

Use account bearer authentication; browser cookies do not authorize these
routes. Every request checks the account and resource access. Successful sync
credential checks can be cached for 30 seconds. Shared project reads require
current membership; replies require editor/owner and use the sender’s allowance.
Only the chat owner can move, delete, or manage a public share. Share links
expose words, never attachment downloads, and stop working after revocation.

`GET /v1/threads` has two existing forms. Without `project`, it lists owned,
nonarchived chats pinned first, then newest, using `offset` (default 0) and
`limit` (default 200, clamped to 1–200). With `project`, it returns `data/next`,
oldest activity first, using `after` and `limit` (default 100, clamped to 1–200).
A project can be an ID, `name:NAME`, or `none`. The two envelopes are intentional.
Neither is a snapshot. `/messages` is an unversioned text-only cursor page;
use [saved history](chat-history.md) for complete rich, revision-pinned reads.

Starting or replying accepts a UUID `request_id`. Reuse it after a lost response.
A start returns 201, or 200 for an existing chat; a reply returns 202 with
`answering` (cloud) or `queued` (computer). Text is trimmed and bounded to
4,000 characters; secrets refuse 422. Busy/offline/phone refusals are 409.
Files use raw uploads before sending, with at most four per message. Uploads
are not idempotent: keep the returned file ID. The OpenAPI response descriptions
and the shared error envelope preserve the existing refusal codes.

Cloud replies use the website’s existing runtime and sender’s allowance.
This qualification does not add public JSON streaming or cancellation: browser
`/chat/{id}/events` serves HTML SSE, and browser forms own stopping. Public
stream/stop and ergonomic SDK sending are follow-up #11893. No read-only
history operation sends, executes, contacts a host, or changes seen state.

Computer sync saves snapshots; the computer executes under its own authority.
`POST /v1/threads/synced/{session}/replies` takes queued work **at most once**.
If its response is lost, another take does not redeliver it. Repeated uploads
are content-deduplicated; a website deletion returns 410 to prevent resurrection.
Sync bodies are bounded to 8 MiB, 4,000 messages, and 256 KiB per message;
captures to 15 MiB. Pending replies are bounded to four, with 16 remembered IDs.
A sync preference does not itself upload or execute anything.

[Qualification evidence](threads-evidence.json) records isolated handler and
schema checks. Tests use synthetic accounts, temporary stores, and loopback
services; no inference, real computers, or paid calls are needed.
