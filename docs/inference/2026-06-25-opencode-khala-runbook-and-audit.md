# OpenCode x Khala Runbook And Compatibility Audit

> Status: internal runbook/audit, 2026-06-25. This is not public claim copy and
> does not change any product-promise state. It operationalizes the OpenCode lane
> from [`2026-06-25-khala-inference-gtm-push.md`](./2026-06-25-khala-inference-gtm-push.md)
> and keeps the honesty boundary from
> [`../promises/2026-06-25-khala-inference-push-promise-review.md`](../promises/2026-06-25-khala-inference-push-promise-review.md):
> Khala is a live OpenAI-compatible endpoint, but OpenCode tool-loop support is
> gated on the exact API compatibility smoke below before it becomes public copy.

## Summary

OpenCode is the right first ecosystem-tool target for Khala because it is a
provider-agnostic coding agent, uses the AI SDK, and can point at an
OpenAI-compatible endpoint with a base URL, bearer key, and model id. The local
OpenCode reference clone was refreshed on 2026-06-25:

- clone: `/Users/christopherdavid/work/projects/repos/opencode`
- upstream: `https://github.com/anomalyco/opencode.git`
- branch: `dev`
- refreshed head: `c45d1db9a` (`origin/dev`)
- local installed CLI: `opencode 1.17.9`
- refreshed source package version: `1.17.10`

Direct Khala API smoke works: a plain chat-completions request to
`openagents/khala` returned `200`, reported `usage.total_tokens: 399`, and the
public `GET /api/public/khala-tokens-served` counter increased by exactly `399`.
That proves the base endpoint, auth, usage reporting, and public token counter are
alive for simple chat.

The first OpenCode smoke exposed a real compatibility bug: OpenCode sends
standard OpenAI-compatible typed message content arrays and streamed tool-call
deltas. Khala rejected text-only content arrays with
`400 {"error":"invalid_request"}` and dropped `tool_calls` payloads while still
returning `finish_reason: "tool_calls"`. Tracked as
[#6232](https://github.com/OpenAgentsInc/openagents/issues/6232). The gateway
fix in this revision normalizes text-only content arrays, preserves request-side
tool replay metadata (`tool_calls`, `tool_call_id`, `name`), and forwards
non-streaming plus streamed OpenAI-compatible tool-call payloads through the
Hydralisk, Fireworks, and generic OpenAI passthrough lanes.

## Runbook

### 1. Refresh the OpenCode reference clone

```sh
cd /Users/christopherdavid/work/projects/repos/opencode
git status --short --branch
git pull --ff-only
git log -1 --oneline --decorate
```

If the checkout is dirty with local reference work, do not stash or reset it.
Use the workspace `projects/sync.sh` lane from the root only when the reference
clone is clean enough for a fast-forward.

### 2. Confirm the local CLI

```sh
which opencode
opencode --version
```

For a strict latest-source smoke, either upgrade the installed CLI or run from the
refreshed clone. For the first compatibility test, the installed `1.17.9` CLI was
enough to expose the production API blocker.

### 3. Get a Khala API key

Prefer an existing registered `oa_agent_` token when testing repeatedly. The free
key mint endpoint is rate-limited per IP and may return `429` if previous tests
already minted keys that day.

```sh
export OPENAGENTS_API_KEY="$OPENAGENTS_AGENT_TOKEN"
```

For a new throwaway free key:

```sh
export OPENAGENTS_API_KEY="$(
  curl -fsS -X POST https://openagents.com/api/keys/free \
    | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>console.log(JSON.parse(s).credential.token))'
)"
```

Do not print the raw key into terminal transcripts, docs, issues, commits, or
benchmark artifacts.

### 4. Confirm the model catalog

```sh
curl -fsS https://openagents.com/api/v1/models
```

Expected public model id today:

```txt
openagents/khala
```

### 5. Confirm direct chat and token accounting

```sh
before="$(
  curl -fsS https://openagents.com/api/public/khala-tokens-served \
    | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>console.log(JSON.parse(s).tokensServed))'
)"

curl -fsS https://openagents.com/api/v1/chat/completions \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openagents/khala",
    "messages": [{"role": "user", "content": "Reply with exactly: khala-direct-ok"}],
    "max_tokens": 32,
    "temperature": 0
  }'

after="$(
  curl -fsS https://openagents.com/api/public/khala-tokens-served \
    | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>console.log(JSON.parse(s).tokensServed))'
)"

echo "tokens delta: $((after - before))"
```

Acceptance: the request returns `200`, carries a `usage.total_tokens` value, and
the public counter delta equals that total. A content-quality failure is a model
quality issue; a missing `usage` object or mismatched counter delta is a metering
bug.

### 6. Configure OpenCode

Use a project config for an isolated test, or global config for daily dogfood.
OpenCode reads `provider` config and migrates it internally.

```json
{
  "$schema": "https://opencode.ai/config.json",
  "provider": {
    "openagents": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "OpenAgents",
      "options": {
        "baseURL": "https://openagents.com/api/v1",
        "apiKey": "{env:OPENAGENTS_API_KEY}"
      },
      "models": {
        "khala": {
          "name": "Khala",
          "tool_call": true,
          "api": {
            "id": "openagents/khala"
          },
          "limit": { "context": 128000, "output": 65536 }
        }
      }
    }
  },
  "model": "openagents/khala"
}
```

The selected #6239 recipe uses short model key `khala` so OpenCode's
`providerId/modelKey` selector is `openagents/khala`. The `api.id` override keeps
the upstream Chat Completions request on the canonical model id:
`model: "openagents/khala"`. No Khala API alias or server behavior change is
required.

### 7. Run the OpenCode smoke

```sh
export OPENAGENTS_API_KEY="$OPENAGENTS_AGENT_TOKEN"

opencode run \
  --pure \
  --agent plan \
  --model openagents/khala \
  --dir /Users/christopherdavid/work/openagents-worktrees/khala-opencode-runbook \
  --dangerously-skip-permissions \
  'Use tools to read docs/faq/khala-inference-quickstart.md. Reply with exactly: base=<base-url>; model=<model-id>'
```

Expected result after #6232 is deployed:

```txt
base=https://openagents.com/api/v1; model=openagents/khala
```

Acceptance checklist for the #6239 recipe:

- OpenCode completes the task by invoking its file-read/tool path, not by a
  text-only guess.
- Streaming remains interactive; SSE chunks arrive normally until `[DONE]`.
- Provider usage is visible in the session/receipt path where OpenCode surfaces
  it, and the public Khala tokens-served counter increases by the same total.
- For an exhausted free key, OpenCode shows a readable 402/quota message and the
  session remains recoverable after key/top-up/reset.

Observed pre-fix result:

```txt
> plan . openagents/khala
Error: Bad Request: invalid_request
```

## Compatibility Fix Coverage

### Fixed: text-only content arrays

OpenCode sends user content as an OpenAI-compatible array of typed parts:

```json
{
  "role": "user",
  "content": [
    { "type": "text", "text": "..." },
    { "type": "text", "text": "<system-reminder>..." }
  ]
}
```

Minimal repro:

```sh
curl -sS https://openagents.com/api/v1/chat/completions \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"openagents/khala","messages":[{"role":"user","content":[{"type":"text","text":"Reply ok"}]}],"stream":true,"stream_options":{"include_usage":true},"max_tokens":64}'
```

Current response:

```json
{"error":"invalid_request"}
```

The fixed route normalizes text-only arrays into the existing string prompt path.
Unsupported part types still reject before provider dispatch.

### Fixed: tool-call finish without tool-call payload

Minimal repro:

```sh
curl -sS https://openagents.com/api/v1/chat/completions \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"openagents/khala","messages":[{"role":"user","content":"Call the read_note tool with path docs/faq/khala-inference-quickstart.md"}],"tools":[{"type":"function","function":{"name":"read_note","description":"Read a named note","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}}],"tool_choice":"auto","max_tokens":256,"temperature":0}'
```

Current response shape:

```json
{
  "finish_reason": "tool_calls",
  "message": {
    "content": "",
    "role": "assistant"
  }
}
```

An OpenAI-compatible tool runner expects `message.tool_calls[]` in non-streaming
responses and/or streaming `delta.tool_calls[]` chunks. Returning the
`tool_calls` finish reason without the payload leaves OpenCode with nothing to
execute. The fixed adapter contract carries `toolCalls` and `toolCallDeltas`
through the route and the OpenAI-compatible serving lanes.

### Fixed: tool replay metadata on the next request

After a client executes a tool, its next Chat Completions request usually
contains an assistant replay message with `tool_calls[]` and one or more
`role: "tool"` messages with `tool_call_id`. The route now preserves that
metadata through Khala's stable-prefix layout and the OpenAI-compatible adapters
send it upstream. Without this, the first tool call could be emitted but the
follow-up request would lose the association between the tool result and the
call that requested it.

### Regression tests

Run from `apps/openagents.com/workers/api`:

```sh
bun x vitest run \
  src/inference/chat-completions-routes.test.ts \
  src/inference/fireworks-adapter.test.ts \
  src/inference/hydralisk-adapter.test.ts

bun run typecheck
```

Expected local result for this revision:

```txt
Test Files  3 passed (3)
Tests       152 passed (152)
tsc exits 0
```

`bun run typecheck` may print existing TS47 advisory messages in
`src/trace-store-routes.ts`; those are not introduced by this fix and do not
change the process exit code.

## What To Do In Order

1. **Deploy the #6232 compatibility fix from clean `origin/main`.** Keep the
   Worker deploy tied to the committed source SHA; do not publish the OpenCode
   recipe until production smoke is green.
2. **Rerun the OpenCode smoke with a dedicated key.** Use a dedicated
   `oa_agent_` token for OpenCode dogfood so token attribution can distinguish
   this lane from generic internal demand.
3. **Verify token movement and tool completion.** The accepted smoke requires:
   OpenCode reads a file via its built-in `read` tool, the final answer is correct,
   `usage.total_tokens` appears in the terminal telemetry/receipt path where
   available, and the public tokens-served counter increases.
4. **Only then publish the recipe.** Keep it in repo docs first; promote to
   `/khala` and public quickstart copy only after the product-promise copy gate
   says the wording is safe.
5. **Add OpenCode to the benchmark gem.** Start the head-to-head harness with
   OpenCode as the first client surface, then add the owner-named Big Pickle
   comparator and OpenCode Zen/Go/free-model alternatives. Capture model id,
   CLI version, provider config, tokens, wall-clock, cost, accepted-outcome
   verdict, and whether tool calls completed.
6. **Broaden after OpenCode.** Aider, Cline/Continue, AI SDK, LiteLLM, and
   LangChain all become easier after the same OpenAI-compatible content/tool-call
   contracts are green.

## Benchmark Gem Shape

The benchmark artifact should be client-first, not model-only. For each run:

- client: OpenCode CLI version and source SHA where known;
- provider config: base URL, model selector, tool-call capability flag, MCP
  servers enabled;
- prompt/workload: exact public-safe task, fixture refs, and repo SHA;
- traffic: prompt/completion/total tokens from provider `usage`, not estimates;
- latency: wall-clock plus TTFT/inter-token latency when streaming telemetry
  reports it;
- outcome: did tools execute, did files change, did tests pass, did the verifier
  accept the artifact;
- economics: cost/accepted outcome where measured, never invented;
- adoption: tokens attributed to OpenCode vs other internal/external lanes.

Keep fixture runs labeled illustrative. Decision-grade comparisons need live
traffic, realistic tasks, owner-approved spend bounds, and public-safe receipts.

## Notes For The Upstream OpenCode Path

This does not require an upstream OpenCode patch to start. The original failure
was on the Khala compatibility side: OpenCode was sending normal
AI SDK/OpenAI-shaped requests. Later, once Khala passes the deployed smoke, an
upstream OpenCode docs PR or provider preset can reduce friction, but the first
job is keeping the endpoint faithful to the request shape OpenCode already
emits.
