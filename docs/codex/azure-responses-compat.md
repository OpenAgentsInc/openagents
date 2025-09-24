# Azure Responses Compatibility

Some Azure endpoints require special handling when using the OpenAI Responses
API.

File: `codex-rs/core/src/client.rs`

## Workaround behavior

- When `ModelProviderInfo::is_azure_responses_endpoint()` is true:
  - Set `store: true` on the request payload.
  - `attach_item_ids` injects stable `id` fields into each `input` item so that
    the service can correlate incremental responses with originals.

## Why

- Azure rejects `store: false` and may require IDs for streaming chains.
- The code paths keep non‑Azure providers lean while still interoperating with
  Azure’s variant.

