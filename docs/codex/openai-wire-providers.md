# OpenAI Wire Providers

File: `codex-rs/core/src/model_provider_info.rs`

Defines provider entries (built‑in and user‑defined) and how they map to
Requests: Responses vs Chat Completions. Handles headers, auth, retry and idle
timeouts, and Azure special cases.

## WireApi

- `Responses` — `/v1/responses`
- `Chat` — `/v1/chat/completions` (default)

## ModelProviderInfo

- `get_full_url(auth)` prefixes base URL and appends query string.
- `create_request_builder(client, auth)` applies Bearer or OAuth token and
  adds provider headers (including env‑sourced ones when present and non‑empty).
- `api_key()` resolves `env_key` with friendly error messages.
- `request_max_retries()`, `stream_max_retries()`, `stream_idle_timeout()`
  provide effective values with hard caps.
- `is_azure_responses_endpoint()` toggles Responses Azure behavior.

