# `@openagentsinc/composer-state`

Effect Schema contracts and pure reducers for the OpenAgents command composer
accepted in ADR-0013.

The package intentionally owns only serializable composer state:

- draft document blocks;
- attachment metadata;
- selections;
- typed editing steps;
- transactions and history;
- collaboration-ready transaction envelopes, rebase mapping, and public-safe
  change summaries;
- input-rule and keymap helpers;
- Markdown parse/serialize helpers for the v1 source-first subset.

It does not render DOM, own uploads, or make routing decisions from user text.
UI packages and app integrations consume this state layer and keep platform
editing behavior native.

## Coding draft envelope

`openagents.coding_composer_draft.v1` binds the editor kernel to one private,
restart-safe coding draft without turning prompt text into routing authority.
It carries stable draft/session/thread identity, ref-only repository/worktree/
editor/diff context, explicit provider/model/account/target selection and
readiness, and an idempotent submission state machine. Queueing fails closed
while attachments are unfinished, context revisions are stale, or authority is
unavailable/revoked/offline. An explicit retry preserves the same submission,
intent, and idempotency identity and increments only the attempt counter.

The bounded submission receipt includes opaque draft/session/thread refs,
counts, context kinds, readiness, and lifecycle only. It never includes draft
text, selection contents, attachment names or executor refs, local paths, raw
diff/editor bodies, provider payloads, or account credentials.

Collaborative helpers intentionally keep raw draft text inside the private
transaction payload. Public projections should use `ComposerChangeSummary` or
attachment upload receipts, not serialized transaction steps.

## Attachment Upload Receipts

Attachments move through `staged`, `uploading`, `ready`, and `error` states.
Upload plans separate authority by surface:

- `desktop-local` registers a local attachment and keeps local-only refs local.
- `web-hosted` uploads to hosted storage before scan, parse, and thumbnail work.

`ComposerAttachmentUploadTask` may carry private executor refs such as
`local-file:` or `browser-file:` so the owning surface can do the work.
`ComposerAttachmentUploadReceipt` is the public-safe projection boundary: it
keeps metadata, digests, dimensions, status, and only content-addressed
`attachment.<surface>.sha256.*` / `attachment_thumbnail.<surface>.sha256.*`
refs. Raw file bytes, preview URLs, prompt text, local paths, and browser-local
file handles must stay out of receipts.
