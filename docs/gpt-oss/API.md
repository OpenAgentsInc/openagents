# GPT-OSS API Reference

This document describes the HTTP surface that `crates/gpt-oss` expects and the Rust types that map to it.

## Base URL

`GptOssClient::new()` reads `GPT_OSS_URL` or defaults to `http://localhost:8000`.

## Endpoints

### `GET /health`

Health probe used by `GptOssClient::health()` and `LocalModelBackend::initialize()`.

**Response:**
```json
{ "status": "ok" }
```

The client treats `status` values `ok` or `healthy` as ready.

### `GET /v1/models`

List available models.

**Supported response shapes:**
```json
[
  { "id": "gpt-oss-20b", "name": "GPT-OSS 20B", "context_length": 8192 }
]
```

or

```json
{
  "data": [
    { "id": "gpt-oss-20b", "name": "GPT-OSS 20B", "context_length": 8192 }
  ]
}
```

Mapped to `GptOssModelInfo`:

- `id` (string, required)
- `name` (string, required)
- `description` (string, optional)
- `context_length` (number, optional; defaults to 8192)

### `POST /v1/completions`

Generate a completion. Streaming uses Server-Sent Events (SSE).

**Request (`GptOssRequest`):**
```json
{
  "model": "gpt-oss-20b",
  "prompt": "Say hello.",
  "max_tokens": 128,
  "temperature": 0.7,
  "top_p": 0.9,
  "stop": ["END"],
  "stream": false
}
```

**Response (`GptOssResponse`):**
```json
{
  "id": "completion-123",
  "model": "gpt-oss-20b",
  "text": "Hello.",
  "finish_reason": "stop",
  "usage": {
    "prompt_tokens": 5,
    "completion_tokens": 2,
    "total_tokens": 7
  }
}
```

### `POST /v1/responses`

Responses API endpoint supporting tool definitions and reasoning effort.

**Request (`GptOssResponsesRequest`):**
```json
{
  "model": "gpt-oss-20b",
  "input": "Summarize this repo.",
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "browser",
        "description": "Search the web",
        "parameters": {
          "type": "object",
          "properties": { "query": { "type": "string" } }
        }
      }
    }
  ],
  "tool_choice": "auto",
  "reasoning": { "effort": "low" },
  "max_output_tokens": 256,
  "temperature": 0.7,
  "stream": false
}
```

**Response (`GptOssResponsesResponse`):**
```json
{
  "id": "resp-123",
  "model": "gpt-oss-20b",
  "output": [
    {
      "type": "message",
      "role": "assistant",
      "content": [
        { "type": "output_text", "text": "Summary..." }
      ]
    },
    {
      "type": "tool_call",
      "id": "call-1",
      "name": "browser",
      "arguments": { "query": "OpenAgents repo" }
    }
  ],
  "usage": {
    "input_tokens": 12,
    "output_tokens": 24,
    "total_tokens": 36
  }
}
```

## Streaming

When `stream: true`, the endpoint returns SSE events with JSON payloads for `GptOssStreamChunk`:

```
data: {"id":"chunk-1","model":"gpt-oss-20b","delta":"Hello ","finish_reason":null}

data: {"id":"chunk-2","model":"gpt-oss-20b","delta":"world","finish_reason":null}

data: [DONE]
```

The client converts `[DONE]` into a final chunk with `finish_reason = "stop"`.

## Rust API Summary

### Client

- `GptOssClient::new()` – uses `GPT_OSS_URL` or `http://localhost:8000`
- `GptOssClient::builder()` – configure base URL, default model, timeout
- `complete(request)` – single-shot completion
- `complete_simple(model, prompt)` – convenience wrapper
- `responses(request)` – Responses API call with tools + reasoning
- `responses_simple(model, input)` – convenience wrapper
- `stream(request)` – SSE stream
- `models()` – list models
- `health()` – readiness probe

### Harmony Helpers

- `HarmonyRenderer::gpt_oss()` – load the GPT-OSS Harmony encoding
- `HarmonyRenderer::render_prompt(...)` – render a Harmony-formatted prompt
- `HarmonyRenderer::extract_assistant_text(...)` – parse assistant output from Harmony completions

### Errors (`GptOssError`)

- `HttpError` – network/transport failures
- `ApiError` – non-2xx HTTP responses
- `JsonError` – response JSON parse failures
- `StreamError` – SSE parse failures
- `ConfigError`, `ModelNotFound`, `InitializationError`, `InvalidRequest`, `IoError`

## Related

- `docs/gpt-oss/README.md`
- `docs/local-inference.md`
- `crates/gpt-oss/src/types.rs`
