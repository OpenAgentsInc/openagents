import { createHash } from "node:crypto"

import { Effect, Schema as S } from "effect"

import {
  BaselinePointer,
  CandidateArtifact,
  ReleasedArtifactPointer,
  UncertaintyRecord,
  beginShadow,
  resolveActivation,
  resolveReleasedArtifact,
  type CompiledProgram,
  type ReleaseChannel,
} from "@openagentsinc/dse"

import {
  APPLE_FM_PROMPT_MAX_CHARS,
  renderAppleFmEnvironmentContext,
  type AppleFmEnvironmentContext,
  type AppleFmPromptTurn,
} from "../apple-fm-prompt.ts"
import {
  HONEST_CHAT_BASELINE,
  HONEST_CHAT_POINTER,
  HONEST_CHAT_UNCERTAINTY,
  HONEST_CHAT_WINNER,
  TURN_ROUTE_BASELINE,
  TURN_ROUTE_POINTER,
  TURN_ROUTE_UNCERTAINTY,
  TURN_ROUTE_WINNER,
} from "./artifacts.generated.ts"

/**
 * AFS-09 runtime release channels for the compiled Apple FM signatures.
 *
 * The checked-in released bytes are decoded and verified here, and every channel
 * opens in SHADOW mode. In shadow the hand-written baseline is served and the
 * compiled artifact causes NO dispatch or user-visible substitution — so
 * introducing the compiled-artifact path changes no live behavior. A later
 * canary or promotion (an explicit owner decision) is the only thing that serves
 * the compiled prompt, and a rollback restores the baseline WITHOUT an
 * application rebuild.
 *
 * This module resolves and verifies released bytes only; it holds no compile,
 * promotion, dispatch, or provider authority.
 */

const decodeCandidate = S.decodeUnknownSync(CandidateArtifact)
const decodePointer = S.decodeUnknownSync(ReleasedArtifactPointer)
const decodeBaseline = S.decodeUnknownSync(BaselinePointer)
const decodeUncertainty = S.decodeUnknownSync(UncertaintyRecord)

/** The pure hasher for canary membership; the app never leaks the request key. */
const sha256Hex = (text: string): string => createHash("sha256").update(text, "utf8").digest("hex")

/** One compiled Apple FM signature: its shadow channel and verified released bytes. */
export interface AppleFmDseRelease {
  readonly signatureId: string
  readonly channel: ReleaseChannel
  readonly candidate: CandidateArtifact
  readonly pointer: ReleasedArtifactPointer
  readonly baseline: BaselinePointer
  readonly uncertainty: UncertaintyRecord
}

const makeRelease = (bytes: {
  readonly winner: unknown
  readonly pointer: unknown
  readonly baseline: unknown
  readonly uncertainty: unknown
}): AppleFmDseRelease => {
  const candidate = decodeCandidate(bytes.winner)
  const pointer = decodePointer(bytes.pointer)
  const baseline = decodeBaseline(bytes.baseline)
  const uncertainty = decodeUncertainty(bytes.uncertainty)
  const shadow = beginShadow({
    signatureId: candidate.signatureId,
    baseline,
    candidate: pointer,
    now: () => pointer.releasedAt,
  })
  if (!shadow.ok) throw new Error(`AFS-09 release channel refused shadow: ${shadow.reason}`)
  return { signatureId: candidate.signatureId, channel: shadow.channel, candidate, pointer, baseline, uncertainty }
}

/** The honest-answer release (AFS-09 first release), shadowed by default. */
export const honestChatRelease: AppleFmDseRelease = makeRelease({
  winner: HONEST_CHAT_WINNER,
  pointer: HONEST_CHAT_POINTER,
  baseline: HONEST_CHAT_BASELINE,
  uncertainty: HONEST_CHAT_UNCERTAINTY,
})

/** The turn-route release, shadowed by default. */
export const turnRouteRelease: AppleFmDseRelease = makeRelease({
  winner: TURN_ROUTE_WINNER,
  pointer: TURN_ROUTE_POINTER,
  baseline: TURN_ROUTE_BASELINE,
  uncertainty: TURN_ROUTE_UNCERTAINTY,
})

/** The checked-in Apple FM releases, keyed by signature id. */
export const APPLE_FM_DSE_RELEASES: ReadonlyArray<AppleFmDseRelease> = [
  honestChatRelease,
  turnRouteRelease,
]

/** Which prompt a turn should serve: the hand-written baseline or a compiled program. */
export type AppleFmPromptPlan =
  | { readonly kind: "baseline" }
  | { readonly kind: "compiled"; readonly program: CompiledProgram }

/**
 * Resolve which prompt a request serves for one release. In SHADOW (and
 * ROLLED_BACK) this always returns the baseline. In CANARY a bounded
 * deterministic population resolves the compiled program; in ACTIVE every
 * request does. A released artifact that fails offline verification falls back
 * to the baseline (fail closed) — a corrupted release never serves a bad prompt.
 */
export const resolveAppleFmPromptPlan = (args: {
  readonly release: AppleFmDseRelease
  readonly requestKey: string
}): AppleFmPromptPlan => {
  const decision = resolveActivation({ channel: args.release.channel, requestKey: args.requestKey, sha256: sha256Hex })
  if (decision.serve === "baseline") return { kind: "baseline" }
  const resolved = Effect.runSyncExit(
    resolveReleasedArtifact({
      pointer: args.release.pointer,
      candidateBytes: args.release.candidate,
      expectedSignatureId: args.release.signatureId,
    }),
  )
  return resolved._tag === "Success"
    ? { kind: "compiled", program: resolved.value.program }
    : { kind: "baseline" }
}

/**
 * Assemble a flattened Apple FM prompt from a compiled program's honesty-bounded
 * preamble (system, instruction, and tool policy) plus the same bounded history
 * window the hand-written builder uses. The compiled preamble REPLACES the
 * hand-written one; the history flattening and the frozen char bound are
 * unchanged. The host-owned ambient `environment` context (if any) is appended
 * to the compiled preamble with the SAME renderer/tripwire as the baseline path,
 * so a promoted compiled prompt still answers environment/identity questions.
 */
export const buildCompiledAppleFmPrompt = (
  program: CompiledProgram,
  turns: ReadonlyArray<AppleFmPromptTurn>,
  environment?: AppleFmEnvironmentContext,
  maxChars: number = APPLE_FM_PROMPT_MAX_CHARS,
): string => {
  const preamble =
    [program.promptIr.system, program.promptIr.instruction, program.promptIr.toolPolicy]
      .map((block) => block.trim())
      .filter((block) => block.length > 0)
      .join("\n\n") + renderAppleFmEnvironmentContext(environment)
  const lines = turns
    .map((turn) => {
      const text = turn.text.trim()
      return text === "" ? null : `${turn.role === "assistant" ? "Assistant" : "User"}: ${text}`
    })
    .filter((line): line is string => line !== null)
  const assemble = (rows: ReadonlyArray<string>): string => `${preamble}\n\n${rows.join("\n")}\nAssistant:`
  let kept = lines
  while (kept.length > 1 && assemble(kept).length > maxChars) kept = kept.slice(1)
  const prompt = assemble(kept.length > 0 ? kept : ["User:"])
  return prompt.length > maxChars ? prompt.slice(0, maxChars) : prompt
}
