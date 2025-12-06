# Foundation Models HTTP Bridge

HTTP server that exposes Apple's Foundation Models via an OpenAI-compatible API.

## Requirements

- macOS 26+ (Sequoia)
- Apple Silicon Mac (M1/M2/M3/M4)
- Apple Intelligence enabled in System Settings
- Xcode 26+ (for building)

## Building

```bash
./build.sh
```

This will compile the binary and place it at `../../bin/foundation-bridge`.

## Usage

```bash
# Start server (default port 11435)
./bin/foundation-bridge

# Start on custom port
./bin/foundation-bridge 8080
```

## API Endpoints

### GET /health

Check server status and model availability.

```bash
curl http://localhost:11435/health
```

Response:
```json
{
  "status": "ok",
  "model_available": true,
  "version": "1.0.0",
  "platform": "macOS"
}
```

### GET /v1/models

List available models.

```bash
curl http://localhost:11435/v1/models
```

Response:
```json
{
  "object": "list",
  "data": [
    {
      "id": "apple-foundation-model",
      "object": "model",
      "created": 1733400000,
      "owned_by": "apple"
    }
  ]
}
```

### POST /v1/chat/completions

Chat completion (OpenAI-compatible).

```bash
curl -X POST http://localhost:11435/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "messages": [
      {"role": "system", "content": "You are a helpful assistant."},
      {"role": "user", "content": "Hello!"}
    ]
  }'
```

Response:
```json
{
  "id": "fm-uuid",
  "object": "chat.completion",
  "created": 1733400000,
  "model": "apple-foundation-model",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help you today?"
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 20,
    "completion_tokens": 10,
    "total_tokens": 30
  }
}
```

## Error Handling

If Foundation Models is not available, the server returns:

```json
{
  "error": {
    "message": "Apple Intelligence is not enabled in System Settings",
    "type": "model_unavailable",
    "code": "model_unavailable"
  }
}
```

## Integration with OpenAgents

This server is designed to be auto-started by the Bun client in `src/llm/foundation-models.ts`.
The client will check health, auto-start if needed, and fall back to other providers if FM is unavailable.
