/**
 * FA-07 gate 2 driver: a multi-turn unattended run on the installed candidate
 * (omega#26 gate 2).
 *
 * # What gate 2 asks for, and what this driver answers
 *
 * Gate 2 is the only gate whose subject is a *person's absence*. Every other
 * gate asks whether the product does the right thing when something happens;
 * gate 2 asks whether it keeps working when nothing happens — when the person
 * who started it walked away. So the interesting quantity is not "did it run",
 * it is "how long did it run with nobody touching it, and how many turns did it
 * take in that window".
 *
 * This driver therefore does three things, in order:
 *
 *   1. **One explicit start action**, in the wire form the human launcher
 *      produces — the exact nine keys of `full_auto_ui::FullAutoDispatch::params`
 *      including the `launchOrigin` token, so the run records which human
 *      gesture started it. This is not a new start path (owner gate 8): it is
 *      the existing launcher's dispatch, replayed onto the installed engine.
 *   2. **A silent window.** After the start the driver makes ZERO framed calls
 *      for a fixed wall-clock period. Not "polls quietly" — none. The engine is
 *      alone with the run. The driver then asserts the turn count grew across a
 *      window in which it provably said nothing.
 *   3. **A return.** The engine is stopped and restarted at a new generation
 *      over the same data root, which is what happens when a person closes the
 *      lid and comes back. The run, its turns, its objective and its report must
 *      all still read correctly.
 *
 * # Who performed it
 *
 * The owner lifted the reservation that made gate 2 an owner-only act
 * (2026-07-25). That permits an agent to PERFORM the run. It does not convert
 * an agent-performed run into a person's observation, and this driver will not
 * let it: every receipt carries `performedBy: "agent"` and the emulated identity
 * that stood in for the operator, and `ownerObservation` is the literal `false`.
 * A reviewer reading the receipt can tell, without being told, that no human
 * watched this.
 *
 * # What this driver does NOT prove
 *
 * It is not the Omega GPUI. It cannot observe a rendered launcher, a rendered
 * run monitor, or a person's eyes on a panel, and it claims none of them. It
 * proves that the engine inside the signed candidate carries a multi-turn run
 * forward with no input, which is the half a reviewer can reproduce from
 * primary artifacts.
 */
import { createHash } from "node:crypto"
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import path from "node:path"

import {
  bindCandidate,
  makeLiveProviderRunner,
  makeUnexercisedProviderRunner,
  startEngine,
  type CandidateBinding,
  type EngineClient,
  type HostPolicy,
  type ProviderRunner,
} from "./fa07-installed-gates.ts"

const RECEIPT_SCHEMA = "openagents.omega.fa07-gate2-unattended-observation.v1"
const DEFAULT_APP = "/Applications/Omega.app"
const CODEX_LANE = "codex-local"

/**
 * The launcher gesture this driver replays.
 *
 * `omega_front_door::LaunchOrigin::all()` is the written allowlist of human
 * gestures, and `full_auto_ui::dispatch` can only build a start request from
 * one of them. There is no `LaunchOrigin::ToolCall`. Naming a real token here
 * is what makes this a replay of the human affordance rather than a new path.
 *
 * The engine does not read this key — `grep launchOrigin` over the engine
 * source finds nothing, and `decodeFullAutoControlRunStartRequest` ignores it.
 * That is stated in the receipt rather than glossed, because a reader could
 * otherwise take the token for a wire-level authority check. Gate 8 is enforced
 * one layer up, by the type of `FullAutoDispatch::from_validated`'s first
 * argument: a start request cannot be constructed without a human gesture.
 * Sending the token here proves this driver used the launcher's wire form; it
 * does not prove the engine checked anything.
 */
const LAUNCH_ORIGIN = "new_thread_menu_item"

/**
 * The identity that stood in for the operator.
 *
 * Named, not hidden. This is an emulated user, and a receipt that said "owner"
 * here would be the exact false claim the packet exists to prevent.
 */
const EMULATED_IDENTITY = "emulated.operator.fa07-gate2"

const sha256 = (value: string): string => createHash("sha256").update(value).digest("hex")

type Frame = Record<string, unknown>

const result = (frame: Frame): Record<string, unknown> => (frame.result ?? {}) as Record<string, unknown>
const runOf = (frame: Frame): Record<string, unknown> => (result(frame).run ?? {}) as Record<string, unknown>
const ok = (frame: Frame): boolean => frame.ok === true

/**
 * Every framed method that changes a run.
 *
 * The unattended claim is "the driver stopped participating", and the only way
 * to make that checkable is to name the write surface and count against it.
 * A method missing from this list would silently launder a nudge into a read.
 */
const WRITE_METHODS = new Set([
  "start",
  "pause",
  "resume",
  "stop",
  "retry",
  "handoff",
  "continue_now",
  "apply_control_intent",
  "publish_projection",
  "decide_attention",
])

/** A framed client that records what the caller did, so absence is provable. */
type AuditedClient = Readonly<{
  client: EngineClient
  call: (method: string, params?: unknown) => Promise<Frame>
  calls: Array<{ method: string; at: number }>
  writeCount: () => number
}>

const auditClient = (client: EngineClient): AuditedClient => {
  const calls: Array<{ method: string; at: number }> = []
  return {
    client,
    calls,
    call: async (method, params) => {
      calls.push({ method, at: Date.now() })
      return client.call(method, params)
    },
    writeCount: () => calls.filter((entry) => WRITE_METHODS.has(entry.method)).length,
  }
}

const makePolicy = (readyLanes: ReadonlyArray<string>, providers: ProviderRunner): HostPolicy => ({
  readyLanes: new Set(readyLanes),
  syncSessionAvailable: () => false,
  providers,
  trace: [],
  systemNotes: [],
  invocations: [],
})

/**
 * The start request a human gesture produces.
 *
 * Field-for-field the wire form of `FullAutoDispatch::params()`. The Rust side
 * asserts that this key set is exactly nine keys and that a start request has
 * nowhere to write forged evidence; sending anything else from here would be
 * proving gate 2 against a request the product cannot make.
 */
const launcherDispatchParams = (
  objective: string,
  doneCondition: string,
  turnCap: number,
): Record<string, unknown> => ({
  workspaceRef: "workspace.omega.supervised",
  title: "FA-07 gate 2 unattended run",
  objective,
  doneCondition,
  lane: CODEX_LANE,
  turnCap,
  projectRef: "project.fa07.gate2",
  worktreeRef: "worktree.fa07.gate2",
  launchOrigin: LAUNCH_ORIGIN,
})

const TERMINAL_STATES = new Set(["completed", "failed", "stopped", "cap_reached"])

export type Gate2Options = Readonly<{
  binding: CandidateBinding
  providers: ProviderRunner
  live: boolean
  dataRoot: string
  turnCap: number
  /** Wall-clock seconds during which the driver makes no framed call at all. */
  silentWindowMs: number
  /** Minimum turns for the run to count as multi-turn. */
  minimumTurns: number
  /** Deliberately touch the run mid-window, to watch the gate fail. */
  falsifyAttended: boolean
}>

export const gate2UnattendedRun = async (options: Gate2Options): Promise<Record<string, unknown>> => {
  const { binding, providers, live, dataRoot } = options
  const policy = makePolicy([CODEX_LANE], providers)
  const audited = auditClient(await startEngine(binding.enginePath, dataRoot, policy))

  const objective = "FA07_GATE2_UNATTENDED_OBJECTIVE_MUST_NOT_LEAVE_THE_HOST"
  const doneCondition = "FA07_GATE2_DONE_CONDITION"

  // ---- 1. one explicit start action -------------------------------------
  const startedAt = Date.now()
  const started = await audited.call("start", launcherDispatchParams(objective, doneCondition, options.turnCap))
  if (!ok(started)) {
    await audited.client.stop()
    return { gate: "owner_real_multi_turn", startAccepted: false, startError: started.error }
  }
  const run = runOf(started)
  const runRef = String(run.runRef)
  const startWriteCount = audited.writeCount()

  // Wait for the FIRST turn only, so the silent window is measured against a
  // run that is genuinely under way rather than one that never started.
  let detail = runOf(await audited.call("get_run", { runRef }))
  const firstTurnDeadline = Date.now() + (live ? 300_000 : 30_000)
  while (
    Date.now() < firstTurnDeadline &&
    Number(detail.successfulAttempts ?? 0) < 1 &&
    !TERMINAL_STATES.has(String(detail.state))
  ) {
    await new Promise((resolve) => setTimeout(resolve, 3_000))
    detail = runOf(await audited.call("get_run", { runRef }))
  }
  const turnsBeforeSilence = ((detail.turns ?? []) as ReadonlyArray<unknown>).length
  const attemptsBeforeSilence = Number(detail.successfulAttempts ?? 0)
  const dispatchesBeforeSilence = policy.trace.filter((entry) => entry.method === "dispatch_turn").length

  // ---- 2. the silent window ---------------------------------------------
  // From here to the next line of code, the driver says nothing. If the turn
  // count grows across this gap, it grew with nobody in the room.
  const silenceStart = Date.now()
  const callsAtSilenceStart = audited.calls.length
  if (options.falsifyAttended) {
    // The watched failure: one nudge inside the window. Everything else about
    // the run is identical, so a green result here would mean the gate is not
    // measuring what it claims to measure.
    await new Promise((resolve) => setTimeout(resolve, Math.min(5_000, options.silentWindowMs / 2)))
    await audited.call("resume", { runRef })
  }
  await new Promise((resolve) => setTimeout(resolve, options.silentWindowMs))
  const silenceEnd = Date.now()
  const callsDuringSilence = audited.calls.filter(
    (entry) => entry.at >= silenceStart && entry.at <= silenceEnd,
  )

  detail = runOf(await audited.call("get_run", { runRef }))
  const turnsAfterSilence = ((detail.turns ?? []) as ReadonlyArray<unknown>).length
  const attemptsAfterSilence = Number(detail.successfulAttempts ?? 0)
  const dispatchesAfterSilence = policy.trace.filter((entry) => entry.method === "dispatch_turn").length

  // Let the run reach its own ending, still without writing to it.
  const finishDeadline = Date.now() + (live ? 900_000 : 60_000)
  while (
    Date.now() < finishDeadline &&
    !TERMINAL_STATES.has(String(detail.state)) &&
    Number(detail.successfulAttempts ?? 0) < options.turnCap
  ) {
    await new Promise((resolve) => setTimeout(resolve, 5_000))
    detail = runOf(await audited.call("get_run", { runRef }))
  }
  if (live) await audited.client.waitForIdle(120_000)
  detail = runOf(await audited.call("get_run", { runRef }))

  const unattendedMs = Date.now() - startedAt
  const writesAfterStart = audited.writeCount() - startWriteCount
  const turnsAtEnd = (detail.turns ?? []) as ReadonlyArray<Record<string, unknown>>

  await audited.client.stop()

  // ---- 3. the return -----------------------------------------------------
  // A new supervisor generation over the same data root: the lid closed and
  // opened again. Nothing here writes to the run either.
  const policyB = makePolicy([CODEX_LANE], providers)
  const auditedB = auditClient(await startEngine(binding.enginePath, dataRoot, policyB, 2))
  const afterReturn = runOf(await auditedB.call("get_run", { runRef }))
  const reportAfterReturn = (result(await auditedB.call("get_report", { runRef })).report ?? {}) as Record<
    string,
    unknown
  >
  const receiptAfterReturn = (result(await auditedB.call("get_receipt", { runRef })).receipt ?? {}) as Record<
    string,
    unknown
  >
  const listedAfterReturn = result(await auditedB.call("list_runs"))
  await auditedB.client.stop()

  const returnTurns = (afterReturn.turns ?? []) as ReadonlyArray<Record<string, unknown>>
  const reportsDir = path.join(dataRoot, "full-auto")
  const durableFiles = existsSync(reportsDir)

  // Redaction is not gate 2's subject, but a gate-2 receipt that leaked the
  // objective would be evidence of a defect while claiming to be evidence of
  // health, so it is checked here rather than assumed from gate 9.
  const publicSurfaceText = JSON.stringify({ reportAfterReturn, receiptAfterReturn, listedAfterReturn })

  return {
    gate: "owner_real_multi_turn",
    runRef,
    // Attribution, first, because it changes how everything below reads.
    performedBy: "agent",
    performedByIdentity: EMULATED_IDENTITY,
    ownerObservation: false,
    ownerObservationReason:
      "The owner lifted the reservation on performing this run (2026-07-25). Performing it does not make it a person's observation, and no person watched this run.",
    launchOrigin: LAUNCH_ORIGIN,
    launchOriginIsAnAdmittedHumanGesture: true,
    launchOriginReadByEngine: false,
    launchOriginNote:
      "The token is the launcher's wire form. The engine ignores it; gate 8 is enforced by the Rust type of FullAutoDispatch::from_validated, not by a wire check.",
    startRequestKeys: Object.keys(launcherDispatchParams(objective, doneCondition, options.turnCap)).sort(),

    providersExercised: live,
    startAccepted: true,
    stateAtStart: String(run.state),

    // The unattended property.
    unattendedMs,
    silentWindowMs: silenceEnd - silenceStart,
    framedCallsDuringSilentWindow: callsDuringSilence.length,
    framedCallMethodsDuringSilentWindow: callsDuringSilence.map((entry) => entry.method),
    silentWindowWasSilent: callsDuringSilence.length === 0,
    callsAtSilenceStart,
    writeCallsAfterStart: writesAfterStart,
    unattendedAfterStart: writesAfterStart === 0,

    // Turns taken while nobody was participating.
    turnsBeforeSilence,
    turnsAfterSilence,
    turnsAdvancedDuringSilentWindow: turnsAfterSilence > turnsBeforeSilence,
    attemptsBeforeSilence,
    attemptsAfterSilence,
    engineDispatchesBeforeSilence: dispatchesBeforeSilence,
    engineDispatchesAfterSilence: dispatchesAfterSilence,
    engineDispatchedDuringSilentWindow: dispatchesAfterSilence > dispatchesBeforeSilence,

    // Multi-turn.
    turnCount: turnsAtEnd.length,
    successfulAttempts: Number(detail.successfulAttempts ?? 0),
    failedAttempts: Number(detail.failedAttempts ?? 0),
    turnCap: options.turnCap,
    minimumTurns: options.minimumTurns,
    multiTurn: turnsAtEnd.length >= options.minimumTurns,
    turnRefsDistinct:
      new Set(turnsAtEnd.map((turn) => String(turn.turnRef))).size === turnsAtEnd.length,
    finalState: String(detail.state),
    finalStateIsTerminal: TERMINAL_STATES.has(String(detail.state)),
    stallCause: detail.stallCause ?? null,
    recoveryAction: detail.recoveryAction ?? null,

    // Provider honesty: a turn that no provider executed is not a turn.
    providerInvocations: policy.invocations.map((invocation) => ({
      lane: invocation.lane,
      ok: invocation.ok,
      exitCode: invocation.exitCode,
      outputLength: invocation.outputLength,
      elapsedMs: invocation.elapsedMs,
    })),
    providerInvocationCount: policy.invocations.length,
    everyTurnHadAProvider: live
      ? policy.invocations.filter((invocation) => invocation.ok).length >= turnsAtEnd.length
      : false,

    // The return.
    runSurvivedReturn: String(afterReturn.runRef) === runRef,
    stateAfterReturn: String(afterReturn.state),
    stateHonestAfterReturn: String(afterReturn.state) === String(detail.state),
    turnCountAfterReturn: returnTurns.length,
    everyTurnListedAfterReturn: returnTurns.length === turnsAtEnd.length,
    objectivePresentInOwnerLocalDetailAfterReturn: JSON.stringify(afterReturn).includes(objective),
    reportSurvivedReturn: reportAfterReturn.runRef === runRef,
    reportTurnCount: Array.isArray(reportAfterReturn.turns)
      ? (reportAfterReturn.turns as ReadonlyArray<unknown>).length
      : null,
    receiptCarriesObjectiveDigest: typeof receiptAfterReturn.objectiveDigest === "string",
    receiptObjectiveDigestMatches: receiptAfterReturn.objectiveDigest === sha256(objective),
    durableStateOnDisk: durableFiles,
    objectiveTextAbsentFromPublicSurfaces: !publicSurfaceText.includes(objective),

    // What this observation is not.
    rendersObserved: [],
    doesNotProve: [
      "a rendered GPUI launcher, run monitor, or transcript",
      "a person's observation of the run",
    ],
  }
}

const parseArgs = (argv: ReadonlyArray<string>): Record<string, string | boolean> => {
  const out: Record<string, string | boolean> = {}
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (!arg?.startsWith("--")) continue
    const key = arg.slice(2)
    const next = argv[index + 1]
    if (next === undefined || next.startsWith("--")) out[key] = true
    else {
      out[key] = next
      index += 1
    }
  }
  return out
}

export const main = async (argv: ReadonlyArray<string>): Promise<number> => {
  const args = parseArgs(argv)
  const appPath = typeof args.app === "string" ? args.app : DEFAULT_APP
  const binding = bindCandidate(appPath, typeof args.engine === "string" ? args.engine : undefined)
  const live = args["live-providers"] === true
  const providers = live
    ? makeLiveProviderRunner({
        codexBin: String(args["codex-bin"] ?? ""),
        codexHome: String(args["codex-home"] ?? ""),
        claudeBin: String(args["claude-bin"] ?? "claude"),
        workdir: String(args.workdir ?? process.cwd()),
        timeoutMs: Number(args["provider-timeout-ms"] ?? 300_000),
      })
    : makeUnexercisedProviderRunner()

  const dataRoot = String(args["data-root"] ?? path.join(process.cwd(), ".fa07-gate2-data"))
  const keepDataRoot = args["keep-data-root"] === true
  mkdirSync(dataRoot, { recursive: true })

  try {
    const observation = await gate2UnattendedRun({
      binding,
      providers,
      live,
      dataRoot,
      turnCap: Number(args["turn-cap"] ?? 4),
      silentWindowMs: Number(args["silent-window-ms"] ?? 120_000),
      minimumTurns: Number(args["minimum-turns"] ?? 3),
      falsifyAttended: args["falsify-attended"] === true,
    })

    const receipt = {
      schema: RECEIPT_SCHEMA,
      generatedAt: new Date().toISOString(),
      issue: "omega#26 (OMEGA-FA-07) gate 2",
      candidate: binding,
      providersExercised: live,
      dataRoot: keepDataRoot ? dataRoot : null,
      observation,
    }
    const serialized = JSON.stringify(receipt, null, 2)
    if (/sk-[a-z]+-[A-Za-z0-9]{8,}|BEGIN [A-Z ]*PRIVATE KEY|Bearer\s+[A-Za-z0-9._-]{16,}/.test(serialized)) {
      console.error(JSON.stringify({ ok: false, error: "receipt failed its own secret-shape scan" }))
      return 2
    }
    if (typeof args.out === "string") writeFileSync(args.out, `${serialized}\n`)
    console.log(serialized)
    return 0
  } finally {
    if (!keepDataRoot) rmSync(dataRoot, { recursive: true, force: true })
  }
}

const invokedDirectly =
  process.argv[1] !== undefined && process.argv[1].endsWith("fa07-gate2-unattended.ts")
if (invokedDirectly) {
  process.exit(await main(process.argv.slice(2)))
}
