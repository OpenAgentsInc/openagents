import { Schema } from "effect"

/**
 * HANDS-3 (#9174): the durable, decomposed Full Auto plan carried across
 * turns. This module is the BACKBONE of the autonomy core (HANDS-1..4): host
 * verification needs a done-condition, churn detection needs a notion of
 * "advancing a step", and objective selection produces the plan a run then
 * executes. The plan is deliberately a pure, self-contained value type plus
 * pure operations over it -- it holds NO dispatch authority. It is stored on
 * the durable `FullAutoRun` record's optional `autonomy` block
 * (full-auto-run-registry.ts), so an existing run without a plan decodes and
 * behaves exactly as before, and the plan only ever influences a turn when the
 * run's autonomy flag is enabled.
 *
 * Design invariants (mirrors the discipline in full-auto-registry.ts):
 *  - Every step carries an explicit status and an explicit dependency set, so
 *    "the next unblocking step" is a deterministic function, never a guess.
 *  - Mutations bump a monotonic `revision`, so drift/coherence tooling can see
 *    that the plan changed without diffing text.
 *  - Reordering never invents or drops a step; it only permutes the same set.
 *  - The mission brief is BOUNDED text (current step + prior-progress summary),
 *    never the whole plan, never raw transcript.
 */
export const FULL_AUTO_PLAN_SCHEMA = "openagents.desktop.full_auto_plan.v1" as const

/** Bounds -- generous enough for a real decomposition, small enough that the
 * durable run record and the mission brief stay bounded. */
export const FULL_AUTO_PLAN_MAX_STEPS = 40
export const FULL_AUTO_PLAN_STEP_TITLE_LIMIT = 200
export const FULL_AUTO_PLAN_STEP_NOTE_LIMIT = 400
export const FULL_AUTO_PLAN_BRIEF_MAX_DONE_TITLES = 8

const StepRef = Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(80))
const Revision = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

/**
 * A step is `pending` until its dependencies clear and it is picked,
 * `in_progress` while a turn works it, `done` when its own work is verified or
 * accepted, `blocked` when an external condition holds it, `skipped` when the
 * run decides it is unnecessary. `done` and `skipped` are the two TERMINAL
 * step statuses (a dependant treats either as satisfied).
 */
export const FullAutoPlanStepStatusSchema = Schema.Literals([
  "pending",
  "in_progress",
  "done",
  "blocked",
  "skipped",
])
export type FullAutoPlanStepStatus = typeof FullAutoPlanStepStatusSchema.Type

export const FULL_AUTO_PLAN_TERMINAL_STEP_STATUSES: ReadonlySet<FullAutoPlanStepStatus> = new Set([
  "done",
  "skipped",
])

export const isFullAutoPlanStepTerminal = (status: FullAutoPlanStepStatus): boolean =>
  FULL_AUTO_PLAN_TERMINAL_STEP_STATUSES.has(status)

export const FullAutoPlanStepSchema = Schema.Struct({
  stepRef: StepRef,
  title: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(FULL_AUTO_PLAN_STEP_TITLE_LIMIT)),
  status: FullAutoPlanStepStatusSchema,
  /** stepRefs that must be terminal (done/skipped) before this step is
   * actionable. Never includes the step's own ref; never a forward-only
   * requirement the validator cannot see. */
  dependsOn: Schema.Array(StepRef).check(Schema.isMaxLength(FULL_AUTO_PLAN_MAX_STEPS)),
  note: Schema.optional(Schema.String.check(Schema.isMaxLength(FULL_AUTO_PLAN_STEP_NOTE_LIMIT))),
})
export type FullAutoPlanStep = typeof FullAutoPlanStepSchema.Type

export const FullAutoPlanSchema = Schema.Struct({
  schema: Schema.Literal(FULL_AUTO_PLAN_SCHEMA),
  steps: Schema.Array(FullAutoPlanStepSchema).check(Schema.isMaxLength(FULL_AUTO_PLAN_MAX_STEPS)),
  revision: Revision,
  createdAt: Schema.String,
  updatedAt: Schema.String,
})
export type FullAutoPlan = typeof FullAutoPlanSchema.Type

export const decodeFullAutoPlan = Schema.decodeUnknownSync(FullAutoPlanSchema)

// -----------------------------------------------------------------------
// Validation -- structural integrity a decode alone cannot express.
// -----------------------------------------------------------------------

export type FullAutoPlanValidationIssue =
  | Readonly<{ kind: "duplicate_step_ref"; stepRef: string }>
  | Readonly<{ kind: "self_dependency"; stepRef: string }>
  | Readonly<{ kind: "unknown_dependency"; stepRef: string; dependsOn: string }>
  | Readonly<{ kind: "dependency_cycle"; stepRefs: ReadonlyArray<string> }>

/**
 * Structural validation over an already-decoded plan: unique step refs, no
 * self-dependency, every dependency resolves to a real step, and the
 * dependency graph is acyclic. Pure -- returns the exact issues, never
 * throws, so a caller can surface a typed reason instead of a crash.
 */
export const validateFullAutoPlan = (
  plan: FullAutoPlan,
): ReadonlyArray<FullAutoPlanValidationIssue> => {
  const issues: Array<FullAutoPlanValidationIssue> = []
  const seen = new Set<string>()
  for (const step of plan.steps) {
    if (seen.has(step.stepRef)) issues.push({ kind: "duplicate_step_ref", stepRef: step.stepRef })
    seen.add(step.stepRef)
  }
  const known = new Set(plan.steps.map((step) => step.stepRef))
  for (const step of plan.steps) {
    for (const dependency of step.dependsOn) {
      if (dependency === step.stepRef) {
        issues.push({ kind: "self_dependency", stepRef: step.stepRef })
      } else if (!known.has(dependency)) {
        issues.push({ kind: "unknown_dependency", stepRef: step.stepRef, dependsOn: dependency })
      }
    }
  }
  // Cycle detection via iterative DFS over the (resolvable) dependency edges.
  const dependencies = new Map(plan.steps.map((step) => [step.stepRef, step.dependsOn.filter((d) => known.has(d) && d !== step.stepRef)]))
  const state = new Map<string, 0 | 1 | 2>() // 0 unvisited, 1 in-stack, 2 done
  const cycleReported = new Set<string>()
  const visit = (start: string): void => {
    const stack: Array<Readonly<{ node: string; index: number }>> = [{ node: start, index: 0 }]
    const path: Array<string> = []
    while (stack.length > 0) {
      const frame = stack[stack.length - 1]!
      if (frame.index === 0) {
        if (state.get(frame.node) === 2) {
          stack.pop()
          continue
        }
        state.set(frame.node, 1)
        path.push(frame.node)
      }
      const edges = dependencies.get(frame.node) ?? []
      if (frame.index < edges.length) {
        const next = edges[frame.index]!
        stack[stack.length - 1] = { node: frame.node, index: frame.index + 1 }
        const nextState = state.get(next)
        if (nextState === 1) {
          const cycleStart = path.indexOf(next)
          const cycle = cycleStart === -1 ? [next] : path.slice(cycleStart)
          const key = [...cycle].sort().join(">")
          if (!cycleReported.has(key)) {
            cycleReported.add(key)
            issues.push({ kind: "dependency_cycle", stepRefs: cycle })
          }
        } else if (nextState !== 2) {
          stack.push({ node: next, index: 0 })
        }
      } else {
        state.set(frame.node, 2)
        path.pop()
        stack.pop()
      }
    }
  }
  for (const step of plan.steps) {
    if (state.get(step.stepRef) !== 2) visit(step.stepRef)
  }
  return issues
}

export const isValidFullAutoPlan = (plan: FullAutoPlan): boolean => validateFullAutoPlan(plan).length === 0

// -----------------------------------------------------------------------
// Construction.
// -----------------------------------------------------------------------

export type FullAutoPlanStepInput = Readonly<{
  stepRef: string
  title: string
  status?: FullAutoPlanStepStatus
  dependsOn?: ReadonlyArray<string>
  note?: string
}>

/**
 * Build and decode a plan from step inputs. Throws (via decode) on a
 * schema/bounds violation; a caller that wants soft handling validates the
 * result with `validateFullAutoPlan`. Steps default to `pending` with no
 * dependencies.
 */
export const makeFullAutoPlan = (
  input: Readonly<{ steps: ReadonlyArray<FullAutoPlanStepInput>; now?: () => Date }>,
): FullAutoPlan => {
  const now = input.now ?? (() => new Date())
  const timestamp = now().toISOString()
  return decodeFullAutoPlan({
    schema: FULL_AUTO_PLAN_SCHEMA,
    steps: input.steps.map((step) => ({
      stepRef: step.stepRef,
      title: step.title,
      status: step.status ?? "pending",
      dependsOn: [...(step.dependsOn ?? [])],
      ...(step.note === undefined ? {} : { note: step.note }),
    })),
    revision: 0,
    createdAt: timestamp,
    updatedAt: timestamp,
  })
}

// -----------------------------------------------------------------------
// Selection -- the next unblocking step (D3 Foresight).
// -----------------------------------------------------------------------

const stepByRef = (plan: FullAutoPlan): ReadonlyMap<string, FullAutoPlanStep> =>
  new Map(plan.steps.map((step) => [step.stepRef, step]))

/** True when every dependency of `step` is terminal (done or skipped). An
 * unknown dependency (should be caught by validation) is treated as NOT
 * satisfied, so a malformed plan fails closed rather than racing ahead. */
export const fullAutoStepDependenciesSatisfied = (
  plan: FullAutoPlan,
  step: FullAutoPlanStep,
): boolean => {
  const byRef = stepByRef(plan)
  return step.dependsOn.every((dependency) => {
    const target = byRef.get(dependency)
    return target !== undefined && isFullAutoPlanStepTerminal(target.status)
  })
}

/**
 * The next actionable step: the FIRST step in plan order that is `pending` (or
 * already `in_progress`) and whose dependencies are all terminal. Returns null
 * when the plan has no runnable step (all terminal, or everything remaining is
 * blocked/waiting on an unsatisfied dependency). An `in_progress` step is
 * preferred over a later `pending` one so a turn resumes the step it started
 * rather than opening a new front.
 */
export const nextActionableFullAutoStep = (plan: FullAutoPlan): FullAutoPlanStep | null => {
  const inProgress = plan.steps.find(
    (step) => step.status === "in_progress" && fullAutoStepDependenciesSatisfied(plan, step),
  )
  if (inProgress !== undefined) return inProgress
  const pending = plan.steps.find(
    (step) => step.status === "pending" && fullAutoStepDependenciesSatisfied(plan, step),
  )
  return pending ?? null
}

// -----------------------------------------------------------------------
// Progress summary + drift.
// -----------------------------------------------------------------------

export type FullAutoPlanProgressSummary = Readonly<{
  total: number
  done: number
  inProgress: number
  blocked: number
  pending: number
  skipped: number
  /** Bounded list of the most recently-ordered completed step titles. */
  doneTitles: ReadonlyArray<string>
  allTerminal: boolean
}>

export const fullAutoPlanProgressSummary = (plan: FullAutoPlan): FullAutoPlanProgressSummary => {
  const count = (status: FullAutoPlanStepStatus): number =>
    plan.steps.filter((step) => step.status === status).length
  const done = count("done")
  const skipped = count("skipped")
  return {
    total: plan.steps.length,
    done,
    inProgress: count("in_progress"),
    blocked: count("blocked"),
    pending: count("pending"),
    skipped,
    doneTitles: plan.steps
      .filter((step) => step.status === "done")
      .map((step) => step.title)
      .slice(-FULL_AUTO_PLAN_BRIEF_MAX_DONE_TITLES),
    allTerminal: plan.steps.length > 0 && plan.steps.every((step) => isFullAutoPlanStepTerminal(step.status)),
  }
}

export type FullAutoPlanDriftSignal =
  | Readonly<{ kind: "structural_invalid"; issues: ReadonlyArray<FullAutoPlanValidationIssue> }>
  | Readonly<{ kind: "all_steps_terminal" }>
  | Readonly<{ kind: "deadlocked"; blockedOrWaiting: number }>
  | Readonly<{ kind: "empty_plan" }>

/**
 * Detect plan drift: the plan no longer describes runnable work that matches
 * reality. Signals are typed, not free text:
 *  - `structural_invalid`: the plan fails validation (cycle/unknown dep/etc).
 *  - `all_steps_terminal`: every step is done/skipped -- the plan is exhausted
 *    and the run should verify+complete or be re-planned, not continue.
 *  - `deadlocked`: work remains but NO step is actionable (everything left is
 *    blocked or waiting on an unsatisfiable dependency).
 *  - `empty_plan`: an autonomy run carries an empty plan.
 * Drift is advisory: it never mutates the plan or the run. The caller decides
 * whether to re-plan, pause, or verify.
 */
export const detectFullAutoPlanDrift = (
  plan: FullAutoPlan,
): Readonly<{ drifted: boolean; signals: ReadonlyArray<FullAutoPlanDriftSignal> }> => {
  const signals: Array<FullAutoPlanDriftSignal> = []
  const issues = validateFullAutoPlan(plan)
  if (issues.length > 0) signals.push({ kind: "structural_invalid", issues })
  if (plan.steps.length === 0) {
    signals.push({ kind: "empty_plan" })
    return { drifted: true, signals }
  }
  const summary = fullAutoPlanProgressSummary(plan)
  if (summary.allTerminal) signals.push({ kind: "all_steps_terminal" })
  else if (nextActionableFullAutoStep(plan) === null) {
    signals.push({ kind: "deadlocked", blockedOrWaiting: summary.blocked + summary.pending + summary.inProgress })
  }
  return { drifted: signals.length > 0, signals }
}

// -----------------------------------------------------------------------
// Mutation -- status change + reorder, each bumping the revision.
// -----------------------------------------------------------------------

const withRevision = (
  plan: FullAutoPlan,
  steps: ReadonlyArray<FullAutoPlanStep>,
  now: () => Date,
): FullAutoPlan =>
  decodeFullAutoPlan({
    schema: FULL_AUTO_PLAN_SCHEMA,
    steps,
    revision: plan.revision + 1,
    createdAt: plan.createdAt,
    updatedAt: now().toISOString(),
  })

/** Set one step's status (and optionally its note). Unknown stepRef is a
 * no-op that returns the SAME plan (no revision bump). */
export const applyFullAutoStepStatus = (
  plan: FullAutoPlan,
  input: Readonly<{ stepRef: string; status: FullAutoPlanStepStatus; note?: string; now?: () => Date }>,
): FullAutoPlan => {
  if (!plan.steps.some((step) => step.stepRef === input.stepRef)) return plan
  const now = input.now ?? (() => new Date())
  const steps = plan.steps.map((step) =>
    step.stepRef === input.stepRef
      ? {
          ...step,
          status: input.status,
          ...(input.note === undefined ? {} : { note: input.note }),
        }
      : step,
  )
  return withRevision(plan, steps, now)
}

// -----------------------------------------------------------------------
// HANDS-3 auto-advancement (#9174): host-side plan-step status advancement
// from a turn's structured output + disposition + host verification result.
// Marker parsing is bounded-field extraction on already-selected text, never
// NLP over provider prose (mirrors deriveFullAutoVerificationSpec's discipline).
// -----------------------------------------------------------------------

export type FullAutoStepMarkers = Readonly<{
  completed: ReadonlyArray<string>
  started: ReadonlyArray<string>
}>

const STEP_REF_BODY = /^[A-Za-z0-9._:-]{1,80}$/
const dedupeStepRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> => [...new Set(refs)]

/**
 * Extract structured plan-step markers a turn may emit to report which step it
 * finished or started. Recognizes exactly these bounded shapes (case-insensitive
 * keyword, the stepRef captured verbatim):
 *  - `STEP-DONE: <stepRef>` or `STEP-DONE(<stepRef>)`
 *  - `STEP-START: <stepRef>` or `STEP-START(<stepRef>)`
 *  - fenced ```step-done\n<stepRef>\n<stepRef>\n``` (one ref per line)
 * A stepRef must match the plan's StepRef shape (bounded, no whitespace) or it
 * is ignored -- this never guesses a step from free text.
 */
export const parseFullAutoStepMarkers = (text: string): FullAutoStepMarkers => {
  const completed: Array<string> = []
  const started: Array<string> = []
  const fenced = /```step-done[ \t]*\r?\n([\s\S]*?)```/gi
  for (let match = fenced.exec(text); match !== null; match = fenced.exec(text)) {
    for (const rawLine of (match[1] ?? "").split(/\r?\n/)) {
      const ref = rawLine.trim()
      if (STEP_REF_BODY.test(ref)) completed.push(ref)
    }
  }
  for (const rawLine of text.split(/\r?\n/)) {
    const done = /^[ \t>*-]*step-done[ \t]*[:(][ \t]*([^)\n]+?)[ \t]*\)?[ \t]*$/i.exec(rawLine)
    if (done?.[1] !== undefined && STEP_REF_BODY.test(done[1].trim())) completed.push(done[1].trim())
    const start = /^[ \t>*-]*step-start[ \t]*[:(][ \t]*([^)\n]+?)[ \t]*\)?[ \t]*$/i.exec(rawLine)
    if (start?.[1] !== undefined && STEP_REF_BODY.test(start[1].trim())) started.push(start[1].trim())
  }
  return { completed: dedupeStepRefs(completed), started: dedupeStepRefs(started) }
}

export type FullAutoPlanAdvancement = Readonly<{
  plan: FullAutoPlan
  /** True when at least one step reached a terminal (done) status this turn --
   * the exact "advancedPlanStep" notion the churn detector (#9175) reads. */
  advanced: boolean
  advancedStepRefs: ReadonlyArray<string>
  startedStepRefs: ReadonlyArray<string>
}>

/**
 * Advance a plan's step statuses from one turn's structured output, disposition,
 * and (optional) host verification verdict. Deterministic and conservative:
 *  - A step reaches `done` ONLY on a `completed` turn AND only when the turn
 *    named it in a structured `STEP-DONE` marker (or the host verified a named
 *    `verifiedStepRef`). A failed/absent/error turn never marks a step done, so
 *    a provider that merely self-reports success cannot fabricate plan progress.
 *  - A `pending` step named by a `STEP-START` marker moves to `in_progress`
 *    (progress, but NOT terminal -- it does not reset churn on its own).
 *  - An unknown or already-terminal stepRef is ignored (a no-op, per
 *    applyFullAutoStepStatus).
 * Returns the mutated plan plus which steps advanced, so the caller can persist
 * the plan and feed `advanced` into the churn signal.
 */
export const advanceFullAutoPlanFromTurn = (
  plan: FullAutoPlan,
  input: Readonly<{
    disposition: string | null
    completedStepRefs?: ReadonlyArray<string>
    startedStepRefs?: ReadonlyArray<string>
    verificationPassed?: boolean
    verifiedStepRef?: string
    now?: () => Date
  }>,
): FullAutoPlanAdvancement => {
  const now = input.now ?? (() => new Date())
  const known = new Map(plan.steps.map((step) => [step.stepRef, step]))
  const isAdvanceable = (ref: string): boolean => {
    const step = known.get(ref)
    return step !== undefined && !isFullAutoPlanStepTerminal(step.status)
  }
  let next = plan
  const advancedStepRefs: Array<string> = []
  const startedStepRefs: Array<string> = []
  if (input.disposition === "completed") {
    const doneRefs = dedupeStepRefs([
      ...(input.completedStepRefs ?? []),
      ...(input.verificationPassed === true && input.verifiedStepRef !== undefined ? [input.verifiedStepRef] : []),
    ])
    for (const ref of doneRefs) {
      if (!isAdvanceable(ref)) continue
      next = applyFullAutoStepStatus(next, { stepRef: ref, status: "done", now })
      advancedStepRefs.push(ref)
    }
  }
  for (const ref of dedupeStepRefs(input.startedStepRefs ?? [])) {
    const step = next.steps.find((candidate) => candidate.stepRef === ref)
    if (step === undefined || step.status !== "pending" || advancedStepRefs.includes(ref)) continue
    next = applyFullAutoStepStatus(next, { stepRef: ref, status: "in_progress", now })
    startedStepRefs.push(ref)
  }
  return { plan: next, advanced: advancedStepRefs.length > 0, advancedStepRefs, startedStepRefs }
}

export type FullAutoPlanReorderResult =
  | Readonly<{ ok: true; plan: FullAutoPlan }>
  | Readonly<{ ok: false; reason: "not_a_permutation" }>

/**
 * Reorder the plan's steps to match `orderedStepRefs`. Refuses (never
 * silently drops or invents) when the supplied order is not an exact
 * permutation of the current step set. The reordered plan preserves each
 * step's status and dependencies -- only the visitation order changes, which
 * is what `nextActionableFullAutoStep` reads.
 */
export const reorderFullAutoPlanSteps = (
  plan: FullAutoPlan,
  orderedStepRefs: ReadonlyArray<string>,
  now: () => Date = () => new Date(),
): FullAutoPlanReorderResult => {
  const current = plan.steps.map((step) => step.stepRef)
  const currentSet = new Set(current)
  const orderedSet = new Set(orderedStepRefs)
  if (
    orderedStepRefs.length !== current.length ||
    orderedSet.size !== orderedStepRefs.length ||
    !orderedStepRefs.every((ref) => currentSet.has(ref))
  ) {
    return { ok: false, reason: "not_a_permutation" }
  }
  const byRef = stepByRef(plan)
  const steps = orderedStepRefs.map((ref) => byRef.get(ref)!)
  return { ok: true, plan: withRevision(plan, steps, now) }
}

// -----------------------------------------------------------------------
// Mission brief -- bounded text for the per-turn mission packet.
// -----------------------------------------------------------------------

export type FullAutoPlanBrief = Readonly<{
  currentStepRef: string | null
  currentStepTitle: string | null
  done: number
  total: number
  text: string
}>

/**
 * Render the bounded plan brief the mission packet carries: the current
 * (next actionable) step plus a compact prior-progress summary. Never the
 * whole plan, never transcript. The `text` is safe to place verbatim in the
 * private provider prompt.
 */
export const renderFullAutoPlanBrief = (plan: FullAutoPlan): FullAutoPlanBrief => {
  const summary = fullAutoPlanProgressSummary(plan)
  const current = nextActionableFullAutoStep(plan)
  const drift = detectFullAutoPlanDrift(plan)
  const lines: Array<string> = [
    `PLAN PROGRESS: ${summary.done}/${summary.total} done` +
      (summary.inProgress > 0 ? `, ${summary.inProgress} in progress` : "") +
      (summary.blocked > 0 ? `, ${summary.blocked} blocked` : "") +
      (summary.skipped > 0 ? `, ${summary.skipped} skipped` : ""),
  ]
  if (summary.doneTitles.length > 0) {
    lines.push(`COMPLETED SO FAR: ${summary.doneTitles.map((title) => `"${title}"`).join("; ")}`)
  }
  if (current !== null) {
    lines.push(`CURRENT STEP (${current.stepRef}): ${current.title}`)
    if (current.note !== undefined && current.note.length > 0) lines.push(`STEP NOTE: ${current.note}`)
    lines.push("Advance the CURRENT STEP now. Do not restart completed steps or open unrelated work.")
  } else if (summary.allTerminal) {
    lines.push("PLAN EXHAUSTED: every step is done or skipped. Verify the done condition; do not invent new work.")
  } else if (drift.drifted) {
    lines.push("PLAN DEADLOCK: no step is currently actionable. Report the blocker; do not spin on blocked steps.")
  }
  return {
    currentStepRef: current?.stepRef ?? null,
    currentStepTitle: current?.title ?? null,
    done: summary.done,
    total: summary.total,
    text: lines.join("\n"),
  }
}
