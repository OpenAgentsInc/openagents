# Muse Glimmer tool-calling acceptance in Omega

Date: 2026-08-10

Status: request-loop acceptance complete

Implementation: [Omega `70bfe6ab75`](https://github.com/OpenAgentsInc/omega/commit/70bfe6ab75)

This follow-up enables sequential tool use in the isolated local Muse entry.
It preserves the local-only routing policy: tool schemas, tool results, title
generation, summarization, and compaction stay on the pinned
`muse-glimmer/muse-glimmer` model, and a local failure does not fall through to
a hosted model.

## Configuration

The accepted llama.cpp command is:

```sh
llama-server \
  --model muse-glimmer-30B-kquant-17gb.gguf \
  --spec-draft-model dflash-kquant.gguf \
  --spec-type draft-dflash \
  --spec-draft-n-max 3 \
  --spec-draft-n-min 0 \
  --spec-draft-p-min 0.00 \
  --spec-draft-backend-sampling \
  --flash-attn on \
  --cache-type-k q8_0 \
  --cache-type-v q8_0 \
  --alias muse-glimmer \
  --ctx-size 32768 \
  --parallel 1 \
  --seed 42 \
  --host 127.0.0.1 \
  --port 8000 \
  --metrics \
  --log-timestamps \
  --no-webui \
  --jinja
```

`--jinja` is required. Without it, the model can describe a function call in
reasoning or text without llama.cpp emitting the OpenAI `tool_calls` object.
With it, the Muse-aware template and ATEM parser emit fragmented OpenAI tool
deltas followed by `finish_reason: "tool_calls"`.

No person-configured API key is required. Omega supplies an internal local
bearer value only for the exact Muse provider on a loopback endpoint, and a
llama.cpp server without `--api-key` accepts the request.

The Omega model declaration uses:

```json
{
  "capabilities": {
    "tools": true,
    "images": false,
    "parallel_tool_calls": false,
    "prompt_cache_key": false,
    "chat_completions": true,
    "interleaved_reasoning": false,
    "max_tokens_parameter": true
  }
}
```

The Basic profile exposes `read`, `write`, `edit`, `bash`, and `delegate` to
Muse. Parallel tool calls remain disabled because Muse supports one call per
turn.

## Live protocol acceptance

The first live request required a `get_weather` function with the argument
`city`. llama.cpp streamed the tool-call ID and name, then these argument
fragments:

```text
{
"city":"
Paris
"
}
```

The assembled call was `get_weather({"city":"Paris"})`. The terminal event
used `finish_reason: "tool_calls"`.

| Metric | Tool-selection request |
| --- | ---: |
| Prompt tokens | 389 |
| Prompt evaluation | 897.643 ms, 433.36 tok/s |
| Generated tokens | 89 |
| Decode | 2,476.849 ms, 35.93 tok/s |
| Draft tokens | 105 |
| Accepted draft tokens | 53 |

The follow-up request replayed the assistant tool call and supplied this tool
result:

```json
{"city":"Paris","temperature_c":22,"conditions":"sunny"}
```

Muse returned `The current weather in Paris is **22°C and sunny**.` with
`finish_reason: "stop"` and no additional call.

| Metric | Tool-result request |
| --- | ---: |
| Prompt tokens | 474 |
| Prompt evaluation | 867.235 ms, 546.56 tok/s |
| Generated tokens | 72 |
| Decode | 2,633.592 ms, 27.34 tok/s |
| Draft tokens | 84 |
| Accepted draft tokens | 44 |

## Omega validation

The agent-loop regression covers the complete sequence:

1. A local-only Muse-capable request contains the enabled tool schema.
2. The model emits a complete tool call and stops for tool use.
3. Omega executes the tool and includes the successful result in a second
   request.
4. The local model returns the final answer and the turn completes.

The Muse UI regression also proves that the pinned model reports tool support
and that the Basic profile exposes the five core tools. A model that reports
no tool support still receives no tool schemas and gets the compact text-only
prompt.

Validation commands:

- `cargo test -p agent local_only_thread_runs_model_supported_tools --lib`
- `cargo test -p agent compact_local_agent_uses_tools_without_expanding_its_system_prompt --lib`
- `cargo test -p agent model_without_tool_support_omits_tools --lib`
- `cargo test -p open_ai stream_maps_preserves_tool_id_and_name_across_empty_deltas --lib`
- `cargo test -p agent_ui muse_glimmer_selection_is_isolated_from_omega_agent --lib`
- `./script/clippy -p agent -p agent_ui`
- Omega push preflight, including `omega_deltas` and changed-file rustfmt checks

All listed gates passed.

## Remaining scope

This acceptance covers sequential JSON-schema function calls and the complete
tool-result loop. Image input remains disabled. Broader scenario benchmarks
should cover file reads, edits, terminal commands, tool failure recovery, and
multiple sequential calls before parallel calls are considered.
