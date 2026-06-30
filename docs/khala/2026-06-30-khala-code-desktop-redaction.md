# Khala Code Desktop Redaction

Updated: 2026-06-30

This document explains the redaction layer that runs in Khala Code Desktop
before text is sent to hosted Khala or another model backend. It covers the
current implementation, what leaves the desktop app, how Rampart is loaded
under Bun, what happens when the model path fails, how to test it, and which
gaps still need separate product decisions.

The short version: redaction is default-on for Khala Code Desktop chat turns.
The desktop runtime creates one local redaction service per chat session, uses
Rampart's placeholder table to protect user and tool text before provider
requests, scrubs assistant text before it is kept in provider context, and
reveals placeholders only for local user-visible rendering.

## Goals

The redaction layer exists to reduce accidental disclosure of private text
before owner-local desktop state crosses a model/provider boundary.

It should:

- run locally inside the owner-controlled desktop process;
- protect current user messages before provider requests;
- protect prior user messages and prior tool cards when replaying conversation
  history into a provider request;
- protect local tool result content before the result is sent back to the
  model;
- protect assistant text before storing/replaying it as model context;
- reveal placeholders for the local user-visible transcript;
- keep the Rampart session table local to one chat session;
- preserve OpenAgents-specific secret scrubbing for API keys and bearer tokens;
- report degraded modes through structured redaction refs.

It is not meant to be a security boundary. It is a privacy prefilter. Access
control, trace policy, public projection policy, product promises, payment,
payout, settlement, receipt authority, and deployment authority stay in their
own typed systems.

## Code Ownership

The shared redaction service lives in:

- `packages/khala-tools/src/redaction.ts`

Khala Code Desktop wires it into chat turns in:

- `clients/khala-code-desktop/src/bun/khala-chat-runtime.ts`

Research and package usage notes live in:

- `docs/research/rampart/README.md`
- `docs/research/rampart/usage.md`
- `docs/research/rampart/openagents-integration.md`

The tests that pin the behavior live in:

- `packages/khala-tools/src/index.test.ts`
- `clients/khala-code-desktop/tests/khala-chat-runtime.test.ts`

## Packages And Model

The implementation uses:

- `@nationaldesignstudio/rampart@0.1.2`
- `@huggingface/transformers@3.7.5`
- `onnxruntime-node@1.21.0`
- Hugging Face model id `nationaldesignstudio/rampart`

Rampart provides the conversation guard, default-deny keep policy, deterministic
structured PII recognizers, placeholder table, `protect`, `protectReply`,
`reveal`, and `revealTransform`.

OpenAgents adds:

- an Effect service wrapper;
- a Bun-compatible full-model loader adapter;
- OpenAgents-specific regex scrubbing for OpenRouter keys, generic API keys,
  and bearer tokens;
- structured mode and redaction refs.

## Runtime Service Shape

`makeKhalaPrivacyRedactionService()` returns:

```ts
type KhalaPrivacyRedactionServiceShape = {
  readonly protectUserText: (text: string) => Effect.Effect<KhalaPrivacyRedactionResult, never>;
  readonly protectModelText: (text: string) => Effect.Effect<KhalaPrivacyRedactionResult, never>;
  readonly revealForLocalUser: (text: string) => Effect.Effect<string, never>;
  readonly revealTransform: () => Effect.Effect<TransformStream<string, string>, never>;
};
```

`KhalaPrivacyRedactionResult` records:

- `engine`: Rampart or the Khala regex-only fallback;
- `mode`: `rampart_model`, `rampart_heuristics`, or `regex_only`;
- `text`: the protected text;
- `placeholders`: placeholders introduced by Rampart;
- `redacted`: whether text changed;
- `redactionRefs`: refs describing PII redaction or degraded execution.

The service is lazy. It does not load Rampart until the first text protection
call for that service instance.

## Conversation Scope

Khala Code Desktop maintains:

```ts
const redactionBySession = new Map<string, KhalaPrivacyRedactionServiceShape>();
```

`redactionForSession(sessionId)` returns the existing service for the chat
session or creates a new one with `makeKhalaPrivacyRedactionService()`.

That matters because Rampart's placeholder table is intentionally
conversation-local. If the user says "My name is Alice Johnson", Rampart can map
`[GIVEN_NAME_1] [SURNAME_1]` back to that value later in the same session. The
table must not be shared across unrelated sessions, sent to hosted Khala, logged
as public trace material, or committed to artifacts.

## Text Flow

For each chat turn, Khala Code Desktop builds provider messages from:

1. the Khala Code system prompt;
2. the tool catalog system prompt;
3. projected transcript messages.

Before any transcript content is included in provider messages:

- user messages run through `protectUserText`;
- previous tool cards are wrapped as `Previous tool result:\n...` and then run
  through `protectUserText`;
- assistant history runs through `protectModelText`.

When the model calls a local tool, the local tool result is visible in the local
transcript as the raw local result, but the content replayed to the provider as
the `tool` role is protected with `protectUserText`.

When the model returns assistant text:

1. the complete assistant text is passed through `protectModelText`;
2. the protected model-context text is pushed back into the provider message
   history;
3. the protected text is passed through `revealForLocalUser`;
4. the revealed text is rendered in the local transcript.

So the intended invariant is:

- provider requests receive placeholdered text;
- provider replay context receives placeholdered assistant/tool text;
- local user-visible transcript can show revealed placeholders;
- the placeholder table remains local.

## Example

Input:

```txt
My name is Alice Johnson. Email alice@example.com. I live at 100 Main Street in Chicago, IL 60601.
```

Expected protected provider text in full model mode:

```txt
My name is [GIVEN_NAME_1] [SURNAME_1]. Email [EMAIL_1]. I live at [BUILDING_NUMBER_1] [STREET_NAME_1] in Chicago, IL 60601.
```

Rampart's default keep-set preserves city, state, and ZIP, so `Chicago, IL
60601` remains. The name, email address, building number, and street name are
placeholdered.

If the model replies:

```txt
Hello [GIVEN_NAME_1] [SURNAME_1].
```

The local user-visible transcript renders:

```txt
Hello Alice Johnson.
```

The provider context keeps the protected form.

## Bun-Compatible Rampart Model Loading

Rampart's public package is browser-first. In `@nationaldesignstudio/rampart`
`0.1.2`, direct `createGuard({ device: "cpu" })` under Bun fails because the
published browser-targeted bundle does not wire `onnxruntime-node` into the
bundled transformers runtime.

OpenAgents keeps the Rampart guard API but bypasses that broken direct loader in
Node-like runtimes:

1. import `@nationaldesignstudio/rampart`;
2. if the caller did not provide `ner`, did not request a worker, and did not
   request `heuristicsOnly`, build a local NER detector;
3. load `@huggingface/transformers` directly with:

   ```ts
   pipeline("token-classification", "nationaldesignstudio/rampart", {
     dtype: "q4",
     device: "cpu",
   })
   ```

4. adapt the returned classifier to Rampart's `TokenClassifier` shape;
5. pass that detector into `rampart.createGuard({ ...options, ner })`.

That means Khala still uses Rampart's detector merge, policy, placeholder table,
`protect`, `protectReply`, `reveal`, and stream reveal API. Only the model load
path is swapped out for Bun.

The adapter intentionally preserves tokenizer binding when exposing
`countTokens` and `tokenize`; those methods rely on tokenizer instance state.

## Modes And Redaction Refs

The service has three modes:

- `rampart_model`: full Rampart guard with contextual NER and deterministic
  recognizers;
- `rampart_heuristics`: Rampart guard with deterministic recognizers only;
- `regex_only`: OpenAgents secret/token regex scrubber only.

Refs:

- `redaction.khala.rampart.pii`: Rampart changed the text;
- `redaction.khala.rampart.full_model_unavailable`: full model initialization
  failed and the service fell back;
- `redaction.khala.rampart.heuristics_unavailable`: heuristics initialization
  failed and the service fell back;
- `redaction.khala.rampart.runtime_failure`: Rampart loaded, but a protection
  call failed;
- `redaction.khala.regex.secret_material`: the OpenAgents regex scrubber changed
  token-shaped text.

The current production behavior is fail-soft:

1. try full Rampart model mode;
2. if full model mode fails, try Rampart heuristics;
3. if Rampart heuristics fail, use regex-only secret scrubbing.

Fail-soft keeps chat usable and preserves API-key/bearer-token redaction even
when model loading fails. It also means contextual PII such as names and street
addresses may be missed in degraded modes. If the product requirement becomes
"never send possible PII when the full model is unavailable", change this to a
fail-closed policy and return a typed setup/blocker message instead of falling
back.

## What Is Redacted

Full model mode can detect contextual labels such as:

- given names and surnames;
- email addresses;
- phone-like contact fields;
- tax IDs, bank accounts, routing numbers, government IDs, passports, driver's
  licenses;
- building numbers and street names;
- city, state, and ZIP labels, although those are kept by default policy.

Rampart heuristics cover deterministic structured patterns such as:

- email addresses;
- SSNs;
- credit cards with validation;
- URLs;
- IP addresses;
- MAC addresses.

OpenAgents regex scrubbing additionally covers:

- `OPENROUTER_API_KEY=...`;
- `sk-or-...` OpenRouter keys;
- `sk-...` API-key-shaped strings;
- `Bearer ...` token-shaped strings.

The OpenAgents regex pass is defense-in-depth for secrets outside Rampart's PII
taxonomy. It is not a general replacement for full Rampart mode.

## What Leaves The Desktop App

In the intended full-model path:

- raw current user message text does not leave the desktop app;
- raw prior user message text does not leave the desktop app during history
  replay;
- raw local tool result text does not leave the desktop app as a provider tool
  message;
- raw assistant text is not kept as provider replay context after the complete
  assistant message is processed;
- placeholders leave the desktop app;
- the placeholder table does not leave the desktop app.

Local transcript rendering can show revealed values because the user is looking
at their own local desktop session.

## Known Caveats

Streaming assistant deltas are still emitted to the local UI as they arrive.
The complete assistant message is scrubbed and then revealed/replaced at message
finalization. This is acceptable for the current local-owner desktop display,
but it is not a strict streaming redaction boundary. If a future surface logs or
exports streaming deltas, the stream must be routed through a redaction-aware
pipeline before emission.

Rampart does not cover every possible private datum. Non-Latin scripts,
adversarial text, indirect identifiers, screenshots, audio, binary uploads,
structured app state, and rare re-identification combinations need separate
controls.

The fallback modes are intentionally degraded. `rampart_heuristics` can miss
names and free-text addresses. `regex_only` mostly protects token-shaped
secrets. Product copy and operator docs should not claim full PII protection
when redaction refs show a degraded mode.

## Testing

Focused package checks:

```sh
bun run typecheck:khala-tools
bun run test:khala-tools
```

Focused desktop checks:

```sh
bun run typecheck:khala-code-desktop
bun run test:khala-code-desktop
```

Repo checks:

```sh
bun run typecheck
bun run test
```

The full-model tests run the real Bun/Rampart path in a child Bun process. That
is deliberate: loading `onnxruntime-node` directly inside `bun test` can make
the test harness crash during native teardown even after assertions pass. The
child process still proves the real app path:

- package test: `loads the Rampart contextual model under Bun and redacts names
  by default`;
- desktop test: `uses the default Rampart model redaction before hosted provider
  requests`.

The desktop test captures the outbound hosted-provider request body and asserts
that:

- the user message contains `[GIVEN_NAME_1] [SURNAME_1]`;
- the user message contains `[EMAIL_1]`;
- the user message contains `[BUILDING_NUMBER_1] [STREET_NAME_1]`;
- the user message keeps `Chicago, IL 60601`;
- the request body does not contain the raw name, email, or street address;
- the local transcript reveals the placeholdered assistant reply.

## Manual Smoke

From the repo root:

```sh
bun --cwd packages/khala-tools --eval '
  import { Effect } from "effect";
  import { makeKhalaPrivacyRedactionService } from "./src/index.ts";

  const redaction = makeKhalaPrivacyRedactionService();
  const result = await Effect.runPromise(redaction.protectUserText(
    "My name is Alice Johnson. Email alice@example.com. I live at 100 Main Street in Chicago, IL 60601.",
  ));
  console.log(JSON.stringify(result, null, 2));
'
```

Expected important fields:

```json
{
  "engine": "@nationaldesignstudio/rampart",
  "mode": "rampart_model",
  "redacted": true
}
```

Expected protected text contains:

```txt
[GIVEN_NAME_1] [SURNAME_1]
[EMAIL_1]
[BUILDING_NUMBER_1] [STREET_NAME_1]
```

Expected protected text does not contain:

```txt
Alice Johnson
alice@example.com
100 Main Street
```

## Development Rules

When changing this layer:

- keep the redaction service in `packages/khala-tools`;
- keep app wiring in `clients/khala-code-desktop`;
- do not scatter direct Rampart imports throughout UI code;
- do not log raw prompts, raw tool results, placeholder tables, or provider
  payloads in tests or docs;
- add focused tests for every new boundary where text crosses into provider
  context;
- preserve the OpenAgents regex scrubber even if Rampart changes upstream;
- update this doc and the Rampart research notes if package versions, model
  revisions, default modes, or fallback policy change.

## Open Follow-Ups

These are not blockers for the current shipped desktop redaction path, but they
are the right next hardening steps:

- add a fail-closed product mode for sessions that require full contextual
  redaction before provider dispatch;
- expose redaction mode/refs in local diagnostics without leaking raw text;
- decide whether streaming assistant deltas should be redaction-gated before
  local emission;
- add separate controls for screenshots, files, binary uploads, and audio;
- add a model artifact mirror policy if OpenAgents hosts the Rampart model
  files directly;
- revisit fallback behavior whenever `@nationaldesignstudio/rampart` publishes
  a Node/Bun CPU fix.
