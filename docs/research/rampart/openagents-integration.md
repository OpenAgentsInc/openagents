# OpenAgents Integration Notes

Rampart fits best as an owner-local privacy prefilter, not as an authority
system. It can reduce accidental disclosure in chat-like paths, but OpenAgents
must still enforce typed boundaries for traces, secrets, receipts, payments,
proofs, deployment authority, and public claims.

## Candidate Surfaces

Good candidates:

- Browser or desktop chat composer text before a remote LLM call.
- Owner-local Pylon or Khala flows before provider dispatch, as long as the
  placeholder table stays local.
- Client-side preview of what would be sent to a model or support tool.
- Log and trace scrubber defense in depth before writing user-entered text.

Poor candidates:

- Server-authoritative safety gates.
- Payment, wallet, settlement, payout, or receipt decisions.
- Public product-promise evidence or accepted-work authority.
- Adversarial content moderation.
- Any flow where non-Latin-script names or government IDs are central and there
  is no compensating detector.

## Boundary Model

The OpenAgents boundary should be:

1. Raw user text starts in the browser or owner-local client.
2. Rampart runs locally and returns placeholdered text.
3. Only placeholdered text is sent to models, logs, analytics, traces, or remote
   storage.
4. The placeholder table remains local and scoped to one conversation.
5. Assistant output is scrubbed before persistence and rehydrated only for the
   user-visible local render.

Never send the Rampart session table to `openagents.com`, Pylon assignment
rows, public traces, Forum posts, product-promise reports, or analytics.

## Product And Invariant Fit

This repo's invariant ledgers already prohibit committing secrets, raw prompts,
private repo content, provider payloads, and private customer data into docs,
tests, fixtures, logs, or public projections. Rampart can be one implementation
tool for that posture, but it does not change the invariant.

Any production integration should model the output as typed redaction evidence,
for example:

```ts
type RedactionEvidence = {
  readonly engine: "rampart";
  readonly engineVersion: string;
  readonly modelId: "nationaldesignstudio/rampart";
  readonly modelRevision: string;
  readonly mode: "full" | "heuristics-only";
  readonly placeholders: readonly string[];
  readonly keptLabels: readonly string[];
};
```

Do not route user-facing intent selection, CRM/database query routing,
retrieval routing, or tool selection through keyword checks added for Rampart.
If a flow needs routing, keep using a central typed selector, embedding search,
structured planner, or modeled parser as the repo guidance requires.

## Suggested OpenAgents Wrapper

Put Rampart behind an OpenAgents-owned interface instead of importing it
directly throughout UI code. That gives us one place to pin versions, add
OpenAgents-specific redactors, and record redaction evidence.

```ts
import { createGuard, type ChatGuard } from "@nationaldesignstudio/rampart";

export type PrivacyGuard = {
  protectUserText(text: string): Promise<{
    text: string;
    placeholders: readonly string[];
  }>;
  protectModelText(text: string): Promise<string>;
  revealForLocalUser(text: string): string;
  revealTransform(): TransformStream<string, string>;
};

export async function createPrivacyGuard(): Promise<PrivacyGuard> {
  const guard: ChatGuard = await createGuard({
    device: "wasm",
    keepLabels: ["CITY", "STATE", "ZIP_CODE"],
  });

  return {
    async protectUserText(text) {
      const protectedText = await guard.protect(text);
      return {
        text: protectedText.text,
        placeholders: protectedText.placeholders,
      };
    },
    async protectModelText(text) {
      return (await guard.protectReply(text)).text;
    },
    revealForLocalUser(text) {
      return guard.reveal(text);
    },
    revealTransform() {
      return guard.revealTransform();
    },
  };
}
```

Production code should add a deterministic OpenAgents redaction pass before or
after Rampart for values outside Rampart's taxonomy:

- OpenAgents agent tokens and bearer tokens.
- API keys and provider credentials.
- Wallet mnemonics, payment tokens, Lightning invoices, and macaroon-like
  strings.
- Local filesystem paths when traces can cross a public/private boundary.
- Internal assignment refs or owner-only IDs if a surface is public.

Keep those recognizers centralized and typed; do not scatter one-off regexes
through feature code.

## Version Pinning

If this becomes product code, pin all of the following in the implementation
note or package lock:

- `@nationaldesignstudio/rampart` version.
- `@huggingface/transformers` version.
- Hugging Face model revision.
- Whether the artifact is loaded from Hugging Face or an OpenAgents-hosted
  mirror.
- Device backend policy, including WebGPU fallback behavior.
- Keep-label policy.

The researched package was `@nationaldesignstudio/rampart@0.1.2`; the observed
Hugging Face revision was `bc423a63513f9ee88a1fc27ad0711ea741431351`.

## Acceptance Checklist

Before shipping:

- Add unit tests for the OpenAgents wrapper with `heuristicsOnly` or injected
  fake `ner` so tests do not download the model.
- Add one manual or browser smoke that loads the real model and verifies a name,
  SSN, email, address, city/state/ZIP, and a model-output scrub.
- Capture latency and cold-load measurements on the target surface.
- Confirm no raw input is persisted in logs, traces, analytics, request bodies,
  or error reports before redaction completes.
- Confirm the placeholder table is conversation-scoped and destroyed when the
  conversation or local session ends.
- Confirm non-Latin and government-ID failure modes have product handling:
  stronger fallback, explicit warning, opt-in, or scoped non-support.
- Confirm CC BY 4.0 attribution appears wherever OpenAgents mirrors or
  redistributes the model files.
