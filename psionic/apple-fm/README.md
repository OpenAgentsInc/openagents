# Apple FM bridge

Apple's on-device foundation model, reached through `FoundationModels`. The
bridge is a small Swift program because the framework has only a Swift API.
Everything that uses it is Coax: [psionic/lib/apple-fm.coax](../lib/apple-fm.coax).
It started as the Lev helper (`swift/lev-bridge`, removed in 2026-10), without
the adapter operations.

Build it on a Mac with macOS 26 and Apple Intelligence turned on:

```sh
psionic/apple-fm/build.sh
```

The binary is `psionic/apple-fm/.build/release/apple-fm-bridge`. Git ignores
`.build`.

## Protocol

One JSON request a line on stdin, one response a line on stdout. Each request
gets its own session, so no request sees another's text. Nothing leaves the
Mac.

```text
→ {"id":"1","op":"availability"}
← {"id":"1","ok":true,"status":"available"}
→ {"id":"2","op":"generate","instructions":"…","prompt":"…","maxTokens":40}
← {"id":"2","latencyMs":1463,"ok":true,"text":"Fixing off-by-one error in parser.coax."}
→ {"id":"3","op":"decide","prompt":"Is this a work request: can you fix CI?","options":["yes","no"]}
← {"choice":"yes","id":"3","latencyMs":408,"ok":true}
```

An unavailable model answers `"status":"unavailable"` with a `reason`:
`apple_intelligence_not_enabled`, `device_not_eligible` or `model_not_ready`.
A failed request answers `"ok":false` with an `error` code from the
framework's generation errors, such as `guardrail_violation` or
`exceeded_context_window_size`.

`decide` constrains the answer to the given options with a generation
schema. Lev's measurements (2026-09-19) found this model unanimous even
when it is wrong, so its choices carry no probability. Use it for text such
as recaps, not as a decision model.

## Users

- Coder's status bar recaps what the agent is doing ([Coder chapter 36](../../coder/book/ch036-a-terminal-ui.md#recaps)).
- `coder apple-fm ["PROMPT"]` checks the bridge and prints the model's answer.
