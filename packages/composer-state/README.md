# `@openagentsinc/composer-state`

Effect Schema contracts and pure reducers for the OpenAgents command composer
accepted in ADR-0013.

The package intentionally owns only serializable composer state:

- draft document blocks;
- attachment metadata;
- selections;
- typed editing steps;
- transactions and history;
- input-rule and keymap helpers;
- Markdown parse/serialize helpers for the v1 source-first subset.

It does not render DOM, own uploads, or make routing decisions from user text.
UI packages and app integrations consume this state layer and keep platform
editing behavior native.

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
