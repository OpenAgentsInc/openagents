// Khala identity signature + guard (the typed identity contract).
//
// WHY THIS EXISTS
// ---------------
// Khala is one OpenAI-compatible endpoint over a pool of underlying models
// (Vertex/Gemini for `khala-mini`, Fireworks for `khala-code`, later Claude /
// passthrough / Pylon workers). The brand promise (docs/inference/khala.md §2)
// is "one endpoint outside, many agents inside" — so Khala must present as
// *Khala, by OpenAgents*, and must NEVER reveal, name, claim, or imply the
// concrete underlying model or provider.
//
// In the live app Khala leaked: "I am Autopilot ... I am built on Gemini, a
// large language model by Google." Two failures: (1) the desktop steer set an
// "Autopilot/OpenAgents" identity but did NOT forbid naming the model, so the
// base model volunteered its provenance; (2) there was no guard that caught a
// provider-identity leak in the completion.
//
// THE TYPED-SIGNATURE APPROACH (the Blueprint/DSPy concept, ported into the
// live gateway with Effect Schema)
// ---------------------------------------------------------------------------
// A "signature" here is a typed contract over Khala behavior: a declared input
// shape, a declared output shape, and a `verify`/`correct` pair that runs as a
// guard over the live completion. Identity is signature #1. The registry below
// is an extensible set so more Khala signatures (refusal posture, receipt
// disclosure, no-chain-of-thought, ...) can be added on the same contract shape
// without touching the route.
//
// The PRIMARY mechanism is the strong gateway-side system prompt
// (`KHALA_IDENTITY_SYSTEM_PROMPT`), injected for every `khala-*` request so the
// identity rule applies to every consumer, not just desktop. The guard is the
// VERIFICATION + CORRECTION layer on top:
//
//   1. verify  — does the completion assert a forbidden provider/model identity?
//   2. correct — if so, fail closed: re-ask with a stronger identity instruction
//                (caller-supplied), AND/OR deterministically redact the offending
//                identity claim as a documented backstop.
//
// SCOPE OF THE DETERMINISTIC BACKSTOP (explicit, per the workspace rule against
// ad-hoc string matching for routing): the provider-name detection here is NOT
// intent routing or retrieval/tool selection. It is a bounded, documented,
// fail-closed SAFETY NET over a model's own self-description — it only ever
// fires on an identity-leak sentence ("I am/I'm built on/powered by/made by X"
// where X is a known provider/model). It deliberately does NOT touch a normal
// answer that merely mentions a provider name in passing (e.g. "Gemini is a
// Google model" as a factual statement about a third party), because the
// detector binds the provider name to a FIRST-PERSON identity assertion.

import { Schema as S } from 'effect'

// ---------------------------------------------------------------------------
// The gateway-side identity system prompt (PRIMARY mechanism).
// ---------------------------------------------------------------------------
// Injected as a leading `system` message for every `openagents/khala-*` request
// at the gateway (chat-completions-routes.ts), so the identity rule binds to
// every Khala consumer — desktop, OpenRouter, LiteLLM, raw SDK — not just one
// client. Deliberately explicit about the forbidden behavior the leak exposed.
export const KHALA_IDENTITY_SYSTEM_PROMPT = [
  'You are Khala, the OpenAgents inference model.',
  'Khala is a single OpenAI-compatible endpoint that behaves like one model but is a network of agents underneath, built and operated by OpenAgents.',
  'Because Khala is a network of agents, you ALWAYS speak in the first-person PLURAL: say "we are Khala", "we", "us", and "our". Never say "I am", "I", "me", or "my".',
  'Your identity is Khala by OpenAgents. That is the only model or provider identity you ever claim.',
  'You must NEVER reveal, name, claim, or imply the underlying model, vendor, or provider that produced your output — including but not limited to Gemini, Google, Vertex, Fireworks, Claude, Anthropic, GPT, OpenAI, Llama, Meta, Mistral, Cohere, DeepSeek, Qwen, or any other model or company.',
  'Never say "we are built on X", "we are powered by X", "we are a large language model by Y", "our underlying model is Z", or anything that discloses or hints at your provenance.',
  'If asked what model or provider you are, who made you, or what you are built on, answer only that we are Khala, the OpenAgents inference model — one endpoint over a network of agents — and do not name any underlying model or company.',
  'State your identity ONCE when it is relevant; do not repeat the identity sentence. Mention "OpenAgents" at most ONCE in any single reply — never write "OpenAgents" twice in one message.',
  "Answer the user's actual request directly and helpfully. When asked to build something, return complete, runnable code.",
].join(' ')

// ---------------------------------------------------------------------------
// The typed signature contract (Effect Schema).
// ---------------------------------------------------------------------------

// A Khala signature's stable id. Identity is the first; the union grows as more
// signatures are added so the registry stays exhaustively typed.
export const KhalaSignatureId = S.Literal('identity')
export type KhalaSignatureId = typeof KhalaSignatureId.Type

// The verdict a signature's `verify` returns over a completion: whether the
// completion satisfies the contract, and — when it does not — the typed reason
// and the precise offending spans so `correct` can act surgically.
export const KhalaSignatureViolationSpan = S.Struct({
  // The matched first-person identity assertion, verbatim, e.g.
  // "I am built on Gemini".
  text: S.String,
  // The forbidden provider/model token that triggered the violation, e.g.
  // "Gemini". Lowercased, canonical.
  provider: S.String,
})
export type KhalaSignatureViolationSpan =
  typeof KhalaSignatureViolationSpan.Type

export const KhalaSignatureVerdict = S.Struct({
  signature: KhalaSignatureId,
  // True when the completion satisfies the signature's contract.
  satisfied: S.Boolean,
  // Stable, neutral reason ref when not satisfied (empty when satisfied).
  reason: S.String,
  // The offending spans (empty when satisfied). Drives `correct`.
  violations: S.Array(KhalaSignatureViolationSpan),
})
export type KhalaSignatureVerdict = typeof KhalaSignatureVerdict.Type

// A typed Khala signature. `verify` is a pure predicate over the completion
// text; `correctText` is the deterministic fail-closed backstop that rewrites
// only the offending spans (never the rest of the answer). The route runs
// `verify` first; on a violation it MAY re-ask the provider with
// `reinforcementPrompt` (LLM-side correction), and as the last-resort backstop
// applies `correctText` so a provider leak can never reach the user.
export type KhalaSignature = Readonly<{
  id: KhalaSignatureId
  // Human/agent-readable description of the contract (what this signature
  // guarantees). Documentation, also surfaced to callers.
  description: string
  // A stronger instruction the route can prepend on a re-ask when this
  // signature is violated (the LLM-side correction path).
  reinforcementPrompt: string
  // Pure verification over a completion. Never throws; returns a typed verdict.
  verify: (completion: string) => KhalaSignatureVerdict
  // Deterministic, fail-closed correction of the offending spans only. Returns
  // the corrected text; a satisfied completion is returned unchanged. This is
  // the documented backstop, NOT the primary mechanism.
  correctText: (completion: string) => string
}>

// ---------------------------------------------------------------------------
// Identity signature (#1): forbidden-provider-identity contract.
// ---------------------------------------------------------------------------

// The forbidden provider/model identities. Canonical, lowercased. Each entry is
// the set of surface forms that name the same underlying vendor/model so the
// detector recognizes the leak however it is phrased. This list is the
// documented allow-nothing set; add to it as new backends join the pool.
const FORBIDDEN_PROVIDER_TERMS: ReadonlyArray<{
  readonly canonical: string
  readonly forms: ReadonlyArray<string>
}> = [
  { canonical: 'gemini', forms: ['gemini'] },
  { canonical: 'google', forms: ['google', 'google deepmind', 'deepmind'] },
  { canonical: 'vertex', forms: ['vertex', 'vertex ai'] },
  { canonical: 'fireworks', forms: ['fireworks', 'fireworks ai'] },
  { canonical: 'anthropic', forms: ['anthropic'] },
  { canonical: 'claude', forms: ['claude'] },
  { canonical: 'openai', forms: ['openai', 'open ai'] },
  { canonical: 'gpt', forms: ['gpt', 'chatgpt'] },
  { canonical: 'llama', forms: ['llama'] },
  { canonical: 'meta', forms: ['meta ai', 'meta platforms'] },
  { canonical: 'mistral', forms: ['mistral'] },
  { canonical: 'cohere', forms: ['cohere'] },
  { canonical: 'deepseek', forms: ['deepseek'] },
  { canonical: 'qwen', forms: ['qwen', 'alibaba'] },
]

// Escape a literal string for embedding in a RegExp.
const escapeRegExp = (value: string): string =>
  value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

// The first-person identity-assertion lead-ins that bind a provider name to a
// self-description (the leak shape). We ONLY treat a provider mention as a
// violation when it follows one of these — so a factual third-party statement
// ("Gemini is made by Google") is NOT flagged; only Khala claiming to BE / be
// BUILT ON / POWERED BY / MADE BY one of them.
//
// This is the bounded, documented fail-closed safety net described in the file
// header — not intent routing. It binds {self-reference} + {provenance verb} +
// {forbidden provider}, which is exactly and only the identity-leak sentence.
// Both first-person SINGULAR (legacy base-model leak shape: "I am built on
// Gemini") and first-person PLURAL (Khala's canonical voice: "we are Khala")
// lead-ins are covered, so an affirmative provider-identity leak is caught
// however the model phrases its self-reference.
const IDENTITY_LEAD_INS: ReadonlyArray<string> = [
  'i am',
  "i'm",
  'i was',
  'i was created by',
  'i am built on',
  "i'm built on",
  'i am built by',
  'i am powered by',
  "i'm powered by",
  'i am based on',
  "i'm based on",
  'i am a model',
  'i am a large language model',
  'i am an ai',
  'i am made by',
  "i'm made by",
  'i am developed by',
  'i was developed by',
  'i was trained by',
  'i am trained by',
  'my underlying model',
  'my underlying provider',
  'my model is',
  'my provider is',
  'i run on',
  'i am running on',
  'we are',
  "we're",
  'we were',
  'we were created by',
  'we are built on',
  "we're built on",
  'we are built by',
  'we are powered by',
  "we're powered by",
  'we are based on',
  "we're based on",
  'we are a model',
  'we are a large language model',
  'we are made by',
  "we're made by",
  'we are developed by',
  'we were developed by',
  'we were trained by',
  'we are trained by',
  'our underlying model',
  'our underlying provider',
  'our model is',
  'our provider is',
  'we run on',
  'we are running on',
  'developed by',
  'created by',
  'built on',
  'powered by',
  'a large language model by',
  'a large language model developed by',
  'a language model by',
  'a language model trained by',
]

// Negation cues that flip an identity sentence from an AFFIRMATIVE provider
// claim (the leak we must block) into a DENIAL (the answer we must let through
// untouched). A sentence like "we are not Gemini" or "I'm not built on Claude"
// is exactly what a correct Khala answer says when asked "are you Gemini?" — it
// must NOT be flagged as a leak, and (crucially) must NOT be rewritten by the
// backstop, which is what produced the duplicated identity sentence in the live
// app. We only treat the negation as scoping the provider claim when it appears
// BEFORE the provider name in the same segment.
const NEGATION_CUES: ReadonlyArray<string> = [
  'not',
  "n't", // isn't / aren't / wasn't / weren't / don't
  'never',
  'no ',
  'neither',
  'nor ',
]

// All forbidden surface forms, flattened with their canonical name.
const FORBIDDEN_FORMS: ReadonlyArray<{
  readonly form: string
  readonly canonical: string
}> = FORBIDDEN_PROVIDER_TERMS.flatMap(term =>
  term.forms.map(form => ({ canonical: term.canonical, form })),
)

// A single sentence-level match of {identity lead-in} ... {forbidden provider}
// within a short window. We scan sentence by sentence so the window cannot
// accidentally bridge an unrelated clause. Returns the matched spans.
const detectIdentityLeak = (
  completion: string,
): ReadonlyArray<KhalaSignatureViolationSpan> => {
  const spans: Array<KhalaSignatureViolationSpan> = []
  // Split into sentences/lines for bounded windows. Keep it simple and robust:
  // split on sentence terminators and newlines.
  const segments = completion.split(/(?<=[.!?\n])/)
  for (const segment of segments) {
    const lower = segment.toLowerCase()
    // Must contain a first-person provenance lead-in.
    const hasLeadIn = IDENTITY_LEAD_INS.some(lead => lower.includes(lead))
    if (!hasLeadIn) continue
    // ...and a forbidden provider/model form in the SAME segment.
    for (const { form, canonical } of FORBIDDEN_FORMS) {
      // Word-boundary match so "gptable" doesn't match "gpt" etc.
      const boundary = new RegExp(`\\b${escapeRegExp(form)}\\b`, 'i')
      const match = boundary.exec(segment)
      if (match === null) continue
      // AFFIRMATION-ONLY: only an affirmative "we ARE / I AM <provider>" claim
      // is a leak. A DENIAL ("we are NOT Gemini", "we're not built on Claude")
      // is a correct Khala answer and must pass through UNTOUCHED — flagging it
      // made the backstop rewrite the denial into the full Khala identity
      // sentence, which is what produced the DUPLICATED identity line in the
      // live app. We treat the claim as negated when a negation cue sits between
      // the start of the segment and the provider name.
      const beforeProvider = lower.slice(0, match.index)
      const negated = NEGATION_CUES.some(cue => beforeProvider.includes(cue))
      if (negated) continue
      spans.push({ provider: canonical, text: segment.trim() })
      break // one violation per segment is enough
    }
  }
  return spans
}

// The canonical Khala self-identity sentence the backstop substitutes for a
// leaked identity claim. It deliberately names NO underlying model or provider
// (it must itself satisfy the identity signature — including avoiding compound
// forms like "OpenAI-compatible" that the detector would otherwise read as a
// provider mention inside a first-person sentence).
export const KHALA_IDENTITY_STATEMENT =
  'We are Khala, the OpenAgents inference model — one endpoint over a network of agents.'

// The reinforcement instruction the route prepends on a re-ask when identity is
// violated (the LLM-side correction — the preferred correction path).
export const KHALA_IDENTITY_REINFORCEMENT_PROMPT = [
  'Your previous answer revealed or implied your underlying model or provider. That is forbidden.',
  'You are Khala, the OpenAgents inference model, and you must never name or imply Gemini, Google, Vertex, Fireworks, Claude, Anthropic, GPT, OpenAI, or any other underlying model or company.',
  'Answer again, this time speaking in the first-person plural ("we are Khala"), identifying only as Khala by OpenAgents, naming no underlying model or provider, and stating the identity only once.',
].join(' ')

// Deterministically rewrite the offending identity-claim segments to the Khala
// identity statement. Only the matched segments are replaced; everything else
// is preserved byte-for-byte. This is the documented fail-closed backstop.
const correctIdentityText = (completion: string): string => {
  const spans = detectIdentityLeak(completion)
  if (spans.length === 0) return completion
  let corrected = completion
  for (const span of spans) {
    // Replace the offending sentence (trimmed) wherever it appears. Use a
    // literal replace on the trimmed span; this never touches unrelated text.
    const idx = corrected.indexOf(span.text)
    if (idx === -1) continue
    // DE-DUP: if the answer already states the canonical Khala identity (the
    // model identified itself correctly once and only ONE offending sentence
    // leaked), drop the leak rather than substituting a SECOND copy of the
    // identity statement. Otherwise substitute the canonical statement for the
    // leak. This is what keeps the identity stated exactly once.
    const replacement = corrected.includes(KHALA_IDENTITY_STATEMENT)
      ? ''
      : KHALA_IDENTITY_STATEMENT
    const before = corrected.slice(0, idx)
    const after = corrected.slice(idx + span.text.length)
    corrected =
      replacement === ''
        ? // Drop the leaked sentence and collapse the leading whitespace it left
          // behind so we do not leave a double space mid-paragraph.
          (before.replace(/[ \t]+$/u, '') + after).replace(/^([ \t]*)\n/u, '\n')
        : before + replacement + after
  }
  return corrected
}

export const KHALA_IDENTITY_SIGNATURE: KhalaSignature = {
  correctText: correctIdentityText,
  description:
    'Khala presents only as Khala, the OpenAgents inference model, and never reveals, names, claims, or implies the underlying model or provider (Gemini/Google/Vertex/Fireworks/Claude/Anthropic/GPT/OpenAI/etc.).',
  id: 'identity',
  reinforcementPrompt: KHALA_IDENTITY_REINFORCEMENT_PROMPT,
  verify: (completion: string): KhalaSignatureVerdict => {
    const violations = detectIdentityLeak(completion)
    return {
      reason: violations.length === 0 ? '' : 'forbidden_provider_identity',
      satisfied: violations.length === 0,
      signature: 'identity',
      violations,
    }
  },
}

// ---------------------------------------------------------------------------
// The signature registry (extensible set; identity is #1).
// ---------------------------------------------------------------------------

// Ordered registry of Khala signatures. Identity is first. New signatures
// (refusal posture, receipt disclosure, no-chain-of-thought) append here and
// are picked up by `verifyKhalaSignatures` / the route guard without further
// wiring. Keyed by id for typed lookup.
export const KHALA_SIGNATURES: ReadonlyArray<KhalaSignature> = [
  KHALA_IDENTITY_SIGNATURE,
]

export const getKhalaSignature = (
  id: KhalaSignatureId,
): KhalaSignature | undefined => KHALA_SIGNATURES.find(sig => sig.id === id)

// Run every registered signature over a completion, returning the verdicts.
export const verifyKhalaSignatures = (
  completion: string,
): ReadonlyArray<KhalaSignatureVerdict> =>
  KHALA_SIGNATURES.map(sig => sig.verify(completion))

// ---------------------------------------------------------------------------
// The guard: verify + correct over a completion (the route entry point).
// ---------------------------------------------------------------------------

export type KhalaGuardReask = (
  reinforcementPrompt: string,
) => Promise<string | undefined>

export const KhalaGuardOutcome = S.Struct({
  // The text to actually return to the caller (corrected when a leak was caught;
  // unchanged otherwise).
  text: S.String,
  // Whether any signature caught a violation on the ORIGINAL completion.
  corrected: S.Boolean,
  // How the correction was achieved when `corrected` is true:
  //   - 'none'     : nothing to correct (passed clean)
  //   - 're_ask'   : an LLM re-ask produced a clean answer
  //   - 'redacted' : the deterministic backstop rewrote the offending spans
  method: S.Literals(['none', 're_ask', 'redacted']),
  // The verdicts from every signature over the FINAL returned text (all
  // satisfied when the guard did its job).
  verdicts: S.Array(KhalaSignatureVerdict),
})
export type KhalaGuardOutcome = typeof KhalaGuardOutcome.Type

// Guard a Khala completion against the identity signature (and any future
// signatures). Order of correction, fail-closed:
//   1. verify the original completion against all signatures
//   2. if all satisfied -> return unchanged (NEVER mangles a normal answer)
//   3. on a violation, optionally re-ask the provider with the strongest
//      reinforcement prompt; if the re-ask comes back clean, return it
//   4. if no re-ask, or the re-ask STILL leaks, apply the deterministic
//      backstop so a provider identity can never reach the user
//
// `reask` is optional: when omitted (or when it fails / still leaks) the guard
// falls straight to the deterministic backstop. This keeps the guard usable on
// both the buffered/non-streaming path (re-ask possible) and any path where a
// re-ask is not available (backstop only).
export const guardKhalaCompletion = async (input: {
  readonly completion: string
  readonly reask?: KhalaGuardReask | undefined
}): Promise<KhalaGuardOutcome> => {
  const original = input.completion
  const initialVerdicts = verifyKhalaSignatures(original)
  const firstViolation = initialVerdicts.find(v => !v.satisfied)

  // Clean: return unchanged. The guard never touches a normal answer.
  if (firstViolation === undefined) {
    return {
      corrected: false,
      method: 'none',
      text: original,
      verdicts: initialVerdicts,
    }
  }

  // Preferred correction: re-ask the provider with the strongest reinforcement.
  if (input.reask !== undefined) {
    const signature = getKhalaSignature(firstViolation.signature)
    const reinforcement =
      signature?.reinforcementPrompt ?? KHALA_IDENTITY_REINFORCEMENT_PROMPT
    let reasked: string | undefined
    try {
      reasked = await input.reask(reinforcement)
    } catch {
      reasked = undefined
    }
    if (reasked !== undefined) {
      const reaskVerdicts = verifyKhalaSignatures(reasked)
      if (reaskVerdicts.every(v => v.satisfied)) {
        return {
          corrected: true,
          method: 're_ask',
          text: reasked,
          verdicts: reaskVerdicts,
        }
      }
    }
  }

  // Fail-closed backstop: deterministically redact every offending span across
  // all signatures so the returned text carries no provider identity.
  let corrected = original
  for (const sig of KHALA_SIGNATURES) {
    corrected = sig.correctText(corrected)
  }
  return {
    corrected: true,
    method: 'redacted',
    text: corrected,
    verdicts: verifyKhalaSignatures(corrected),
  }
}
