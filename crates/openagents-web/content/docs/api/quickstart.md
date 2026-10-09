# Quickstart

The OpenAgents API sends your request to a good model for the job and
charges the provider's price plus a small, posted margin. It speaks
[Open Responses](https://www.openresponses.org/) and the OpenAI Chat
Completions API, so the SDKs you already use work by changing two lines.

**Beta.** Our own apps run on this API today. Keys for everyone aren't
open yet; the guides here show how it works when they are.

| | |
| --- | --- |
| Base URL | `https://api.openagents.com/v1` |
| Same API, one domain | `https://openagents.com/api/v1` |
| Key | `Authorization: Bearer oak_...` |

## 1. Get a key

Keys start with `oak_`. Keep yours out of source code; put it in an
environment variable:

```sh
export OPENAGENTS_API_KEY=oak_...
```

## 2. Send a request

```sh
curl https://api.openagents.com/v1/responses \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "openagents/chat", "input": "Say hello in five words."}'
```

`openagents/chat` lets us pick the model. Name one yourself, such as
`google/gemini-3.8-flash`, when you want that model.

## 3. Use the OpenAI SDK

Python:

```python
import os

from openai import OpenAI

client = OpenAI(
    base_url="https://api.openagents.com/v1",
    api_key=os.environ["OPENAGENTS_API_KEY"],
)

reply = client.responses.create(model="openagents/chat", input="Say hello.")
print(reply.output_text)
```

JavaScript:

```js
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://api.openagents.com/v1",
  apiKey: process.env.OPENAGENTS_API_KEY,
});

const reply = await client.chat.completions.create({
  model: "openagents/chat",
  messages: [{ role: "user", content: "Say hello." }],
});
console.log(reply.choices[0].message.content);
```

Add `stream: true` to either call to get the answer as it's written.

## From the command line

```sh
openagents inference google/gemini-3.8-flash "Say hello in five words."
openagents inference openagents/chat --input "Tell me a story." --stream
openagents inference models
```

It reads your key from `OPENAGENTS_API_KEY`. With no key,
`--pay x402 --max-msat N` pays for one request over Lightning
([Pay per request](/docs/api/pay-per-request)).

## Next

- [Models and prices](/docs/api/models): every model and what it costs.
- [Which model to use](/docs/api/decisions): pick by task, price, and speed.
- [Errors](/docs/api/errors): what each error means and what to do.
- The OpenAPI description of every route: `https://api.openagents.com/v1/openapi.json`.
