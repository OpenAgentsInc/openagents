# Chat Completions

`POST /v1/chat/completions` is the OpenAI Chat Completions API. Use it when
your code or SDK already speaks it. It reaches the same models, the same
way, at the same prices as [Open Responses](/docs/api/responses).

**Beta.**

```sh
curl https://api.openagents.com/v1/chat/completions \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "openagents/fast",
       "messages": [{"role": "user", "content": "Hi"}]}'
```

## What works the same

| Chat Completions | Notes |
| --- | --- |
| `system`, `developer`, `user`, `assistant`, `tool` messages | As in OpenAI's API |
| Text and `image_url` parts | |
| `tools`, `tool_choice`, `parallel_tool_calls` | Function tools |
| `response_format` | `json_object` and `json_schema` |
| `max_tokens`, `max_completion_tokens` | `max_completion_tokens` wins when both are set |
| `reasoning_effort` | |
| `temperature`, `top_p`, `stop`, `seed`, `user` | Where the model supports them; otherwise ignored |
| `stream`, `stream_options.include_usage` | Usage arrives in the last chunk before `[DONE]` |
| `finish_reason` | `stop`, `tool_calls`, `length`, `content_filter`, or `error` |
| `usage` | Prompt, completion, cached, and reasoning tokens, plus our `openagents` cost object |
| `openagents` | The same [routing options](/docs/api/routing) |

## What doesn't carry over

| Feature | What happens |
| --- | --- |
| `n` above 1 | `400`; send separate requests |
| `input_audio` parts, uploaded `file_id`s | `400`, naming the part |
| A message's `name` | Dropped |
| Reasoning | A summary arrives in a `reasoning` field; hidden reasoning can't be carried to the next turn |
| Citations and annotations | Dropped |
| Built-in tools such as web search | Function tools only |
| Several output items in one turn | Joined into one message, in order |

When you need reasoning across turns or more than one output item, use
[Open Responses](/docs/api/responses).
