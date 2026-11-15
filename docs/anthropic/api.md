Anthropic publishes a comprehensive API reference that documents **every publicly documented endpoint, HTTP method, request parameter, data type, response schema, and error format**.

For engineering purposes, there are three especially useful sources.

## **1. Where to find the definitive API specification**

[**Official Claude API Reference**](https://platform.claude.com/docs/en/api/overview?utm_source=chatgpt.com)

The primary source of truth. Navigate the sidebar to inspect endpoints, required and optional parameters, nested objects, response types, and examples in cURL, Python, TypeScript, Go, and other languages.

**Important:** Enable the **Include beta APIs** toggle to see experimental endpoints.

[**Official TypeScript SDK — API index**](https://github.com/anthropics/anthropic-sdk-typescript/blob/main/api.md?utm_source=chatgpt.com)

An extensive index of the SDK's resources, methods, and types. Particularly useful for understanding the entire API surface and tracking additions.

[**Official SDK — typed resource definitions**](https://github.com/anthropics/anthropic-sdk-typescript/tree/main/src/resources?utm_source=chatgpt.com)

The actual generated source code showing request interfaces, type definitions, URL paths, required fields, and serialization behavior. This is exceptionally useful when implementing API compatibility.

As of November 14, 2026, the reference displays API version `2026-10-21`. The API is hosted at:

`https://api.anthropic.com`

Anthropic's current API is substantially broader than just `/v1/messages`: it includes model inference, batches, files, agents, sandbox sessions, organization administration, usage analytics, and other infrastructure APIs.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://platform.claude.com\&sz=32)

Claude Platform Docs+2



## **2. Endpoint inventory**

For a direct list of HTTP methods and paths, these two pages are especially useful:

- [HTTP API endpoint index](https://platform.claude.com/docs/en/api/http?utm_source=chatgpt.com)
- [Beta HTTP API endpoint index](https://platform.claude.com/docs/en/api/http/beta?utm_source=chatgpt.com)

Here is a representative map of the main endpoint families.

| API family            | Example endpoints                                            | Purpose                               |
| --------------------- | ------------------------------------------------------------ | ------------------------------------- |
| Messages              | `POST /v1/messages`                                          | Model inference                       |
| Token counting        | `POST /v1/messages/count_tokens`                             | Estimate input token usage            |
| Message batches       | `POST /v1/messages/batches`, `GET /v1/messages/batches/{id}` | Asynchronous batch processing         |
| Models                | `GET /v1/models`, `GET /v1/models/{id}`                      | Model discovery and capabilities      |
| Files                 | `POST /v1/files`, `GET /v1/files`, `DELETE /v1/files/{id}`   | File management                       |
| Skills                | `POST /v1/skills`, `GET /v1/skills`                          | Custom reusable skills                |
| Skill versions        | `/v1/skills/{id}/versions`                                   | Version management                    |
| User profiles         | `/v1/user_profiles`                                          | User-scoped profiles                  |
| Organization          | `GET /v1/organizations/me`                                   | Organization identity                 |
| Admin                 | `/v1/organizations/...`                                      | API keys, workspaces, members, limits |
| Analytics             | `/v1/organizations/analytics/usage`, `/cost`                 | Usage and billing analytics           |
| Managed agents (beta) | `/v1/agents`                                                 | Persistent agent configurations       |
| Sessions (beta)       | `/v1/sessions`                                               | Stateful agent execution              |
| Environments (beta)   | `/v1/environments`                                           | Agent sandbox infrastructure          |

The beta reference additionally covers deployments, memory stores, vaults, telemetry, webhooks, plugins, and other resources. Availability can depend on beta enrollment, organization permissions, and platform.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://platform.claude.com\&sz=32)

Claude API Reference+2



## **3. Parameters: example for&#x20;**`/v1/messages`

The most important endpoint is [Create a Message](https://platform.claude.com/docs/en/api/messages/create?utm_source=chatgpt.com).

Its current request schema includes:

| Parameter        | Required | Purpose                            |
| ---------------- | -------- | ---------------------------------- |
| `model`          | Yes      | Model identifier                   |
| `messages`       | Yes      | Conversation history and content   |
| `max_tokens`     | Yes      | Maximum generated tokens           |
| `system`         | No       | System instructions                |
| `stream`         | No       | Server-sent event streaming        |
| `tools`          | No       | Client and server tool definitions |
| `tool_choice`    | No       | Control tool selection             |
| `thinking`       | No       | Adaptive or configured reasoning   |
| `output_config`  | No       | Effort and structured JSON output  |
| `cache_control`  | No       | Prompt caching settings            |
| `stop_sequences` | No       | Custom stopping sequences          |
| `metadata`       | No       | Request-associated metadata        |
| `service_tier`   | No       | Capacity selection                 |
| `speed`          | No       | Inference speed mode               |
| `container`      | No       | Container reuse and skills         |
| `inference_geo`  | No       | Inference region selection         |
| `diagnostics`    | No       | Request diagnostics                |

Some fields require specific models or beta access. The reference specifies their nested object types, enums, constraints, and semantics.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://platform.claude.com\&sz=32)

Claude API Reference+1



A minimal HTTP request looks like this:

```

curl https://api.anthropic.com/v1/messages \
  -H "authorization: Bearer $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2026-10-21" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-sonnet-5-5",
    "max_tokens": 1024,
    "messages": [
      {
        "role": "user",
        "content": "Hello!"
      }
    ]
  }'

```

For organization-scoped API keys, you may additionally need the `anthropic-workspace-id` header.

One significant versioning detail: `temperature`**,&#x20;**`top_p`**, and&#x20;**`top_k`**&#x20;were removed in API version&#x20;**`2026-10-21`. Requests containing them receive HTTP 400. Older `2023-06-01` integrations have different compatibility behavior.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://platform.claude.com\&sz=32)

Claude Platform Docs+1



## **4. Is there a downloadable OpenAPI specification?**

This is the distinction I would pay attention to.

Anthropic maintains a detailed official reference and generated SDKs, but I couldn't verify a current, publicly supported, first-party OpenAPI YAML/JSON download.

The generated SDKs are nevertheless extremely useful for programmatic analysis.

For example, to extract the SDK's HTTP endpoint inventory:

```

curl -fsSL \
  https://raw.githubusercontent.com/anthropics/anthropic-sdk-typescript/main/api.md \
  | grep -oE 'title="(get|post|put|patch|delete) /[^"]+"' \
  | sed -E 's/^title="(.*)"$/\1/' \
  | sort -u
  
```

This extracts method/path pairs from the generated API index. It is a useful starting point, although it is not a substitute for a complete OpenAPI specification. For parameters, follow each indexed method to its TypeScript request interface and referenced types.

The SDK's current metadata reports **373 configured endpoints**, but that should not be interpreted as 373 distinct, publicly available HTTP routes.&#x20;

[image](https://www.google.com/s2/favicons?domain=https://github.com\&sz=32)

GitHub+1



## **5. What I would use for an API-compatible implementation**

For an Anthropic-compatible provider interface in OpenAgents, I would maintain a versioned contract derived from the official reference and SDK types, with separate coverage for stable endpoints and beta features.

The highest-priority compatibility concerns would be request/response JSON shapes, streaming SSE events, tool-use content blocks, error formats, API version headers, and model-dependent behavior. These affect whether existing Anthropic SDK clients work correctly against another server.

**The best starting combination is the [HTTP reference](https://platform.claude.com/docs/en/api/http?utm_source=chatgpt.com) for canonical behavior and the [TypeScript SDK](https://github.com/anthropics/anthropic-sdk-typescript?utm_source=chatgpt.com) for programmatic type-level inspection.**&#x20;

