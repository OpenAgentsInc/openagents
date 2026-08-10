# Muse Glimmer benchmarks

This directory keeps reproducible local-runtime reports beside the harnesses
and raw evidence used to derive them.

## Reports

- [2026-08-10 M5 Max export and direct-runner benchmark](2026-08-10-muse-glimmer-m5-max.md)
- [2026-08-10 M5 Max HTTP runtime matrix](2026-08-10-m5-max-http-runtime-matrix.md)
- [2026-08-10 Omega live-chat acceptance](2026-08-10-omega-live-chat-acceptance.md)

## Run the Chat Completions matrix

Start a host on `http://127.0.0.1:8000/v1`, then run:

```sh
python3 run_llama_cpp_benchmark.py \
  --configuration target-only \
  --server-pid "$SERVER_PID" \
  --output evidence/result.json
```

The script uses only the Python standard library. It sends each scenario three
times by default, samples RSS for the server and its worker descendants,
preserves every request and response, and
exits nonzero on a transport or decoding error. It records a transport-level
completion rate for every compatible server and retains runtime-native timing
objects when the server supplies them.

Useful options:

```text
--base-url URL
--api-key KEY
--model MODEL
--reasoning-control {api,template}
--repetitions COUNT
--scenario NAME
--max-tokens COUNT
--seed INTEGER
--omit-seed
```

Repeat `--scenario` to select more than one scenario. `--max-tokens` overrides
the scenario budgets and is useful for confirming visible-answer behavior
after the bounded matrix exposes reasoning-budget exhaustion.

llama.cpp accepts the `reasoning_effort` request field, which is the default
`api` control. ExecuTorch deliberately rejects that field and exposes the
canonical template variable instead. Use `--reasoning-control template` for
ExecuTorch; the harness sends `reasoning_strength` and requests the separated
reasoning stream through `chat_template_kwargs`.

The released ExecuTorch greedy PTEs do not export sampling support. Add
`--omit-seed`; sending even a deterministic seed causes the worker to reject
the request before prefill. llama.cpp keeps the default `--seed 42` behavior.

The llama.cpp server adds its own timing object to SSE events. ExecuTorch's
reference server reports prefill and decode timing in its server log, so an
ExecuTorch run must preserve that log alongside the harness JSON.

Prove that the host consumes actual prior output on a second request with:

```sh
python3 run_conversation_history_smoke.py \
  --configuration target-only-history \
  --server-pid "$SERVER_PID" \
  --output evidence/history.json
```

For ExecuTorch, add `--reasoning-control template --omit-seed`. The second
request includes the first request's visible response and private reasoning in
its supplied assistant turn, then asks for the codeword introduced by the first
user turn.

## Evidence policy

Evidence files may include model output and benchmark prompts. They must not
contain credentials, machine serial numbers, user documents, private source,
or unrelated environment data. Record only the hardware model, OS version,
runtime revisions, artifact hashes, launch configuration, and metrics needed
to reproduce the result.
