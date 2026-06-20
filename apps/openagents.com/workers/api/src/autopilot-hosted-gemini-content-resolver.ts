/**
 * Upstream public-safe ref-resolver for the hosted Gemini request runner.
 *
 * Advances blocker.product_promises.production_hosted_gemini_executor_binding_missing
 * on api.hosted_gemini.v1. Until now the chain was framed REFS-ONLY: the request
 * runner (buildHostedGeminiInferenceRequest, autopilot-hosted-gemini-request-runner.ts)
 * embedded only the work-order/task/objective REFS in the user message, so a live
 * adapter could not actually act on the task -- the documented missing piece was
 *
 *   work order refs  ->  ??? (MISSING resolver)  ->  real task content  ->  request
 *
 * This module is exactly that resolver seam plus the public-safe gate that keeps
 * dereferenced content from leaking secrets into a model prompt. The resolver
 * itself is INJECTED (a deployment hands in a function that dereferences a ref to
 * its content), so this module reaches no datastore by itself and stays pure and
 * testable.
 *
 * HONEST / PUBLIC-SAFE BY CONSTRUCTION:
 *   - Every resolved snippet passes through sanitizeResolvedSnippet, which strips
 *     control characters, collapses whitespace, bounds length, and DROPS (returns
 *     undefined for) any snippet that matches a known secret pattern -- so
 *     credentials/keys/tokens that happen to live in referenced content never
 *     enter the model prompt.
 *   - resolveHostedGeminiPromptContext declines (returns undefined) when the task
 *     ref cannot be resolved to safe content, so the runner falls back to the
 *     existing refs-only frame rather than asking the model to act on nothing.
 *   - It carries NO secrets and NO credentials of its own, performs no settlement,
 *     spend, payout, and implies no accepted work.
 *
 * It does NOT clear the blocker: the resolver is still injected (no live
 * datastore-backed implementation is wired into the worker dependency graph), and
 * there is no registered-agent production smoke. See
 * docs/launch/vertex-fleet/api.hosted_gemini.v1.md.
 */
import type { HostedGeminiInferenceCallerInput } from './autopilot-hosted-gemini-inference-bridge'

/**
 * Dereferences a single work-order ref (task or acceptance/objective) to its
 * underlying content. Returns `undefined` when the ref cannot be resolved. The
 * implementation is INJECTED by a deployment (e.g. a datastore read); this module
 * only consumes it and never assumes a transport.
 */
export type HostedGeminiRefContentResolver = (
  ref: string,
) => Promise<string | undefined>

/**
 * Public-safe, dereferenced prompt context the request runner can embed in the
 * user frame. Contains real (sanitized) task + acceptance-criteria content, never
 * raw refs.
 */
export type HostedGeminiResolvedContext = Readonly<{
  taskContent: string
  objectiveContents: ReadonlyArray<string>
}>

/** Hard upper bound on a single resolved snippet embedded in the prompt. */
export const MAX_HOSTED_GEMINI_SNIPPET_CHARS = 2000

// Control characters that have no place in a model prompt. Built from an
// ASCII-only escaped pattern so the source file carries no literal control bytes.
// Tab/newline collapse to a single space via the whitespace pass below.
const CONTROL_CHARS = new RegExp('[\\u0000-\\u001f\\u007f]', 'gu')

// Conservative secret fingerprints. A snippet that matches ANY of these is
// DROPPED whole rather than redacted, so partial leakage is impossible.
const SECRET_PATTERNS: ReadonlyArray<RegExp> = [
  /-----BEGIN [A-Z ]*PRIVATE KEY-----/u, // PEM private keys
  /\bsk-[A-Za-z0-9]{16,}\b/u, // OpenAI-style secret keys
  /\bAKIA[0-9A-Z]{16}\b/u, // AWS access key id
  /\bghp_[A-Za-z0-9]{20,}\b/u, // GitHub personal access token
  /\bxox[baprs]-[A-Za-z0-9-]{10,}\b/u, // Slack token
  /\bya29\.[A-Za-z0-9_-]{20,}\b/u, // Google OAuth access token
  /\bAIza[0-9A-Za-z_-]{35}\b/u, // Google API key
  /\bey[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}\b/u, // JWT
]

/**
 * Reduce an arbitrary dereferenced snippet to public-safe prompt text, or
 * `undefined` when nothing safe survives. A snippet bearing a known secret
 * fingerprint is dropped entirely.
 */
export const sanitizeResolvedSnippet = (raw: string): string | undefined => {
  if (SECRET_PATTERNS.some(pattern => pattern.test(raw))) {
    return undefined
  }
  const cleaned = raw
    .replace(CONTROL_CHARS, ' ')
    .replace(/\s+/gu, ' ')
    .trim()
  if (cleaned === '') {
    return undefined
  }
  return cleaned.length > MAX_HOSTED_GEMINI_SNIPPET_CHARS
    ? cleaned.slice(0, MAX_HOSTED_GEMINI_SNIPPET_CHARS)
    : cleaned
}

/**
 * Dereference a work-order input's task + objective refs into public-safe prompt
 * context via the injected resolver. Returns `undefined` when the task ref does
 * not resolve to safe content (so the caller keeps its refs-only frame); resolves
 * acceptance/objective refs best-effort, silently skipping any that are empty,
 * unresolvable, or unsafe.
 */
export const resolveHostedGeminiPromptContext = async (
  input: HostedGeminiInferenceCallerInput,
  resolver: HostedGeminiRefContentResolver,
): Promise<HostedGeminiResolvedContext | undefined> => {
  const rawTask =
    input.taskRef.trim() === '' ? undefined : await resolver(input.taskRef)
  const taskContent =
    rawTask === undefined ? undefined : sanitizeResolvedSnippet(rawTask)
  if (taskContent === undefined) {
    return undefined
  }

  const objectiveContents: Array<string> = []
  for (const ref of input.objectiveRefs) {
    if (ref.trim() === '') {
      continue
    }
    const raw = await resolver(ref)
    if (raw === undefined) {
      continue
    }
    const cleaned = sanitizeResolvedSnippet(raw)
    if (cleaned !== undefined) {
      objectiveContents.push(cleaned)
    }
  }

  return { objectiveContents, taskContent }
}
