# Rampart Research Notes

Researched: 2026-06-30

Rampart is National Design Studio's local-first PII redaction system for chat
and intake flows. The Hugging Face artifact at
`nationaldesignstudio/rampart` is the ONNX token-classification model; the
intended product integration is the `@nationaldesignstudio/rampart` TypeScript
runtime, which combines that model with deterministic recognizers, a
default-deny policy layer, and a per-conversation placeholder table.

The practical reading: use the package, not the raw model, unless the goal is
research. The raw classifier does not give OpenAgents the checksum-backed
structured detectors, redaction policy, stable placeholders, streaming reveal,
or model-output scrub hooks.

## Sources

- Hugging Face model: <https://huggingface.co/nationaldesignstudio/rampart>
- Hugging Face model API snapshot observed 2026-06-30:
  <https://huggingface.co/api/models/nationaldesignstudio/rampart>
- Model card:
  <https://huggingface.co/nationaldesignstudio/rampart/raw/main/MODEL_CARD.md>
- Whitepaper:
  <https://huggingface.co/nationaldesignstudio/rampart/raw/main/WHITEPAPER.md>
- NDS launch post:
  <https://ndstudio.gov/posts/say-hello-to-rampart>
- NPM package:
  <https://www.npmjs.com/package/@nationaldesignstudio/rampart>
- Source repository:
  <https://github.com/nationaldesignstudio/rampart>

## Artifact Facts

| Field | Observed value |
| --- | --- |
| Hugging Face model id | `nationaldesignstudio/rampart` |
| HF revision observed | `bc423a63513f9ee88a1fc27ad0711ea741431351` |
| NPM package observed | `@nationaldesignstudio/rampart@0.1.2` |
| GitHub HEAD observed | `c91cafd0308bdc8de93e744db03c5197df4913b3` |
| Pipeline | `token-classification` via `transformers.js` |
| Base architecture | MiniLM-L6-H384-uncased with a BIO token head |
| Model file | `onnx/model_q4.onnx` |
| Runtime shape | ONNX Runtime Web through `@huggingface/transformers` |
| License | CC BY 4.0 |
| Training corpus | AI4Privacy OpenPII 1.5M plus a synthetic reinforcement set |
| Supported languages | English, Spanish, French, German, Italian, Portuguese, Dutch |
| Default kept labels | `CITY`, `STATE`, `ZIP_CODE` |

The model card reports about 18.5M parameters, a 19,730-piece trimmed
WordPiece vocabulary, 512-token maximum sequence length, and a 14.7 MB Q4 ONNX
artifact. The Hugging Face API reported about 14.8 MB used storage for the
model repo.

## What The System Does

Rampart's main contract is client-side harm reduction: redact user-entered
PII before the text reaches a hosted LLM, a backend, analytics, logs, traces,
or crash reporting. It replaces detected values with stable typed placeholders
such as `[GIVEN_NAME_1]` or `[SSN_1]`, sends only placeholdered text downstream,
then rehydrates placeholders locally before rendering model replies to the
user.

The runtime has two layers:

- A deterministic layer for structured PII. In the published `0.1.2` source,
  this validates credit cards with Luhn checks, validates SSNs with structural
  rules, and pattern-matches email addresses, URLs, IPv4, IPv6, and MAC
  addresses.
- A contextual MiniLM token classifier for names, phone numbers, tax IDs, bank
  and routing numbers, government IDs, passports, driver's licenses, street
  components, and coarse geography labels.

Those spans are merged, filtered through a default-deny keep-set, and converted
to placeholders in a conversation-local table.

## Evaluation Snapshot

The model card and package README report full-system numbers, meaning model
plus deterministic layer plus policy:

| Evaluation slice | Private-term recall | Public retention | Notes |
| --- | ---: | ---: | --- |
| All seven supported languages | 98.42% | 91.69% term-presence | 30,000 held-out OpenPII rows |
| English only | 98.85% | 90.5% | 11,569 rows |
| Spanish only | 98.84% | 91.6% | 3,234 rows |
| English plus Spanish | 98.85% | 91.0% | Reported as a useful sub-slice |

Latency reported by the authors is 6.6 ms p50 in Node ONNX over the held-out
set, 3.9 ms p50 in browser WebGPU on Apple Metal, and 12.6 ms p50 in browser
WASM. These are hardware-dependent and should be remeasured in any OpenAgents
surface before treating them as UX facts.

## Important Limitations

Rampart is not a security boundary. It is a small client-side redaction layer
for good-faith user input.

- It still leaked 2,082 of 131,707 private terms on the seven-language
  held-out test in the authors' own report.
- Non-Latin scripts are out of scope in this release. The authors report about
  13.7% aggregate recall on non-Latin-script names in their fairness suite.
- Government-style identifiers have a documented weak spot because many carry
  no checksum. The model card reports about 67.6% model-only recall in a
  structured-ID probe.
- Adversarial text is only partially covered. The reported hostile-input suite
  recall is 86.36%, and combined perturbations can bypass the union of layers.
- Indirect identifiers are out of scope. A rare condition plus a ZIP code can
  re-identify a person even if neither term is individually redacted.
- It handles text. Images, audio, screenshots, binary uploads, and structured
  app state require separate controls.

For OpenAgents, this means Rampart can reduce accidental disclosure before
model calls and logging, but it cannot replace typed data boundaries, secret
redaction, trace scrubbing, access control, or product-promise evidence gates.

## Local Findings From Package Inspection

The published `0.1.2` package exposes:

- `createGuard(options?)`
- `ChatGuard.protect(text)`
- `ChatGuard.reveal(reply)`
- `ChatGuard.revealTransform()`
- `ChatGuard.protectReply(reply)`
- `detectHeuristics`, `detectNer`, `loadNerClassifier`, and lower-level helpers

Runtime options include `model`, `device`, `worker`, `heuristicsOnly`,
`keepLabels`, `aliases`, `ner`, `minScore`, and `noPrefilter`.

One detail to preserve in implementation notes: the Hugging Face model card says
the keep-set is compile-time, but the published NPM package exposes
`keepLabels` as a runtime `createGuard` option. For OpenAgents integration work,
follow the package type surface and verify against the pinned package version.

## OpenAgents Implementation Update

The OpenAgents integration pins `@nationaldesignstudio/rampart@0.1.2`,
`@huggingface/transformers@3.7.5`, and `onnxruntime-node@1.21.0` in
`packages/khala-tools`. The shared package now exports an Effect service that
owns one Rampart guard per caller-created service:

- `makeKhalaPrivacyRedactionService()`
- `KhalaPrivacyRedactionService`
- `KhalaPrivacyRedactionLive`

Khala Code Desktop creates a default redaction service per chat session. It
protects outbound current and previous user text, assistant/history context,
and local tool output before provider requests. Assistant replies are scrubbed
for provider context and then revealed locally before rendering.

The published Rampart package works in this worktree with `heuristicsOnly:
true`. The full Node/Bun CPU model initializer currently fails before inference
because the browser-targeted published bundle does not successfully wire
`onnxruntime-node` into the Node path. The OpenAgents service therefore tries
the configured full guard first, falls back to Rampart heuristics, and finally
falls back to the existing Khala token/API-key scrubber if Rampart itself is
unavailable. Tests pin that fail-soft behavior.

## Recommendation

Use Rampart as an optional owner-local privacy prefilter for chat-like
surfaces, especially where raw user text would otherwise leave the browser or
desktop client before a model call. Do not route server-authoritative safety,
payment, settlement, proof, trace, or promise logic through Rampart as if it
were an authority. Any integration should keep the placeholder table client- or
owner-local and should add OpenAgents-specific deterministic redactors for
secrets, bearer tokens, wallet material, internal IDs, and other workspace
formats that Rampart does not know about.

See [usage.md](./usage.md) for copy-paste setup and
[openagents-integration.md](./openagents-integration.md) for how to fit this
into OpenAgents boundaries.
