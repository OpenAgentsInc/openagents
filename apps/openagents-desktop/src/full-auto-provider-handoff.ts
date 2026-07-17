import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import type { DesktopThread } from "./chat-contract.ts"
import { PROVIDER_SWITCH_HISTORY_CHARS, PROVIDER_SWITCH_HISTORY_MESSAGES, projectBoundedHistory } from "./provider-lane-registry.ts"
import { FullAutoRunActorSchema, type FullAutoRun, type FullAutoRunActor } from "./full-auto-run-registry.ts"

/**
 * FA-HO-01 (#8975): the host-owned, objective-priority `ProviderHandoffEnvelope`
 * for a manual same-thread cross-provider (Codex<->Claude) switch, and the
 * durable `ProviderHandoffTransitionRecord` receipt every switch appends.
 *
 * Hard boundary this module exists to enforce (per the issue and ProductSpec
 * FA-AC-60): the envelope is ALWAYS assembled from trusted host-owned state
 * (the FullAutoRun's objective/doneCondition, and the existing bounded
 * host-visible thread-history projection from provider-lane-registry.ts) --
 * never from provider-private session state, credentials, hidden reasoning,
 * or raw environment. Every envelope carries an explicit
 * `provider_private_never_transferred` omission entry so no caller can ever
 * present this as "the target provider received the source provider's
 * session" -- only a host-owned bounded projection of Desktop-visible
 * history transfers.
 *
 * `run` is optional so the SAME envelope builder serves both callers named by
 * FA-AC-58: a Full Auto run's Pause -> switch provider -> Resume sequence
 * (run present; objective/doneCondition/stateRevision populate) and a plain
 * interactive-chat manual switch with no bound FullAutoRun (run absent;
 * objective/doneCondition are explicitly omitted rather than invented, per
 * the same "never invented, always attributed" discipline FA-AC-38 already
 * applies to run objectives).
 */
export const PROVIDER_HANDOFF_ENVELOPE_SCHEMA = "openagents.desktop.provider_handoff_envelope.v1" as const
export const PROVIDER_HANDOFF_TRANSITION_SCHEMA = "openagents.desktop.provider_handoff_transition.v1" as const
export const PROVIDER_HANDOFF_TRANSITION_LIMIT = 200
export const PROVIDER_HANDOFF_REASON_LIMIT = 400

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))
const LaneRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))
const Reason = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(PROVIDER_HANDOFF_REASON_LIMIT))

export const ProviderHandoffHistoryMessageSchema = Schema.Struct({
  role: Schema.Literals(["user", "assistant"]),
  text: Schema.String,
})
export type ProviderHandoffHistoryMessage = typeof ProviderHandoffHistoryMessageSchema.Type

/** Every field this envelope could not honestly populate is named explicitly
 * -- never silently empty, never fabricated. `provider_private_never_transferred`
 * is present on every envelope (FA-AC-60); `bounded_truncation` fires when the
 * shared 32-message/64,000-char projection cut source history;
 * `not_modeled_yet` marks fields this builder does not yet have a durable
 * source for (verified artifact/commit refs, pending provider questions) so a
 * caller can never mistake "empty array" for "nothing was omitted." */
export const ProviderHandoffOmissionReasonSchema = Schema.Literals([
  "bounded_truncation",
  "not_modeled_yet",
  "provider_private_never_transferred",
  "no_run_bound",
])
export type ProviderHandoffOmissionReason = typeof ProviderHandoffOmissionReasonSchema.Type

export const ProviderHandoffOmissionSchema = Schema.Struct({
  field: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80)),
  reason: ProviderHandoffOmissionReasonSchema,
  detail: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(300)),
})
export type ProviderHandoffOmission = typeof ProviderHandoffOmissionSchema.Type

export const ProviderHandoffEnvelopeSchema = Schema.Struct({
  schema: Schema.Literal(PROVIDER_HANDOFF_ENVELOPE_SCHEMA),
  runRef: Schema.optional(Ref),
  threadRef: Schema.optional(Ref),
  sourceLaneRef: LaneRef,
  targetLaneRef: LaneRef,
  /** Priority channel (FA-AC-49): never crowded out by transcript/tool-note
   * truncation. Absent (not empty-string) when no FullAutoRun is bound. */
  objective: Schema.optional(Schema.String.check(Schema.isMinLength(1))),
  doneCondition: Schema.optional(Schema.String.check(Schema.isMinLength(1))),
  runStateRevision: Schema.optional(Schema.Number),
  /** Ordered recent authored user/assistant context, oldest first -- the
   * exact same bound as the existing manual-switch projection. */
  recentContext: Schema.Array(ProviderHandoffHistoryMessageSchema),
  contextTruncated: Schema.Boolean,
  contextSourceMessageCount: Schema.Number,
  contextIncludedMessageCount: Schema.Number,
  /** Bounded tool/system outcome summary -- kept separate from authored
   * conversation so a target provider never confuses a tool note for
   * something the user or assistant said. */
  toolNoteSummary: Schema.Array(Schema.String),
  artifactRefs: Schema.Array(Schema.String),
  openWorkItems: Schema.Array(Schema.String),
  pendingQuestions: Schema.Array(Schema.String),
  promotedFollowups: Schema.Array(Schema.String),
  interruptionState: Schema.NullOr(Schema.String),
  omissions: Schema.Array(ProviderHandoffOmissionSchema).check(Schema.isMinLength(1)),
  reason: Reason,
  actor: FullAutoRunActorSchema,
  at: Schema.String,
  correlationRef: Schema.optional(Ref),
})
export type ProviderHandoffEnvelope = typeof ProviderHandoffEnvelopeSchema.Type

export type BuildProviderHandoffEnvelopeInput = Readonly<{
  /** The durable FullAutoRun this handoff belongs to, or null for a plain
   * interactive-chat switch with no bound run. */
  run: FullAutoRun | null
  sourceLaneRef: string
  targetLaneRef: string
  thread: DesktopThread | null
  reason: string
  actor: FullAutoRunActor
  at: string
  correlationRef?: string
}>

/**
 * Pure assembly function -- the exhaustive-unit-test surface for FA-AC-49,
 * FA-AC-58, and FA-AC-60. Takes only already-durable host state (never
 * touches a provider adapter, credential, or live session) and returns a
 * fully validated, explicitly-bounded envelope.
 */
export const buildProviderHandoffEnvelope = (
  input: BuildProviderHandoffEnvelopeInput,
): ProviderHandoffEnvelope => {
  const projected = input.thread === null
    ? { history: [], truncated: false }
    : projectBoundedHistory(input.thread)
  const sourceMessageCount = input.thread === null
    ? 0
    : input.thread.notes.filter(note => note.text.trim() !== "").length
  const authored = projected.history.filter(
    (message): message is ProviderHandoffHistoryMessage => message.role === "user" || message.role === "assistant",
  )
  const toolNoteSummary = projected.history
    .filter(message => message.role === "system")
    .map(message => message.text)

  const omissions: Array<ProviderHandoffOmission> = [{
    field: "providerPrivateSessionState",
    reason: "provider_private_never_transferred",
    detail:
      "Only this host-owned bounded projection of Desktop-visible thread history transfers. No " +
      "provider-private session state, credentials, hidden reasoning, raw environment, or " +
      "unsupported native capability is transferred or implied to have transferred.",
  }]
  if (projected.truncated) {
    omissions.push({
      field: "recentContext",
      reason: "bounded_truncation",
      detail:
        `Context is bounded to ${PROVIDER_SWITCH_HISTORY_MESSAGES} messages / ` +
        `${PROVIDER_SWITCH_HISTORY_CHARS} characters; earlier authored and tool/system history was ` +
        "not carried into this envelope.",
    })
  }
  omissions.push({
    field: "artifactRefs",
    reason: "not_modeled_yet",
    detail: "Verified artifact/commit refs have no durable per-run source yet; none are carried over.",
  })
  omissions.push({
    field: "pendingQuestions",
    reason: "not_modeled_yet",
    detail: "Pending provider questions are not sourced by this envelope builder; check the thread directly.",
  })
  if (input.run === null) {
    omissions.push({
      field: "objective",
      reason: "no_run_bound",
      detail: "No FullAutoRun is bound to this thread; objective and done condition are run-level state and do not exist for a plain interactive switch.",
    })
  }

  return Schema.decodeUnknownSync(ProviderHandoffEnvelopeSchema)({
    schema: PROVIDER_HANDOFF_ENVELOPE_SCHEMA,
    ...(input.run?.runRef === undefined ? {} : { runRef: input.run.runRef }),
    ...(input.thread === null ? {} : { threadRef: input.thread.id }),
    sourceLaneRef: input.sourceLaneRef,
    targetLaneRef: input.targetLaneRef,
    ...(input.run === null ? {} : { objective: input.run.objective, doneCondition: input.run.doneCondition, runStateRevision: input.run.stateRevision }),
    recentContext: authored,
    contextTruncated: projected.truncated,
    contextSourceMessageCount: sourceMessageCount,
    contextIncludedMessageCount: projected.history.length,
    toolNoteSummary,
    artifactRefs: [],
    openWorkItems: [],
    pendingQuestions: [],
    promotedFollowups: [],
    interruptionState: null,
    omissions,
    reason: input.reason,
    actor: input.actor,
    at: input.at,
    ...(input.correlationRef === undefined ? {} : { correlationRef: input.correlationRef }),
  })
}

/** The exact disposition vocabulary the issue names verbatim: "complete
 * within bounds", "truncated with confirmation", or "refused". */
export const ProviderHandoffDispositionSchema = Schema.Literals([
  "complete_within_bounds",
  "truncated_with_confirmation",
  "refused",
])
export type ProviderHandoffDisposition = typeof ProviderHandoffDispositionSchema.Type

export const providerHandoffDispositionForEnvelope = (
  envelope: Pick<ProviderHandoffEnvelope, "contextTruncated">,
): ProviderHandoffDisposition => (envelope.contextTruncated ? "truncated_with_confirmation" : "complete_within_bounds")

/** FA-AC-59's refusal vocabulary: reuses `ProviderLaneSwitchRefusal` plus one
 * handoff-specific reason (`run_not_paused`) for the Full-Auto-run path. */
export const ProviderHandoffRefusalReasonSchema = Schema.Literals([
  "unknown_lane",
  "thread_not_found",
  "missing_auth",
  "unadmitted_peer",
  "capability_mismatch",
  "run_not_paused",
])
export type ProviderHandoffRefusalReason = typeof ProviderHandoffRefusalReasonSchema.Type

/** The durable, owner-visible receipt every successful (or refused) handoff
 * appends -- "one durable transcript/report transition event with exact
 * provider identities" (FA-AC-58). Stored independently of `FullAutoRun.transitions`
 * (which carries lifecycle-state edges, not provider-pair edges) so a future
 * FullAutoRunReport (#8972) can list every handoff for a run without
 * conflating the two event kinds. */
export const ProviderHandoffTransitionRecordSchema = Schema.Struct({
  handoffRef: Ref,
  runRef: Schema.optional(Ref),
  threadRef: Schema.optional(Ref),
  from: LaneRef,
  to: LaneRef,
  actor: FullAutoRunActorSchema,
  at: Schema.String,
  reason: Reason,
  disposition: ProviderHandoffDispositionSchema,
  truncated: Schema.Boolean,
  refusalReason: Schema.optional(ProviderHandoffRefusalReasonSchema),
  envelopeSchema: Schema.optional(Schema.Literal(PROVIDER_HANDOFF_ENVELOPE_SCHEMA)),
  correlationRef: Schema.optional(Ref),
})
export type ProviderHandoffTransitionRecord = typeof ProviderHandoffTransitionRecordSchema.Type

const ProviderHandoffRegistryFileSchema = Schema.Struct({
  schema: Schema.Literal(PROVIDER_HANDOFF_TRANSITION_SCHEMA),
  transitions: Schema.Array(ProviderHandoffTransitionRecordSchema),
})

const ensurePrivateParent = (filePath: string): void => {
  const parent = path.dirname(filePath)
  mkdirSync(parent, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(parent, 0o700)
}

const writePrivateAtomic = (filePath: string, value: unknown): void => {
  ensurePrivateParent(filePath)
  const pending = `${filePath}.pending`
  try {
    rmSync(pending, { force: true })
    writeFileSync(pending, `${JSON.stringify(value)}\n`, { encoding: "utf8", mode: 0o600 })
    if (process.platform !== "win32") chmodSync(pending, 0o600)
    renameSync(pending, filePath)
    if (process.platform !== "win32") chmodSync(filePath, 0o600)
  } catch (error) {
    rmSync(pending, { force: true })
    throw new Error(
      `provider handoff registry unavailable: ${error instanceof Error ? error.message : "unknown error"}`,
    )
  }
}

/** Corrupt-file quarantine, matching the FA-H10/FA-AC-41 pattern the other
 * durable Full Auto stores already use -- a corrupt file never crash-loops
 * the app; it is quarantined and the registry starts empty. */
const decodeFile = (filePath: string, now: () => Date): ReadonlyArray<ProviderHandoffTransitionRecord> => {
  if (!existsSync(filePath)) return []
  try {
    const decoded = Schema.decodeUnknownSync(ProviderHandoffRegistryFileSchema)(
      JSON.parse(readFileSync(filePath, "utf8")),
    )
    return decoded.transitions
  } catch (error) {
    const quarantinePath = `${filePath}.quarantined-${now().toISOString()}`
    try {
      renameSync(filePath, quarantinePath)
      console.error(
        `provider handoff registry failed validation; quarantined the corrupt file at ${quarantinePath} and starting empty`,
        error,
      )
    } catch {
      console.error(
        `provider handoff registry failed validation and the corrupt file at ${filePath} could not be quarantined; starting empty`,
        error,
      )
    }
    return []
  }
}

export type ProviderHandoffRecordInput = Omit<ProviderHandoffTransitionRecord, "handoffRef">

export type ProviderHandoffRegistry = Readonly<{
  list: (filter?: Readonly<{ runRef?: string; threadRef?: string }>) => ReadonlyArray<ProviderHandoffTransitionRecord>
  record: (input: ProviderHandoffRecordInput) => ProviderHandoffTransitionRecord
}>

export const openProviderHandoffRegistry = (
  file: string,
  now: () => Date = () => new Date(),
): ProviderHandoffRegistry => {
  const filePath = path.resolve(file)
  let transitions = [...decodeFile(filePath, now)]

  const persist = (): void => {
    const bounded = transitions.slice(-PROVIDER_HANDOFF_TRANSITION_LIMIT)
    transitions = bounded
    writePrivateAtomic(filePath, { schema: PROVIDER_HANDOFF_TRANSITION_SCHEMA, transitions: bounded })
  }

  const mintHandoffRef = (): string => {
    const random = Math.random().toString(36).slice(2, 10)
    return `handoff.provider.${now().getTime().toString(36)}.${random}`
  }

  const list: ProviderHandoffRegistry["list"] = filter => transitions.filter(record =>
    (filter?.runRef === undefined || record.runRef === filter.runRef) &&
    (filter?.threadRef === undefined || record.threadRef === filter.threadRef))

  const record: ProviderHandoffRegistry["record"] = input => {
    const decoded = Schema.decodeUnknownSync(ProviderHandoffTransitionRecordSchema)({
      ...input,
      handoffRef: mintHandoffRef(),
    })
    transitions.push(decoded)
    persist()
    return decoded
  }

  return { list, record }
}
