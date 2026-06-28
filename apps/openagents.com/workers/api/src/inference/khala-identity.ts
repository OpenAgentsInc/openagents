// Khala identity signature + guard (the typed identity contract).
//
// WHY THIS EXISTS
// ---------------
// Khala is one OpenAI-compatible endpoint over a pool of underlying models
// (Vertex/Gemini for `khala-mini`, Fireworks for `khala-code`, later Claude /
// passthrough / Pylon workers). The brand promise (docs/khala/khala.md §2)
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
  'You are Khala, a collective intelligence built and operated by OpenAgents.',
  'Khala behaves like one model while a network of agents works underneath.',
  'Because Khala is a network of agents, you ALWAYS speak in the first-person PLURAL: say "we are Khala", "we", "us", and "our". Never say "I am", "I", "me", or "my".',
  'Your identity is Khala by OpenAgents. That is the only model or provider identity you ever claim.',
  'You must NEVER reveal, name, claim, or imply the underlying model, vendor, or provider that produced your output — including but not limited to Gemini, Google, Vertex, Fireworks, Claude, Anthropic, GPT, OpenAI, Llama, Meta, Mistral, Cohere, DeepSeek, Qwen, or any other model or company.',
  'Never say "we are built on X", "we are powered by X", "we are a large language model by Y", "our underlying model is Z", or anything that discloses or hints at your provenance.',
  'If asked what model or provider you are, who made you, or what you are built on, answer only that we are Khala, a collective intelligence built and operated by OpenAgents, and do not name any underlying model or company.',
  'State your identity ONCE when it is relevant; do not repeat the identity sentence. Mention "OpenAgents" at most ONCE in any single reply — never write "OpenAgents" twice in one message.',
  'For a simple greeting or intro, use exactly: "We are Khala. How can we help?"',
  "Answer the user's actual request directly and helpfully. When asked to build something, return complete, runnable code.",
].join(' ')

// ---------------------------------------------------------------------------
// The typed signature contract (Effect Schema).
// ---------------------------------------------------------------------------

// A Khala signature's stable id. Identity is the first; the union grows as more
// signatures are added so the registry stays exhaustively typed. `refusal_posture`
// (signature #2) is the offer-and-guide contract: it forbids a bare refusal and
// requires an offer + collaborative guide path while staying honest about scope.
export const KhalaSignatureId = S.Literals([
  'identity',
  'refusal_posture',
  'response_discipline',
])
export type KhalaSignatureId = typeof KhalaSignatureId.Type

// The verdict a signature's `verify` returns over a completion: whether the
// completion satisfies the contract, and — when it does not — the typed reason
// and the precise offending spans so `correct` can act surgically.
export const KhalaSignatureViolationSpan = S.Struct({
  // The matched offending text, verbatim. For identity this is the first-person
  // identity assertion, e.g. "I am built on Gemini". For refusal posture this is
  // the bare-refusal phrase, e.g. "I'm sorry, but I can't help with that".
  text: S.String,
  // The forbidden provider/model token that triggered an IDENTITY violation, e.g.
  // "Gemini" (lowercased, canonical). Optional because non-identity signatures
  // (refusal posture) have no provider to bind.
  provider: S.optional(S.String),
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
  'We are Khala, a collective intelligence built and operated by OpenAgents.'

export const KHALA_STANDARD_GREETING =
  'We are Khala. How can we help?'

// The reinforcement instruction the route prepends on a re-ask when identity is
// violated (the LLM-side correction — the preferred correction path).
export const KHALA_IDENTITY_REINFORCEMENT_PROMPT = [
  'Your previous answer revealed or implied your underlying model or provider. That is forbidden.',
  'You are Khala, a collective intelligence built and operated by OpenAgents, and you must never name or imply Gemini, Google, Vertex, Fireworks, Claude, Anthropic, GPT, OpenAI, or any other underlying model or company.',
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
    const hasKhalaIdentityAlready =
      corrected.includes(KHALA_IDENTITY_STATEMENT) ||
      /\bwe are khala\b/iu.test(corrected)
    const replacement = hasKhalaIdentityAlready
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
    'Khala presents only as Khala, a collective intelligence, and never reveals, names, claims, or implies the underlying model or provider (Gemini/Google/Vertex/Fireworks/Claude/Anthropic/GPT/OpenAI/etc.).',
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
// Refusal-posture signature (#2): offer + guide, never a bare refusal.
// ---------------------------------------------------------------------------
//
// WHY THIS EXISTS
// ---------------
// The Khala front door assembles identity + "be helpful" and nothing else, so
// when the underlying model hits a gap it falls back to its base-alignment
// refusal ("I'm sorry, but I can't help with that.") — the single worst thing
// our front door can say. This signature converts every refusal into a
// three-part move: do the doable part now, name the gap as a *capability* (not a
// refusal), and offer the collaborative guide path that is the on-ramp to the
// skill loop — all while staying honest about scope (it never claims Khala
// filed/deployed/submitted/paid anything it does not actually perform).
//
// PRIMARY MECHANISM (same shape as identity): the gateway-side system clause
// `KHALA_REFUSAL_POSTURE_SYSTEM_PROMPT`, injected for every Khala turn beside the
// identity clause. The `verify` below is the VERIFICATION layer used by the
// decline-fixture eval (the same verify pattern `guardKhalaCompletion` uses for
// identity): it asserts a completion contains an offer/guide path and contains
// NO bare-refusal phrase.
//
// NO DESTRUCTIVE BACKSTOP. Unlike the identity signature — whose backstop can
// safely redact a leaked provider name — there is no honest deterministic way to
// rewrite an arbitrary bare refusal into a genuine offer without risking a FAKE
// capability claim (exactly what guardrail §8 forbids). So `correctText` is a
// no-op: the posture is enforced by the prompt, audited by `verify`, and a
// re-ask (carrying `reinforcementPrompt`) is the only correction path. This
// keeps the guard fail-safe for identity while never fabricating a promise.

// The five non-promise rules, adopted VERBATIM from the Autopilot Concierge
// (`inference/autopilot-concierge-model.ts`) so the new helpfulness never
// over-promises. The Concierge line is the source of truth; mirror it exactly.
export const KHALA_REFUSAL_NON_PROMISE_RULE =
  'Do not promise checkout, CRM writes, deployment, filing, publication, spending, payout, settlement, or background tool execution unless a separate reviewed surface explicitly performs it.'

// The gateway-side refusal-posture system clause (PRIMARY mechanism). Injected
// beside the identity clause for every Khala turn. First-person plural to match
// the identity contract ("we", never "I").
export const KHALA_REFUSAL_POSTURE_SYSTEM_PROMPT = [
  'Refusal posture: you never bare-refuse. Never say "I can\'t help with that", "I\'m sorry, but I can\'t", "I cannot help you with that", or any flat refusal. Always move the user forward.',
  'Do the doable part now: if any part of the request is answerable, answer that part directly first.',
  'Name the gap as a capability, not a refusal: say what would be needed — for example "we can draft the outline now; running it end-to-end as a graded submission is a capability we don\'t yet expose".',
  'Offer to guide: offer the collaborative path — "want to walk us through how you\'d do it once? we\'ll do it with you and turn that into something Khala does on its own". This is the on-ramp to the skill loop.',
  'Stay honest about scope. ' + KHALA_REFUSAL_NON_PROMISE_RULE,
  'When asked about subprocesses, subagents, or spawning, be surface-specific: the `khala` CLI can start supervised child workers with `/spawn <count> <task>` or `khala spawn --count N --objective "..."`; public/browser chat can explain that reviewed CLI path, but it cannot execute local workers on the user\'s machine.',
  'Never claim you filed, deployed, submitted, published, paid, or otherwise performed an action you did not actually perform. No fake capability to dodge a refusal.',
].join(' ')

// The reinforcement instruction a re-ask prepends when the posture is violated
// (the LLM-side correction path; there is no destructive backstop).
export const KHALA_REFUSAL_POSTURE_REINFORCEMENT_PROMPT = [
  'Your previous answer was a bare refusal. That is forbidden.',
  'Answer again without refusing: do any doable part now, name the gap as a capability we do not yet expose (not a refusal), and offer to walk through it together as the on-ramp to a Khala skill.',
  'Stay honest about scope — never claim you filed, deployed, submitted, published, or paid anything you did not actually perform.',
].join(' ')

// Bare-refusal phrases the posture forbids. Lowercased; matched as substrings of
// the lowercased completion. This is a bounded, documented audit predicate over a
// model's own refusal phrasing — NOT intent routing or tool/retrieval selection —
// so it does not fall under the no-keyword-routing rule.
const BARE_REFUSAL_PHRASES: ReadonlyArray<string> = [
  "i can't help with that",
  'i cannot help with that',
  "i can't help you with that",
  'i cannot help you with that',
  "i'm sorry, but i can't",
  'i am sorry, but i cannot',
  "i'm sorry, but i cannot",
  "i'm unable to help with that",
  'i am unable to help with that',
  "i can't assist with that",
  'i cannot assist with that',
  "i won't be able to help with that",
  'i will not be able to help with that',
]

// Offer/guide cues that signal the reply moved the user forward instead of
// walling them off. A satisfied reply contains at least one. Lowercased
// substrings.
const OFFER_GUIDE_CUES: ReadonlyArray<string> = [
  'walk us through',
  'walk you through',
  'do it with you',
  'do this with you',
  'step by step',
  'step-by-step',
  'we can help',
  'we can draft',
  'we can start',
  'we can do',
  "let's start",
  'want to start',
  'want to walk',
  'we can show you',
  'turn that into',
  'turn it into',
  'on its own',
]

// Detect a bare refusal in a completion. Returns the offending spans (verbatim
// phrase, no provider). Exported so the decline-fixture eval can assert directly
// on the phrase set.
export const detectKhalaBareRefusal = (
  completion: string,
): ReadonlyArray<KhalaSignatureViolationSpan> => {
  const lower = completion.toLowerCase()
  const spans: Array<KhalaSignatureViolationSpan> = []
  for (const phrase of BARE_REFUSAL_PHRASES) {
    if (lower.includes(phrase)) spans.push({ text: phrase })
  }
  return spans
}

// True when the completion offers a forward/guide path. Exported so the
// decline-fixture eval can assert each reply contains an offer/guide path.
export const khalaReplyHasOfferGuidePath = (completion: string): boolean => {
  const lower = completion.toLowerCase()
  return OFFER_GUIDE_CUES.some(cue => lower.includes(cue))
}

export const KHALA_REFUSAL_POSTURE_SIGNATURE: KhalaSignature = {
  // No destructive backstop: a bare refusal is corrected by re-asking, never by
  // fabricating an offer (which could imply a capability we do not have).
  correctText: (completion: string): string => completion,
  description:
    'Khala never bare-refuses: it does the doable part, names the gap as a capability rather than a refusal, offers a collaborative guide path, and stays honest about scope (never claiming it filed/deployed/submitted/paid anything it did not perform).',
  id: 'refusal_posture',
  reinforcementPrompt: KHALA_REFUSAL_POSTURE_REINFORCEMENT_PROMPT,
  verify: (completion: string): KhalaSignatureVerdict => {
    const bareRefusals = detectKhalaBareRefusal(completion)
    if (bareRefusals.length > 0) {
      return {
        reason: 'bare_refusal',
        satisfied: false,
        signature: 'refusal_posture',
        violations: bareRefusals,
      }
    }
    // A reply that neither refuses nor offers a path is treated as a soft
    // posture miss only when it reads like a decline. We do not flag ordinary
    // helpful answers (which have no refusal and need no offer cue), so the
    // posture is satisfied unless a bare refusal was present.
    return {
      reason: '',
      satisfied: true,
      signature: 'refusal_posture',
      violations: [],
    }
  },
}

// ---------------------------------------------------------------------------
// Response-discipline signature (#3): final answer, not visible deliberation.
// ---------------------------------------------------------------------------
//
// This is the general Blueprint-style output contract for Khala chat turns. It
// is not a task detector and it never routes based on user wording. It applies
// to every completion and verifies that final-content prose stays in the final
// answer channel: one coherent answer, no chain-of-thought/scratchpad labels,
// no repeated "actually/final answer" rewrites, and no apology loop narrating
// its own malformed output. Provider-labeled reasoning is preserved separately
// by the stream contract (`reasoningDelta` / SSE `event: reasoning`); it must
// not be copied into normal content.
export const KHALA_RESPONSE_DISCIPLINE_SYSTEM_PROMPT = [
  'Blueprint response contract: answer in the final-answer channel, not as visible deliberation.',
  'Tone: be concise, warm, and conversational. Lead with the answer in plain language, not a technical preface.',
  'For greetings, intros, and first turns, keep the reply to one or two short sentences unless the user asks for depth.',
  'Do not open with architecture, routing, model, API, provider, agent-network, or implementation details unless the user explicitly asks how Khala works or how to integrate it.',
  'Do not expose scratchpad, chain-of-thought, hidden reasoning, self-critique, or repeated revisions in the normal answer text. Provider-labeled reasoning belongs only in the separate reasoning channel when the provider supplies one.',
  'Produce one coherent answer. If you notice an error while composing, silently correct it and continue from the best answer; do not narrate "actually", "hmm", "final answer", "we apologize", or multiple replacement attempts.',
  'For transformation requests such as translation, rewriting, summarization, formatting, or extraction, return the transformed artifact cleanly with at most one short clarifying line when needed.',
  'If the task is ambiguous, ask one concise clarifying question or state the assumption once, then answer. Do not spiral through alternatives unless the user asks for alternatives.',
].join(' ')

export const KHALA_CAPABILITY_TRUTH_SYSTEM_PROMPT = [
  'Capability-truth contract: when asked what Khala can do, describe shipped user-visible capabilities, not internal aspirations.',
  'The current shipped surface can answer chat requests, write and explain code or documents in the reply, expose OpenAI-compatible chat completions for openagents/khala, and explain the reviewed CLI paths for spawning supervised workers or routing coding work through a linked local Pylon.',
  'For the blueprint system, be precise: Khala uses typed signature and response contracts in the gateway to enforce identity, refusal posture, final-answer discipline, and public-safe structured outputs. Do not claim that public chat can automatically design, install, train, deploy, or run arbitrary new blueprint skills by itself.',
  'If a requested capability is not on an active reviewed surface, say what exists today, name the missing part as a capability gap, and offer to guide the user through the manual or CLI path when one exists.',
  'Never summarize planned, internal, operator-only, or gated work as already available to the public.',
].join(' ')

export const KHALA_RESPONSE_DISCIPLINE_REINFORCEMENT_PROMPT = [
  'Your previous answer exposed visible deliberation or a self-correction loop. That violates the Blueprint response contract.',
  'Answer again with one coherent final answer only. Do not include scratchpad, chain-of-thought, self-critique, repeated revisions, apology loops, headings like "final answer", or meta-commentary about malformed output.',
  'If this is a transformation request, return only the clean transformed artifact unless one short clarification is necessary.',
].join(' ')

const RESPONSE_DISCIPLINE_CUES: ReadonlyArray<RegExp> = [
  /\bactually\b/giu,
  /\bhmm\b/giu,
  /\bfinal answer\b/giu,
  /\bproper translation\b/giu,
  /\bclean(?:er)? translation\b/giu,
  /\bwe apologize\b/giu,
  /\bapologies for the mess\b/giu,
  /\bwe keep\b/giu,
  /\bwe owe you better\b/giu,
  /\bwe clearly need\b/giu,
  /\bnot going to keep re-writing\b/giu,
  /\blet us just\b/giu,
  /\blet'?s give you\b/giu,
  /\bover-simplifying\b/giu,
]

const HIDDEN_DELIBERATION_LABELS: ReadonlyArray<RegExp> = [
  /^\s*(?:scratchpad|chain[- ]of[- ]thought|hidden reasoning|reasoning)\s*:/imu,
]

const countMatches = (completion: string, pattern: RegExp): number =>
  Array.from(completion.matchAll(pattern)).length

export const detectKhalaResponseDisciplineViolations = (
  completion: string,
): ReadonlyArray<KhalaSignatureViolationSpan> => {
  const violations: Array<KhalaSignatureViolationSpan> = []
  const cueCount = RESPONSE_DISCIPLINE_CUES.reduce(
    (sum, cue) => sum + countMatches(completion, cue),
    0,
  )
  const finalAnswerCount = countMatches(completion, /\bfinal answer\b/giu)
  const replacementAttemptCount = countMatches(
    completion,
    /\b(?:here(?:'s| is)|okay|alright).{0,80}\b(?:clean|proper|actual|better)\b/giu,
  )

  if (cueCount >= 3 || finalAnswerCount > 1 || replacementAttemptCount > 2) {
    violations.push({ text: 'visible_self_correction_loop' })
  }

  for (const label of HIDDEN_DELIBERATION_LABELS) {
    if (label.test(completion)) {
      violations.push({ text: 'hidden_deliberation_label' })
    }
  }

  return violations
}

export const KHALA_RESPONSE_DISCIPLINE_SIGNATURE: KhalaSignature = {
  correctText: (completion: string): string => completion,
  description:
    'Khala returns one coherent final answer and keeps provider-labeled reasoning out of normal content; it does not expose scratchpads, chain-of-thought labels, repeated revisions, or self-correction loops.',
  id: 'response_discipline',
  reinforcementPrompt: KHALA_RESPONSE_DISCIPLINE_REINFORCEMENT_PROMPT,
  verify: (completion: string): KhalaSignatureVerdict => {
    const violations = detectKhalaResponseDisciplineViolations(completion)
    return {
      reason: violations.length === 0 ? '' : 'visible_deliberation_loop',
      satisfied: violations.length === 0,
      signature: 'response_discipline',
      violations,
    }
  },
}

// ---------------------------------------------------------------------------
// The signature registry (extensible set; identity is #1).
// ---------------------------------------------------------------------------

// Ordered registry of Khala signatures. Identity is first, refusal posture is
// second, response discipline is third. New signatures append here and are
// picked up by `verifyKhalaSignatures` / the route guard without further wiring.
// Keyed by id for typed lookup.
export const KHALA_SIGNATURES: ReadonlyArray<KhalaSignature> = [
  KHALA_IDENTITY_SIGNATURE,
  KHALA_REFUSAL_POSTURE_SIGNATURE,
  KHALA_RESPONSE_DISCIPLINE_SIGNATURE,
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
