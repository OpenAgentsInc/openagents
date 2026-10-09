# Web chat deletion and retention

How long the openagents.com web chat keeps a chat, and how a chat is removed
(#11038). The chat store is `crates/openagents-web/src/chat_store.rs`; the
routes are in `crates/openagents-web/src/pages/chat.rs`.

## What a person can do

- **Delete chat** in a chat's header row opens `/chat/{id}/delete`: "Delete
  this chat? This can't be undone." with **Delete** and **Cancel**. The page
  works without scripts.
- **Delete** posts the form with the browser's CSRF ticket. The server
  removes the chat at the version it just read. If the chat changed in
  between it says "This chat just changed. Try again." While an answer is
  still being written it says "Wait for the answer to finish, then delete
  this chat." A deleted chat is gone from the sidebar and its address
  answers 404. An answer that finishes later cannot write it back.
- Only the browser whose cookie owns the chat can delete it. Another
  browser gets 404.

There is no delete-all yet.

## What delete removes

- **Local store** (`--chat-store`): the chat's file and its lock file.
- **Bucket** (`--chat-bucket`): the chat's live object, fenced by its
  generation, then every older version of that object. The README asks for
  object versioning on the bucket, so each change to a chat leaves an older
  version behind; delete lists them (`versions=true`) and removes each one.
- Answer leases (`.active.json`) hold only a request ID and are left alone.

Cloud Storage **soft delete** keeps deleted objects recoverable for the
bucket's soft-delete window (7 days by default on new buckets). The privacy
page says a copy may be kept "for up to 7 days". If the bucket's window is
changed, change that sentence in
`crates/openagents-web/content/docs/privacy-and-security.md` and
`knowledge/openagents/openagents.chat-privacy.md` in the same change.

## Retention of untouched chats

Off by default. Turn it on with `--chat-retention-days DAYS` or
`OPENAGENTS_WEB_CHAT_RETENTION_DAYS=DAYS` (1 to 3650). The server then
removes chats untouched for that many days, once at start and every six
hours after, and logs how many it removed.

- "Untouched" is the chat's last write: on disk its `updated_unix`, in the
  bucket the time its current object was written (every change rewrites
  the object).
- Each removal is fenced by the version it was judged on, so a chat that
  gets a message during the sweep stays.
- Every replica may run the sweep; the generation fence makes running it
  twice harmless.

When a retention is turned on, say the number of days in the privacy page
and the chat privacy answer in the same change. Today both say there's no
time limit yet.

## The bucket lifecycle rule

[`deploy/web-chats/lifecycle.json`](../../deploy/web-chats/lifecycle.json)
deletes an older version of a chat record one day after it was replaced.
Without it, a versioned bucket keeps every earlier state of every chat
forever, even though only the newest is ever read. It does not expire live
chats; that is the server's retention above.

Apply it (owner step, not done by the change that added it):

```sh
gcloud storage buckets update gs://BUCKET \
  --lifecycle-file=deploy/web-chats/lifecycle.json
gcloud storage buckets describe gs://BUCKET \
  --format="default(lifecycle_config,soft_delete_policy,versioning_enabled)"
```

A bucket-level backstop for retention can be added later as a second rule
with `"age": DAYS` and `"isLive": true` on the same prefix. Age counts from
when the current object was written, so it matches "untouched for DAYS".
It also removes answer leases older than that, which is harmless.
