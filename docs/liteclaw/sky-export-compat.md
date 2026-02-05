# LiteClaw Sky Export Compatibility

This document describes the **minimum guarantees** for LiteClaw Sky exports.

## Versioning

- `liteclaw_session_version` is emitted in the export header.
- `cf_sky_version` is emitted in the export header and in every run receipt.
- Schema versions are included in the export header and on each record.

## Required records

Every export **must** include:

- A header record with `type: "liteclaw.export"`.
- One or more `message` records for the transcript.
- Any `run`, `event`, and `receipt` records for runs that happened in the session.

## Event contract

Events are keyed by `type` and must validate against the schema for that type.
Current canonical event types:

- `run.started`
- `model.delta`
- `model.completed`
- `run.error`
- `run.completed`
- `tool.call.started`
- `tool.call.args.delta`
- `tool.call.args.completed`
- `tool.call.completed`
- `tool.result`

## Tool args streaming

Tool arguments are streamed as JSON deltas via `tool.call.args.delta`, then
finalized as a fully validated `tool.call.args.completed` payload.

## Attachments

Binary attachments are normalized into `ref` parts pointing at R2 keys:

- `r2://bucket/key#sha256=...`

Consumers should treat `ref` parts as opaque references to external blobs.
