# Rampart Usage Guide

This guide uses the NPM runtime because it wraps the Hugging Face model in the
full redaction pipeline. The raw model is useful for experiments, but the NPM
runtime is what chat applications should wire in.

## Install

OpenAgents uses Bun, so prefer:

```sh
bun add @nationaldesignstudio/rampart @huggingface/transformers
```

The package declares `@huggingface/transformers` as an optional peer dependency.
Add it explicitly whenever the classifier will load. For a heuristics-only
smoke path, the peer dependency is not needed at runtime.

The current OpenAgents integration installs the package in `packages/khala-tools`:

```sh
bun add @nationaldesignstudio/rampart@0.1.2 @huggingface/transformers@3.7.5 --cwd packages/khala-tools
bun add onnxruntime-node@1.21.0 --cwd packages/khala-tools
```

## Minimal Conversation Guard

Create one guard per conversation. The guard owns the placeholder table, so
reusing it across unrelated conversations can mix identities.

```ts
import { createGuard } from "@nationaldesignstudio/rampart";

const guard = await createGuard();

const safe = await guard.protect(
  "My name is Alex Rivera and my SSN is 472-81-0094.",
);

// Send this text to the model or server.
await callModel(safe.text);

// Restore placeholders locally before rendering the assistant response.
const visible = guard.reveal("Thanks, [GIVEN_NAME_1].");
```

Expected shape:

```txt
My name is [GIVEN_NAME_1] [SURNAME_1] and my SSN is [SSN_1].
```

## Browser Worker Setup

Inference should run off the UI thread in a chat surface. Create a worker entry:

```ts
// pii-worker.ts
import { registerNerWorker } from "@nationaldesignstudio/rampart/worker";

registerNerWorker(self);
```

Then create the guard from the main thread:

```ts
import { createGuard } from "@nationaldesignstudio/rampart";

const guard = await createGuard({
  device: "webgpu",
  worker: new URL("./pii-worker.ts", import.meta.url),
});
```

Use `device: "wasm"` as the safer default if WebGPU support is unknown or the
browser cannot initialize it reliably. A practical client can probe
`navigator.gpu` and fall back to WASM.

## Streaming Replies

Models can emit placeholders split across stream chunks. Use Rampart's transform
instead of doing per-chunk string replacement.

```ts
const safe = await guard.protect(userMessage);
const modelStream = await streamModelText(safe.text);

const visibleStream = modelStream.pipeThrough(guard.revealTransform());
```

## Scrub Model Output Before Persistence

The model can hallucinate or infer personal information the user never typed.
Before saving a response to logs, traces, analytics, or database rows, scrub the
response too.

```ts
const rawReply = await callModel(safe.text);
const logSafeReply = await guard.protectReply(rawReply);

await persist({
  prompt: safe.text,
  reply: logSafeReply.text,
});

return guard.reveal(rawReply);
```

The user-facing path can reveal placeholders; the persistence path should keep
the placeholdered form.

## Useful Options

| Option | Default | Use |
| --- | --- | --- |
| `model` | `nationaldesignstudio/rampart` | Hugging Face model id or local ONNX directory |
| `device` | `"wasm"` | `"wasm"` or `"webgpu"` in browsers; `"cpu"` in Node-like runtimes |
| `worker` | unset | Worker script URL for off-main-thread inference |
| `heuristicsOnly` | `false` | Skip the model and run deterministic recognizers only |
| `keepLabels` | `CITY`, `STATE`, `ZIP_CODE` | Override the default-deny keep-set |
| `aliases` | `{}` | Rename visible placeholders, e.g. `GIVEN_NAME` to `NAME` |
| `ner` | unset | Inject a custom detector and skip model loading |
| `minScore` | `0.4` | Recall-biased classifier threshold |
| `noPrefilter` | `false` | Use only with a compatible no-prefilter model ablation |

Example: keep nothing, including city/state/ZIP.

```ts
const guard = await createGuard({
  keepLabels: [],
});
```

Example: stable generic name tokens for UI copy.

```ts
const guard = await createGuard({
  aliases: {
    GIVEN_NAME: "NAME",
    SURNAME: "NAME",
  },
});
```

## Server Or CLI Smoke

For a Node/Bun smoke test, use the CPU backend:

```ts
import { createGuard } from "@nationaldesignstudio/rampart";

const guard = await createGuard({
  device: "cpu",
});

const safe = await guard.protect("Email me at alex@example.com.");
console.log(safe.text);
```

For fast tests that should not download the model, run heuristics only:

```ts
const guard = await createGuard({
  device: "cpu",
  heuristicsOnly: true,
});
```

That catches the deterministic layer only. It is not a substitute for the full
model path because it will miss names, phone numbers, addresses, and many IDs.

As of `@nationaldesignstudio/rampart@0.1.2`, the heuristics-only Bun/Node smoke
passes in this worktree, while the full CPU model initializer fails before
inference because the published browser-targeted bundle does not successfully
wire `onnxruntime-node` into the Node path. OpenAgents wraps the package with a
fail-soft Effect service so redaction remains default-on while a future Rampart
package fix can enable the full contextual model in the same call sites.

## Raw Model Use

Use this only for research or evaluation. It skips the deterministic layer,
policy layer, placeholder table, and output scrub methods.

```ts
import { pipeline } from "@huggingface/transformers";

const classifier = await pipeline(
  "token-classification",
  "nationaldesignstudio/rampart",
  {
    dtype: "q4",
    device: "wasm",
  },
);

const entities = await classifier("My name is Alex Rivera.", {
  aggregation_strategy: "simple",
});
```

For production chat, prefer `createGuard()`.

## Self-Hosting The Model

The Hugging Face repo contains the files the runtime needs:

- `config.json`
- `onnx/model_q4.onnx`
- `special_tokens_map.json`
- `tokenizer.json`
- `tokenizer_config.json`
- `vocab.txt`

Use `model` to point the guard at a local model directory or self-hosted static
path, then validate with an end-to-end smoke in the target browser and bundler.

```ts
const guard = await createGuard({
  model: "/models/rampart",
  device: "wasm",
});
```

If OpenAgents mirrors the artifact, keep the CC BY 4.0 attribution, pin the
source revision, serve the model files with immutable caching, and do not commit
large downloaded artifacts to this repo unless a separate artifact policy says
to do so.

## Verification Checklist

- Confirm the first model load path works offline or degrades clearly.
- Measure cold load, p50, p95, and UI-thread jank on target hardware.
- Confirm `protect()` runs before any model call, logging call, trace emission,
  analytics event, crash report, or remote persistence.
- Confirm the placeholder table never leaves the browser or owner-local client.
- Confirm `protectReply()` is used before storing model output.
- Add OpenAgents-specific redactors for API keys, bearer tokens, wallet
  material, Lightning invoices, internal agent refs, and other formats outside
  Rampart's label taxonomy.
- Test non-Latin-script input and government IDs as known weak paths, then
  decide whether to block, warn, fall back to a stronger detector, or collect
  explicit consent for those flows.
