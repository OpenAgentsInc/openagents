# Muse Glimmer live-chat acceptance in Omega

Date: 2026-08-10

Status: complete

Tracking: [OpenAgentsInc/omega#308](https://github.com/OpenAgentsInc/omega/issues/308)

Implementation: [Omega `06323a7974`](https://github.com/OpenAgentsInc/omega/commit/06323a7974a889bf9e2429008c8820315213eebc)

Tool-calling follow-up: [Muse Glimmer tool-calling acceptance](2026-08-10-omega-tool-calling-acceptance.md)

## Accepted configuration

| Property | Value |
| --- | --- |
| Computer | MacBook Pro, Apple M5 Max (`Mac17,6`) |
| Memory | 128 GiB unified memory |
| OS | macOS 26.4, arm64 |
| llama.cpp | `10349 (62bf73d25)` |
| Model | `muse-glimmer-30B-kquant-17gb.gguf` |
| Context | 32,768 tokens |
| Parallel slots | 1 |
| Server | OpenAI-compatible Chat Completions on `127.0.0.1:8000/v1` |
| Omega model pair | `muse-glimmer/muse-glimmer` |
| Omega owner id | `muse-glimmer-local` |

The accepted server command was:

```sh
llama-server \
  --model muse-glimmer-30B-kquant-17gb.gguf \
  --alias muse-glimmer \
  --ctx-size 32768 \
  --parallel 1 \
  --seed 42 \
  --api-key local \
  --host 127.0.0.1 \
  --port 8000 \
  --metrics \
  --log-timestamps \
  --no-webui
```

Target-only llama.cpp is intentional. The companion runtime matrix found its
Metal path faster and smaller than llama.cpp DFlash for every tested scenario
on this machine. ExecuTorch MLX remains the DFlash performance lane.

The isolated Omega profile used this provider declaration:

```json
{
  "language_models": {
    "openai_compatible": {
      "muse-glimmer": {
        "api_url": "http://127.0.0.1:8000/v1",
        "available_models": [
          {
            "name": "muse-glimmer",
            "display_name": "Muse Glimmer (Local)",
            "max_tokens": 32768,
            "max_output_tokens": 8192,
            "capabilities": {
              "tools": false,
              "images": false,
              "parallel_tool_calls": false,
              "prompt_cache_key": false,
              "chat_completions": true,
              "interleaved_reasoning": false,
              "max_tokens_parameter": true
            }
          }
        ]
      }
    }
  }
}
```

The server and Omega process both received the same nonempty local bearer
token through `MUSE_GLIMMER_API_KEY`. The value is deliberately omitted here.

## Product behavior

Omega exposes `Muse Glimmer (Local)` as a separate opt-in composer entry only
when the exact compatible provider and model are configured. `Omega Agent`
remains first and keeps its existing router-backed implementation.

Muse conversations use a distinct persisted owner and a bare native text
loop. The composer displays `Message Muse Glimmer (Local)`, the fixed model
face displays `muse-glimmer`, and the disclosure reads
`Muse Glimmer (Local) · muse-glimmer`. Model and profile switching are disabled
inside this entry.

The local-only policy applies on creation and restoration:

- hosted fallback is disabled;
- title generation, summarization, and compaction stay on the pinned local
  model;
- tools are omitted when the thread is local-only or the model reports no tool
  support;
- a stopped local endpoint produces Muse-specific recovery guidance instead of
  a generic transport failure.

## Live acceptance results

### Cold first turn

The first prompt asked the model to remember `ORBIT-308` and return an exact
acknowledgement. Omega sent 20,561 prompt tokens after applying its native
agent instructions and local context. llama.cpp reported:

| Metric | Result |
| --- | ---: |
| Prompt evaluation | 72.085 s, 285.23 tok/s |
| Decode | 217 tokens, 16.859 s, 12.87 tok/s |
| End to end | 88.944 s |
| Visible answer | `STORED ORBIT-308` |

The model load itself took 5.15 seconds before this turn. The large difference
from the bare API smoke is caused by Omega's 20.5K-token native-agent envelope,
not by endpoint startup or HTTP streaming.

Omega then generated the conversation title on the same pinned local model.
That separate request decoded 404 tokens at 14.38 tok/s and took 29.435
seconds. No hosted summarization request was made.

### Cached history turn

The next message asked for the nonce without repeating it. llama.cpp reused the
conversation prefix and evaluated 238 new prompt tokens:

| Metric | Result |
| --- | ---: |
| Prompt evaluation | 1.665 s, 142.93 tok/s |
| Decode | 8 tokens, 0.585 s, 13.68 tok/s |
| End to end | 2.250 s |
| Visible answer | `ORBIT-308` |

This proves that Omega resends real conversation history through the compatible
transport and that llama.cpp's prefix cache makes continued chat interactive
after the expensive cold turn.

### Cancellation

A request to count from 1 to 500 was stopped from Omega while streaming. The
server had decoded at least 100 tokens, logged cancellation of the active task,
released its slot, and Omega returned to an idle, usable composer. No second
request or replacement executor was started.

### Endpoint loss and routing isolation

The llama.cpp server was then stopped and a new message was sent from the same
Muse thread. Omega exhausted the single permitted
`muse-glimmer/muse-glimmer` rung, returned no model answer, and did not invoke an
OpenAgents or other hosted fallback.

The UI rendered `Muse Glimmer Unavailable` with instructions to start the
local OpenAI-compatible server, keep the base endpoint ending in `/v1`, and
verify `/v1/chat/completions`. Retrying remains available after the server is
restored.

## Code validation

The implementation passed:

- `cargo fmt --all --check`
- `git diff --check`
- `cargo test -p omega_front_door` — 58 passed
- `cargo test -p agent local_only` — 2 passed
- `cargo test -p agent model_without_tool_support_omits_tools` — 1 passed
- `cargo test -p agent_ui muse_` — 7 passed
- Omega delta checks for the router-backed Omega entry, composer front door,
  and forbidden bare native-server downcasts
- `./script/clippy -p agent -p agent_ui -p language_model -p omega_front_door`
- `cargo build --profile release-fast -p omega`
- the repository push preflight, including the full `omega_deltas` policy gate

## Decision and next work

Target-only llama.cpp is the default local Omega host now. It provides the
streamed OpenAI-compatible contract, reliable client-supplied history, warm
prefix reuse, cancellation, lower memory use, and actionable offline behavior.

The cold 20.5K-token Omega envelope is the largest remaining latency cost for
basic chat. Reducing that prompt or adding a smaller local-chat profile would
improve first-turn latency more than enabling llama.cpp DFlash on this Mac.

ExecuTorch MLX should remain an explicit performance lane. Its DFlash runtime
won several generation-heavy scenarios, but its reference server needs
incremental SSE and addressable warm-session reuse before it replaces
llama.cpp for this Omega path. The linked follow-up completes sequential tool
calling. Images remain follow-up work; this acceptance intentionally preserves
the text-chat, history, cancellation, restoration, and local-only failure
baseline.

## Throughput follow-up

The [throughput optimization report](2026-08-10-muse-throughput-optimization.md)
implements the prompt reduction identified above and corrects the original
llama.cpp DFlash operating point. Muse now uses a compact text-only prompt that
excludes the 20.5K-token project envelope, while Omega Agent remains unchanged.
The prompt change is [Omega `1115dc54df`](https://github.com/OpenAgentsInc/omega/commit/1115dc54df98c9c57606fd7addfda9ea21f45004).
On the matched compact workload, Q8 KV plus DFlash `n_max=3` reached a 37.87
tok/s median versus 28.95 tok/s target-only. Use the follow-up report's command
for compact chat and retain target-only Q8 for cold near-limit contexts.
