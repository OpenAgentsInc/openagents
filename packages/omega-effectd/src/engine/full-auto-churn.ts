import { Schema } from "effect"

/**
 * HANDS-4 (#9175): value-aware no-progress detection. The existing
 * no-progress gate (full-auto-reconcile.ts `detectFullAutoNoProgress`) only
 * catches MACHINE failure -- a trailing run of `failed`/`interrupted_by_restart`
 * turns. A turn that self-reports `completed` always counts as progress, even
 * when it repeats the same low-value work. The full-auto-run-analyzer.ts
 * header explicitly flags the missing piece: a typed per-turn action taxonomy
 * (recon/setup/edit/verify) instead of dispatch phase + disposition only.
 *
 * This module adds that taxonomy and a churn detector over it. Churn is
 * "repeated near-identical, non-advancing `completed` turns": each turn claims
 * success, but the per-turn action signature does not change and no plan step
 * advances. The detector is deterministic and pure over already-bounded action
 * rows; it never reads transcript text. When it trips, the caller pauses the
 * run with a typed `low_value_churn` reason (opt-in, autonomy runs only), so
 * waste stops before it accumulates.
 */
export const FULL_AUTO_TURN_ACTION_SCHEMA = "openagents.desktop.full_auto_turn_action.v1" as const

/** Default: three consecutive near-identical non-advancing completed turns. */
export const FULL_AUTO_CHURN_TURN_THRESHOLD = 3
export const FULL_AUTO_TURN_ACTION_SIGNATURE_LIMIT = 200

/**
 * The typed per-turn action taxonomy the analyzer's header names as the
 * missing upstream field:
 *  - `recon`  -- read-only reconnaissance (no file change, no verify run).
 *  - `setup`  -- environment/scaffolding work that changed no product file.
 *  - `edit`   -- a real content change (files were modified).
 *  - `verify` -- a verification/check was executed this turn.
 *  - `other`  -- anything not classifiable from structured signals.
 */
export const FullAutoTurnActionKindSchema = Schema.Literals(["recon", "setup", "edit", "verify", "other"])
export type FullAutoTurnActionKind = typeof FullAutoTurnActionKindSchema.Type

const Ref = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(180))

export const FullAutoTurnActionSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_TURN_ACTION_SCHEMA),
  turnRef: Ref,
  kind: FullAutoTurnActionKindSchema,
  /** A bounded, stable digest of what the turn did (sorted changed paths +
   * kind + optional result hint). Two turns with the same signature did the
   * same observable work. Never raw transcript. */
  signature: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_TURN_ACTION_SIGNATURE_LIMIT)),
  /** Whether this turn advanced a plan step to a terminal status. A turn that
   * advanced the plan is progress regardless of its signature. */
  advancedPlanStep: Schema.Boolean,
  at: Schema.String,
})
export type FullAutoTurnAction = typeof FullAutoTurnActionSchema.Type

const decodeFullAutoTurnAction = Schema.decodeUnknownSync(FullAutoTurnActionSchema)

// -----------------------------------------------------------------------
// Classification + signature -- deterministic over STRUCTURED signals only.
// -----------------------------------------------------------------------

export type FullAutoTurnActionSignals = Readonly<{
  turnRef: string
  /** Bounded set of workspace-relative paths the turn changed (may be empty). */
  changedPaths?: ReadonlyArray<string>
  /** Whether the turn executed a verification/test/check. */
  verificationRan?: boolean
  /** Whether the turn only read (no mutation, no command execution). */
  readOnly?: boolean
  /** Whether this turn advanced a plan step to terminal. */
  advancedPlanStep?: boolean
  /** Optional short, bounded, public-safe hint (e.g. a step ref) that
   * distinguishes otherwise-identical actions. Never transcript. */
  resultHint?: string
  at: string
}>

export const FULL_AUTO_CHANGED_PATHS_MAX = 50
export const FULL_AUTO_CHANGED_PATH_LIMIT = 256

/**
 * HANDS-4 (#9175): extract the bounded set of workspace-relative paths a turn
 * reports it changed, from STRUCTURED markers only (bounded-field parsing, not
 * NLP). Recognized shapes:
 *  - a line:  CHANGED: <path>  or  CHANGED-PATH: <path>
 *  - a fence: ```changed-paths\n<path>\n<path>\n```
 * Whitespace-only or over-long entries are ignored; the result is de-duplicated
 * and bounded. Feeding these into the action signature makes churn detection
 * changed-paths aware: two `completed` turns that touch DIFFERENT files get
 * distinct signatures and never look like churn, while repeated no-op turns
 * (no changed paths, same output) still collapse to one churning signature.
 */
export const parseFullAutoChangedPaths = (text: string): ReadonlyArray<string> => {
  const paths: Array<string> = []
  const push = (raw: string): void => {
    const value = raw.trim()
    if (value.length > 0 && value.length <= FULL_AUTO_CHANGED_PATH_LIMIT && !/\s/.test(value)) paths.push(value)
  }
  const fenced = /```changed-paths[ \t]*\r?\n([\s\S]*?)```/gi
  for (let match = fenced.exec(text); match !== null; match = fenced.exec(text)) {
    for (const line of (match[1] ?? "").split(/\r?\n/)) push(line)
  }
  for (const rawLine of text.split(/\r?\n/)) {
    const match = /^[ \t>*-]*changed(?:-path)?:[ \t]*(.+?)[ \t]*$/i.exec(rawLine)
    if (match?.[1] !== undefined) push(match[1])
  }
  return [...new Set(paths)].slice(0, FULL_AUTO_CHANGED_PATHS_MAX)
}

/** Deterministic classification from structured signals -- never NLP over
 * provider prose. Precedence: verify > edit > recon > setup/other. */
export const classifyFullAutoTurnActionKind = (signals: FullAutoTurnActionSignals): FullAutoTurnActionKind => {
  if (signals.verificationRan === true) return "verify"
  if ((signals.changedPaths?.length ?? 0) > 0) return "edit"
  if (signals.readOnly === true) return "recon"
  return "setup"
}

const stableSignature = (kind: FullAutoTurnActionKind, signals: FullAutoTurnActionSignals): string => {
  const paths = [...(signals.changedPaths ?? [])].sort().join(",")
  const parts = [
    kind,
    paths.length > 0 ? `paths=${paths}` : "paths=∅",
    signals.verificationRan === true ? "verify=1" : "verify=0",
    signals.resultHint !== undefined && signals.resultHint.length > 0 ? `hint=${signals.resultHint}` : "",
  ].filter((part) => part.length > 0)
  const signature = parts.join("|")
  return signature.length <= FULL_AUTO_TURN_ACTION_SIGNATURE_LIMIT
    ? signature
    : signature.slice(0, FULL_AUTO_TURN_ACTION_SIGNATURE_LIMIT)
}

/** Build a typed, decoded per-turn action from structured signals. */
export const buildFullAutoTurnAction = (signals: FullAutoTurnActionSignals): FullAutoTurnAction => {
  const kind = classifyFullAutoTurnActionKind(signals)
  return decodeFullAutoTurnAction({
    schema: FULL_AUTO_TURN_ACTION_SCHEMA,
    turnRef: signals.turnRef,
    kind,
    signature: stableSignature(kind, signals),
    advancedPlanStep: signals.advancedPlanStep ?? false,
    at: signals.at,
  })
}

// -----------------------------------------------------------------------
// Churn detection.
// -----------------------------------------------------------------------

export type FullAutoChurnDecision = Readonly<{
  churn: boolean
  /** Length of the trailing run of near-identical, non-advancing completed
   * turns (0 when the most recent turn advanced the plan or is distinct). */
  consecutive: number
  /** The repeated action signature, when churn was detected. */
  signature: string | null
}>

/**
 * Detect low-value churn over an ORDERED list of per-turn actions (each row
 * corresponds to a `completed` turn; the caller filters non-completed turns
 * out before calling, because a failed turn is machine no-progress, handled by
 * the existing gate). Counts the trailing run of turns that:
 *   - did NOT advance a plan step, AND
 *   - share the exact signature of the most recent action.
 * Churn fires when that run reaches the threshold. An advancing turn or a
 * distinct signature resets the count -- so genuinely varied work, or work
 * that moves the plan, never trips the gate.
 */
export const detectFullAutoChurn = (
  input: Readonly<{
    actions: ReadonlyArray<FullAutoTurnAction>
    /** Only consider actions at/after this anchor (the run's lastResumedAt ??
     * enabledAt), so pre-grant/pre-resume history cannot pause a fresh loop. */
    anchorAt?: string | null
    threshold?: number
  }>,
): FullAutoChurnDecision => {
  const threshold = input.threshold ?? FULL_AUTO_CHURN_TURN_THRESHOLD
  const actions = input.actions
    .filter((action) => input.anchorAt === undefined || input.anchorAt === null || action.at > input.anchorAt)
    .toSorted((left, right) => left.at.localeCompare(right.at))
  if (actions.length === 0) return { churn: false, consecutive: 0, signature: null }
  const last = actions[actions.length - 1]!
  if (last.advancedPlanStep) return { churn: false, consecutive: 0, signature: null }
  let consecutive = 0
  for (let index = actions.length - 1; index >= 0; index -= 1) {
    const action = actions[index]!
    if (action.advancedPlanStep || action.signature !== last.signature) break
    consecutive += 1
  }
  return {
    churn: consecutive >= threshold,
    consecutive,
    signature: consecutive >= threshold ? last.signature : null,
  }
}

/** The typed pause reason a churn decision produces. */
export const fullAutoChurnPauseReason = (decision: FullAutoChurnDecision): string =>
  `low_value_churn:${decision.consecutive}_near_identical_completed_turns`
