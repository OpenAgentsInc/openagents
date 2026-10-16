# Start a chat and stream the answer

Two ways to talk to OpenAgents from code:

| Way | Credential | Streams | Kept |
| --- | --- | --- | --- |
| The model API: `POST /v1/responses` or `POST /v1/chat/completions` | An `oak_` API key (or no key, paid per request) | Yes, server-sent events | Nothing between requests: send the whole conversation each time |
| An OpenAgents chat: `POST /v1/threads` on openagents.com | A login key (`oa_sk_...`) | No: read it until it's done | A chat on the account, also shown on the website and phone |

## With an API key: stream a model answer

### Step 1: send the request with `"stream": true`

```sh
curl -N https://api.openagents.com/v1/responses \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "openagents/chat", "input": "Write a haiku about Lightning.", "stream": true}'
```

### Step 2: read the events

Each event is an `event:` line and one `data:` line of JSON. The stream
ends with `data: [DONE]`.

```text
event: response.created
data: {"type":"response.created","sequence_number":0,"response":{"id":"resp_...","status":"in_progress",...}}

event: response.output_text.delta
data: {"type":"response.output_text.delta","item_id":"msg_...","output_index":0,"content_index":0,"delta":"Sats ","sequence_number":4}

event: response.completed
data: {"type":"response.completed","sequence_number":21,"response":{"id":"resp_...","status":"completed","usage":{...}}}

data: [DONE]
```

| Event | What to do |
| --- | --- |
| `response.output_text.delta` | Append `delta` to the answer |
| `response.output_text.done` | The full text of one part |
| `response.function_call_arguments.delta` | Append to a tool call's arguments |
| `response.completed` | Done; `response.usage` has the token counts |
| `response.incomplete` | Stopped early, for example at `max_output_tokens` |
| `response.failed`, `error` | Failed after it started; read `error.code` and `error.message` |

Add the header `x-openagents-events: 1` to also get our own events:
`openagents:route` (`{"model", "upstream"}`: which model and provider
answered) and `openagents:cost` (`{"cost"}`).

The answer's headers name the model and provider: `x-openagents-model`
and `x-openagents-upstream`, with `x-request-id` for help.

### The same with Chat Completions

```sh
curl -N https://api.openagents.com/v1/chat/completions \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "openagents/chat", "messages": [{"role": "user", "content": "Say hello."}], "stream": true, "stream_options": {"include_usage": true}}'
```

Each line is `data: {"object":"chat.completion.chunk","choices":[{"index":0,"delta":{"content":"..."},"finish_reason":null}],...}`,
then a last chunk with `usage` and no choices, then `data: [DONE]`.

### With the command

```sh
export OPENAGENTS_API_KEY=oak_...
openagents inference openagents/chat "Write a haiku about Lightning." --stream
openagents inference openagents/chat "Write a haiku about Lightning." --stream --format events
```

`--format events` prints each event as one JSON line.

### With an OpenAI SDK

```python
from openai import OpenAI

client = OpenAI(base_url="https://api.openagents.com/v1", api_key=OPENAGENTS_API_KEY)
with client.responses.stream(model="openagents/chat", input="Say hello.") as stream:
    for event in stream:
        if event.type == "response.output_text.delta":
            print(event.delta, end="")
```

## With a login key: an OpenAgents chat

This starts a chat on the account, as starting one on the website does.
It can run tools and Coder, and it shows on the website and phone.

### Step 1: start the chat

```sh
curl -s https://api.openagents.com/v1/threads \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"request_id\": \"$(uuidgen)\", \"text\": \"What changed in my repo this week?\"}"
```

```json
{"id": "...", "answering": true}
```

`request_id` is a UUID you make. Sending the same one again returns the
same chat (`200`) instead of a second one, so a retry is safe. Add
`"project": "prj_..."` (or `"name:Launch notes"`) to start the chat inside
a project. Text is at most 4,000 characters.

### Step 2: read it until the answer is done

```sh
curl -s https://api.openagents.com/v1/threads/THREAD_ID \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN"
```

```json
{"thread": {"id": "...", "working": true, ...}, "messages": [{"role": "user", "text": "..."}, {"role": "assistant", "text": "..."}], "earlier": false, "waiting": false}
```

Read it again about once a second while `thread.working` is `true`. The
last `assistant` message grows as the answer is written.

### Step 3: reply

```sh
curl -s https://api.openagents.com/v1/threads/THREAD_ID/messages \
  -H "Authorization: Bearer $OPENAGENTS_TOKEN" \
  -H "Content-Type: application/json" \
  -d "{\"request_id\": \"$(uuidgen)\", \"text\": \"Only the main branch, please.\"}"
```

The answer is `202`; read the chat again for the reply.

| Status | `code` | Why |
| --- | --- | --- |
| 400 | `invalid` | `request_id` isn't a UUID, or the text is empty or too long |
| 409 | `busy` | The chat is still answering |
| 422 | `secret` | The text looks like a password or key; it wasn't sent |

### With the command

```sh
openagents chat send "What changed in my repo this week?" --project "Launch notes" --json
```

Under `--json` it prints one event per line while the answer is written
(`{"event":"partial","thread":"...","text":"...","delta":"..."}`), then
the result. Without `--project`, the chat is kept on this computer.

## See also

- [Errors](/docs/agents/errors)
- [Responses](/docs/api/responses) and [Chat Completions](/docs/api/chat-completions), the full API guides
