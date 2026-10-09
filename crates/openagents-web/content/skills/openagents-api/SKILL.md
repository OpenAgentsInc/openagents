---
name: openagents-api
description: Call the OpenAgents API (Open Responses and Chat Completions, many models, one key) and read its docs. Use when a task needs a model through OpenAgents, or asks what OpenAgents models cost.
---

# OpenAgents API

One API for many models. It speaks Open Responses and the OpenAI Chat
Completions API, so OpenAI SDKs work with two changes. Beta.

| | |
| --- | --- |
| Base URL | `https://api.openagents.com/v1` (also `https://openagents.com/api/v1`) |
| Key | `Authorization: Bearer $OPENAGENTS_API_KEY` (an `oak_` key) |
| Docs | <https://openagents.com/docs/api.md> |
| Prices | <https://openagents.com/docs/api/models.md> |
| Getting a key | <https://openagents.com/auth.md> |

## Steps

1. Read the key from `OPENAGENTS_API_KEY`. If it is missing, tell the person
   to make one at <https://openagents.com/settings/api-keys>; never ask for
   it in chat or write it to a file.
2. Send a request:

   ```sh
   curl https://api.openagents.com/v1/responses \
     -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
     -H "Content-Type: application/json" \
     -d '{"model": "openagents/chat", "input": "Say hello."}'
   ```

   `openagents/chat` lets OpenAgents pick the model. Name one (see the
   prices page) to choose it yourself.
3. With an OpenAI SDK, set `base_url` to `https://api.openagents.com/v1` and
   pass the key as the API key.
4. On `429`, wait for `Retry-After`. On `402`, the balance is too low; tell
   the person. Every answer has an `x-request-id` to quote when asking for
   help.

## Reading the docs as tools

The docs MCP server at `https://openagents.com/mcp/docs` (Streamable HTTP,
no key) has `list_docs`, `search_docs`, `read_doc`, and `list_models`.
