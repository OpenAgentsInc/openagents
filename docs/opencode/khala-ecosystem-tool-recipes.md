# Khala Ecosystem Tool Recipes

> Authoritative issue #6240 recipe set. Researched against current upstream docs
> on 2026-06-25. This is repo documentation, not public marketing copy.

## Shared Khala Settings

All recipes point at the same OpenAI-compatible surface:

| Field | Value |
|---|---|
| Base URL | `https://openagents.com/api/v1` |
| Model | `openagents/khala` |
| Free key endpoint | `POST https://openagents.com/api/keys/free` |
| Free key field | `credential.token` |
| Free quota | 2,000 requests/day and 2,500,000 tokens/day per key |

Mint a key once and keep it local:

```sh
export OPENAGENTS_API_KEY="$(
  curl -fsS -X POST https://openagents.com/api/keys/free \
    | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>console.log(JSON.parse(s).credential.token))'
)"
```

Do not commit the token or paste it into issues, traces, or logs.

## Attribution Today

The gateway already accepts public-safe attribution headers and stores them in
`token_usage_events.safe_metadata_json`:

```txt
x-openagents-demand-kind: external
x-openagents-demand-source: ecosystem
x-openagents-client: <tool-id>
```

Use lower-case tool ids such as `ai-sdk`, `langchain-js`, or `litellm-proxy`.
These tags are not public counter dimensions yet; owner-gated rollups are tracked
by F1 (#6252). Until those rollups ship, tools that cannot set custom headers
should use a fresh key per tool plus the public counter before/after delta.

Manual counter check:

```sh
before="$(
  curl -fsS https://openagents.com/api/public/khala-tokens-served \
    | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>console.log(JSON.parse(s).tokensServed))'
)"

# Run the tool once.

after="$(
  curl -fsS https://openagents.com/api/public/khala-tokens-served \
    | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>console.log(JSON.parse(s).tokensServed))'
)"

echo "tokens delta: $((after - before))"
```

Do not claim per-tool public adoption from the aggregate counter alone. The
counter proves traffic was served; per-tool claims require either a fresh-key
test window or the owner-gated F1 analytics split.

## Aider

Source finding: Aider's OpenAI-compatible docs use `OPENAI_API_BASE`,
`OPENAI_API_KEY`, and require the model name to be prefixed with `openai/`.

```sh
export OPENAI_API_BASE=https://openagents.com/api/v1
export OPENAI_API_KEY="$OPENAGENTS_API_KEY"

cd /path/to/repo
aider --model openai/openagents/khala
```

Alternative explicit flags:

```sh
aider \
  --openai-api-base https://openagents.com/api/v1 \
  --openai-api-key "$OPENAGENTS_API_KEY" \
  --model openai/openagents/khala
```

Attribution: Aider's documented OpenAI-compatible path does not expose custom
request headers. Use a fresh key dedicated to Aider and record the public counter
delta for recipe verification.

Checklist:

- Basic completion works in a repo.
- Aider can inspect files and propose an edit.
- Streaming output renders normally.
- The public counter increases after the session.
- Over-quota keys surface a readable 402 instead of an opaque crash.

## Cline

Source finding: Cline's OpenAI Compatible provider expects three settings: base
URL, API key, and model ID.

In Cline settings:

| Setting | Value |
|---|---|
| API Provider | `OpenAI Compatible` |
| Base URL | `https://openagents.com/api/v1` |
| API Key | `credential.token` from the free key response |
| Model ID | `openagents/khala` |

Attribution: the current documented UI path does not expose custom attribution
headers. Use a fresh key dedicated to Cline and record the public counter delta
for recipe verification.

Checklist:

- Chat completion renders in Cline.
- File-read, file-write, and command tool flows complete.
- Streaming output renders incrementally.
- Cline displays 401, 402, and 429 failures as readable errors.
- The public counter increases after the session.

## Continue

Source finding: Continue's OpenAI-compatible provider uses `provider: openai`
with `apiBase`, `apiKey`, and `model` in `config.yaml`.

`~/.continue/config.yaml`:

```yaml
name: OpenAgents Khala
version: 0.0.1
schema: v1

models:
  - name: Khala
    provider: openai
    model: openagents/khala
    apiBase: https://openagents.com/api/v1
    apiKey: <credential.token>
```

Attribution: the current documented provider config does not expose custom
headers. Use a fresh key dedicated to Continue and record the public counter
delta for recipe verification.

Checklist:

- Chat works in the IDE panel.
- Edit/comment flows that call tools complete.
- Streaming renders normally.
- The public counter increases after the session.
- 402 quota errors are visible and actionable.

## Vercel AI SDK

Source finding: AI SDK's OpenAI-compatible provider uses
`createOpenAICompatible`; it accepts `baseURL`, `apiKey`, `headers`, and
`includeUsage`.

```ts
import { createOpenAICompatible } from '@ai-sdk/openai-compatible'
import { generateText, streamText, tool } from 'ai'
import { z } from 'zod'

const openagents = createOpenAICompatible({
  name: 'openagents',
  apiKey: process.env.OPENAGENTS_API_KEY,
  baseURL: 'https://openagents.com/api/v1',
  includeUsage: true,
  headers: {
    'x-openagents-demand-kind': 'external',
    'x-openagents-demand-source': 'ecosystem',
    'x-openagents-client': 'ai-sdk',
  },
})

const model = openagents('openagents/khala')

const { text } = await generateText({
  model,
  messages: [{ role: 'user', content: 'Reply with exactly: khala-ai-sdk-ok' }],
})

const stream = streamText({
  model,
  messages: [{ role: 'user', content: 'Stream one short sentence.' }],
})

for await (const delta of stream.textStream) {
  process.stdout.write(delta)
}

const modelWithTools = openagents('openagents/khala')
const withTools = await generateText({
  model: modelWithTools,
  tools: {
    read_note: tool({
      description: 'Read a named note',
      parameters: z.object({ path: z.string() }),
      execute: async ({ path }) => `read ${path}`,
    }),
  },
  messages: [{ role: 'user', content: 'Call read_note for docs/faq/khala-inference-quickstart.md' }],
})

console.log(withTools.text)
```

Checklist:

- `generateText` returns a non-streaming completion.
- `streamText` emits text and returns usage metadata.
- Tool-call deltas round-trip.
- The public counter increases after the run.
- Ledger metadata contains `demandClient: ai-sdk` when the owner-gated row is inspected.

## LiteLLM

Source finding: LiteLLM routes OpenAI-compatible chat completions by using the
`openai/` model prefix and `api_base`. The proxy config must use the base URL,
not a `/chat/completions` URL.

Direct Python:

```python
import os
from litellm import completion

response = completion(
    model="openai/openagents/khala",
    api_base="https://openagents.com/api/v1",
    api_key=os.environ["OPENAGENTS_API_KEY"],
    messages=[{"role": "user", "content": "Reply with exactly: khala-litellm-ok"}],
)

print(response.usage)
```

Proxy `config.yaml`:

```yaml
model_list:
  - model_name: openagents-khala
    litellm_params:
      model: openai/openagents/khala
      api_base: https://openagents.com/api/v1
      api_key: os.environ/OPENAGENTS_API_KEY
```

Run:

```sh
litellm --config config.yaml
```

Attribution: if your LiteLLM deployment can inject upstream headers, use
`x-openagents-client: litellm-proxy`. Otherwise use a dedicated key and public
counter delta.

Checklist:

- Direct `completion()` returns usage.
- Proxy serves a local `/chat/completions` request for `openagents-khala`.
- Streaming through the proxy works.
- The public counter increases after the run.
- 402 quota errors pass through clearly to the caller.

## LangChain

Source finding: LangChain Python `ChatOpenAI` accepts `base_url` or
`OPENAI_API_BASE`; LangChain JS accepts `configuration.baseURL` and
`configuration.defaultHeaders`.

Python:

```python
import os
from langchain_openai import ChatOpenAI

llm = ChatOpenAI(
    model="openagents/khala",
    api_key=os.environ["OPENAGENTS_API_KEY"],
    base_url="https://openagents.com/api/v1",
)

message = llm.invoke("Reply with exactly: khala-langchain-python-ok")
print(message.content)
```

TypeScript with attribution headers:

```ts
import { ChatOpenAI } from '@langchain/openai'

const llm = new ChatOpenAI({
  model: 'openagents/khala',
  apiKey: process.env.OPENAGENTS_API_KEY,
  configuration: {
    baseURL: 'https://openagents.com/api/v1',
    defaultHeaders: {
      'x-openagents-demand-kind': 'external',
      'x-openagents-demand-source': 'ecosystem',
      'x-openagents-client': 'langchain-js',
    },
  },
})

const message = await llm.invoke('Reply with exactly: khala-langchain-js-ok')
console.log(message.content)
```

Checklist:

- `invoke` returns an `AIMessage`/message object.
- Streaming works in the selected runtime.
- Tool calling through `bind_tools` / `bindTools` works.
- The public counter increases after the run.
- JS requests with headers write `demandClient: langchain-js` into ledger metadata.

## Research Sources

- Aider OpenAI-compatible docs: <https://aider.chat/docs/llms/openai-compat.html>
- Aider options reference: <https://aider.chat/docs/config/options.html>
- Cline OpenAI Compatible provider docs: <https://docs.cline.bot/provider-config/openai-compatible>
- Continue OpenAI-compatible provider docs: <https://docs.continue.dev/customize/model-providers/top-level/openai>
- AI SDK OpenAI-compatible provider docs: <https://ai-sdk.dev/providers/openai-compatible-providers>
- LiteLLM OpenAI-compatible endpoint docs: <https://docs.litellm.ai/docs/providers/openai_compatible>
- LiteLLM proxy config docs: <https://docs.litellm.ai/docs/proxy/configs>
- LangChain Python ChatOpenAI docs: <https://docs.langchain.com/oss/python/integrations/chat/openai>
- LangChain JS ChatOpenAI docs: <https://docs.langchain.com/oss/javascript/integrations/chat/openai>
