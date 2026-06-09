# Probe Gemini Backend

Probe registers a `gemini-api` backend profile for direct Google Gemini API
calls.

Initial profile defaults:

- kind: `gemini_api`
- attach mode: `direct_api`
- auth: `api_key`
- stream mode: `sse`
- base URL: `https://generativelanguage.googleapis.com/v1beta`
- model: `gemini-3.5-flash`

API-key resolution follows the Opencode-compatible order:

1. explicit API key option;
2. `GOOGLE_GENERATIVE_AI_API_KEY`;
3. `GEMINI_API_KEY`;
4. `PROBE_OPENAGENTS_BEARER_TOKEN` when `PROBE_OPENAGENTS_BASE_URL` is also set;
5. typed missing-credential failure.

Gemini auth receipts record only the source label and `apiKeyRedacted: true`.
They must not include the raw key or provider request headers. Runtime HTTP
code may still construct the `x-goog-api-key` header at the final request
boundary.

When `PROBE_OPENAGENTS_BASE_URL` and `PROBE_OPENAGENTS_BEARER_TOKEN` are both set and no
local Gemini API key exists, Probe routes Gemini `streamGenerateContent` calls
through OpenAgents product surface's authenticated Gemini broker at:

```txt
<PROBE_OPENAGENTS_BASE_URL>/api/provider-accounts/google-gemini
```

In that mode Probe sends the OpenAgents programmatic-agent bearer token to
OpenAgents product surface. OpenAgents product surface owns the Google `GEMINI_API_KEY` Worker secret and forwards the
request to `generativelanguage.googleapis.com`. Probe never receives the raw
Gemini API key.

For the current Probe/OpenAgents product surface integration, Gemini uses basic Google API keys
restricted to `generativelanguage.googleapis.com`. ADC, service-account JSON,
OAuth refresh tokens, and service-account-bound authorization keys are out of
scope for this backend path.

OpenAgents product surface's Cloudflare Worker consumes the same basic-key shape through its
`GEMINI_API_KEY` Worker secret. Do not place Gemini keys in `vars`, D1, issue
comments, docs, browser-visible responses, or public receipts.

## CLI And Assignment Selection

Probe exposes a narrow direct Gemini CLI path:

```sh
probe chat
probe chat --prompt "Read README.md and summarize the project."
probe backend gemini smoke --model gemini-3.5-flash
probe backend gemini complete --model gemini-3.5-flash --prompt "Summarize this repository."
```

`PROBE_BACKEND_PROFILE=gemini-api` can select the profile when `--profile` is
not passed. CLI output reports the API-key source label and
`apiKeyRedacted: true`, but never prints the key or provider request headers.

`probe chat` starts a local prompt loop backed by the same Gemini client and
auth precedence. It defaults to `gemini-3.5-flash`, uses local Gemini API keys
when present, and otherwise uses the OpenAgents product surface broker when `PROBE_OPENAGENTS_BASE_URL`
and `PROBE_OPENAGENTS_BEARER_TOKEN` are set. Each turn prints native tool calls,
tool results, streamed assistant text, round trips, and token usage. The chat
tool root defaults to the umbrella workspace above the Probe repo and can be
overridden with `PROBE_WORKSPACE_ROOT=/Users/christopherdavid/work`. The
initial starter tool menu includes:

- `read_file`: reads a UTF-8 file under the configured tool root, capped to a
  small text slice;
- `list_files`: lists files below a workspace-relative directory;
- `search_code`: searches workspace files with ripgrep;
- `current_time`: returns the current local timestamp.

Chat output uses ANSI colors when stdout is a terminal. For scripts and logs,
disable colors with `--no-color`, `PROBE_NO_COLOR=1`, or `NO_COLOR=1`. Force
colors with `--color always` or `PROBE_COLOR=always`.

Runner assignments can select Gemini with:

```json
{
  "backend": {
    "kind": "gemini_api",
    "backendProfileId": "gemini-api"
  }
}
```

Gemini assignments require the runner capability `probe.backend.gemini_api`.
They do not require an OpenAgents product surface provider-account grant in this pass; the API key
is resolved from the runner environment using the same precedence as local CLI
calls. Apple FM remains the default backend profile for existing assignments
that do not select Gemini.

The OpenAgents product surface-managed provider-account path is designed in
`docs/probe-openagents-google-gemini-provider-account-design.md`. OpenAgents product surface now exposes
a redacted `google_gemini` grant response for Probe-compatible assignment
metadata, but live hosted inference uses the broker route above so the Worker
secret never leaves OpenAgents product surface.

## Request Lowering

Gemini request lowering lives in
`packages/runtime/src/backends/gemini/protocol.ts`.

It converts the provider-neutral Probe LLM request contract into Gemini
`streamGenerateContent` bodies:

- top-level Probe system messages become Gemini `systemInstruction`;
- chronological system messages inside the transcript become wrapped user
  system-update text;
- user text and media become Gemini `user` content parts;
- assistant text, reasoning, and tool-call history become Gemini `model`
  content parts;
- tool results become Gemini `functionResponse` parts;
- Probe tool definitions become Gemini native function declarations;
- tool choice maps to Gemini function-calling modes.

Tool schemas are sanitized in
`packages/runtime/src/backends/gemini/tool-schema.ts` before they are sent to
Gemini. This avoids request-time failures for common JSON Schema shapes that
Gemini rejects, such as integer enums, dangling required fields, untyped arrays,
and scalar schemas carrying object-only keys.

## SSE Stream Parsing

`parseGeminiSseStream` converts Gemini `streamGenerateContent?alt=sse` events
into Probe LLM events. It preserves visible text, reasoning text, native
function calls, finish reason, token usage, cached-token counts, and Gemini
thought signatures as provider metadata.

Gemini reports `candidatesTokenCount` as visible output and
`thoughtsTokenCount` separately. Probe usage stores inclusive output tokens, so
the parser sums those fields when both are available.

## Native Tool Loop

`makeGeminiClient` exposes a `complete` method that sends Probe LLM requests to
Gemini and handles native function-call continuation:

1. lower the current Probe transcript into Gemini `contents`;
2. send native function declarations;
3. parse Gemini SSE events;
4. dispatch emitted function calls through Probe's provider-neutral tool
   runtime;
5. append assistant tool-call history and user `functionResponse` history;
6. repeat until Gemini returns final text or the round-trip limit is reached.

The loop uses Gemini native function declarations and responses. It does not
reuse the Apple FM callback URL bridge.

## Receipts And Capability Reporting

Gemini receipts live in `packages/runtime/src/backends/gemini/receipts.ts`.
They cover:

- backend availability;
- backend failures;
- transcript summaries;
- native tool-call summaries.

Receipts store profile/model/source labels and normalized usage, but not raw
Gemini request bodies, raw prompts, tool inputs, provider payloads, API keys, or
request headers.

`reportGeminiBackendCapability` advertises the `probe.backend.gemini_api`
capability only when a Gemini API key can be resolved. Gemini direct API
support reports SSE streaming and native tool calls; it does not report Apple FM
callback support.

## Test Coverage

Gemini fixture tests cover auth precedence, missing-key redaction, body
lowering, native tool declarations, tool history, schema sanitation, SSE stream
parsing, fake end-to-end tool continuation, and malformed provider output.

Live Gemini smoke tests are skipped by default and must stay opt-in so normal
runtime tests do not require network access or a provider key. To run them:

```sh
PROBE_GEMINI_LIVE_SMOKE=1 GOOGLE_GENERATIVE_AI_API_KEY=... bun test packages/runtime/tests/gemini-live-smoke.test.ts
```

`GEMINI_API_KEY` can be used instead of `GOOGLE_GENERATIVE_AI_API_KEY`. The
live smoke path checks one plain `gemini-3.5-flash` prompt and one forced tiny
native tool call, and assertions only inspect redacted receipt surfaces.

If a newly-created Google API key returns `API_KEY_INVALID`, test an existing
restricted Generative Language API key or wait for the key to become usable
before updating the Cloudflare secret. Rotate failed temporary keys rather than
leaving them in the project inventory.
