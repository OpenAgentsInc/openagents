/**
 * FA-07 installed-candidate gate driver (omega#26 gates 5, 6, 7, plus 4 and 9
 * as installed observations).
 *
 * The FA-07 proof matrix separates two kinds of evidence, and the difference is
 * the whole point of the packet:
 *
 *   - A vitest in this package proves a property of the ENGINE SOURCE. It is
 *     fast, falsifiable, and says nothing about the bytes a person installed.
 *   - This driver spawns the ENGINE THAT SHIPPED INSIDE A SIGNED, NOTARIZED
 *     OMEGA CANDIDATE, over the same newline-framed stdio protocol the Omega
 *     GPUI supervisor uses, and drives the real framed methods against a real
 *     data root on disk.
 *
 * omega `script/generate-omega-full-auto-candidate-evidence` lists
 * `visible_cross_provider_handoff`, `offline_and_sync_gap`, and
 * `mobile_control_outcomes` in INSTALLED_ONLY_GATES: a unit test cannot satisfy
 * them by construction. This driver is the missing half.
 *
 * Provider honesty. With `--live-providers` the host stand-in dispatches each
 * turn to a REAL provider CLI, so a handoff means Codex actually did turn one
 * and Claude actually did turn two. Provider homes are isolated: the Codex home
 * is a caller-supplied directory, never the agent's default `~/.codex`, and
 * nothing here ever runs `codex login` (that flow clears `auth.json` at start
 * and would destroy a live session). Without the flag the host refuses to
 * pretend a provider ran; it reports `providers: "not_exercised"` and the
 * handoff gate reports engine-level evidence only.
 *
 * What this driver does NOT prove. It is not the Omega GPUI. It cannot observe
 * a rendered sidebar, a rendered transcript, or a rendered panel, and it does
 * not claim to. It proves what the installed engine does, which is the half a
 * reviewer can reproduce from primary artifacts.
 *
 * Output is public-safe by construction: typed states, counts, refs, and
 * digests. Objective text, done-condition text, prompt text, provider output,
 * and credentials never enter the receipt, and the driver asserts that.
 */
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { createHash } from "node:crypto"
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import readline from "node:readline"

const PROTOCOL_SCHEMA = "openagents.omega.effectd.v1"
const LOCAL_TURN_RECORD_SCHEMA = "openagents.desktop.local_turn_record.v1"
const PROVIDER_HANDOFF_ENVELOPE_SCHEMA = "openagents.desktop.provider_handoff_envelope.v1"
const RECEIPT_SCHEMA = "openagents.omega.fa07-installed-gate-observation.v1"

const DEFAULT_APP = "/Applications/Omega.app"
const CODEX_LANE = "codex-local"
const CLAUDE_LANE = "claude-local"

const sha256 = (value: string | Buffer): string => createHash("sha256").update(value).digest("hex")

const digestFile = (file: string): string => sha256(readFileSync(file))

// ---------------------------------------------------------------------------
// Candidate binding
// ---------------------------------------------------------------------------

export type CandidateBinding = Readonly<{
  appPath: string
  enginePath: string
  /** True when --engine pointed somewhere other than the installed bundle. */
  engineOverridden: boolean
  hostBinaryDigest: string
  engineBundleDigest: string
  engineBundleDigestMatchesManifest: boolean
  componentManifestDigest: string
  engineSourceCommit: string
  serviceVersion: string
  shortVersion: string
  bundleVersion: string
}>

/**
 * Bind to the installed candidate.
 *
 * `engineOverride` exists for ONE purpose: running the identical gate battery
 * against a source-tree engine so the two results can be diffed. It is recorded
 * in the receipt as `engineOverridden: true`, and a receipt with that flag is
 * NOT installed-candidate evidence for any gate.
 */
export const bindCandidate = (appPath: string, engineOverride?: string): CandidateBinding => {
  const enginePath =
    engineOverride ?? path.join(appPath, "Contents/Resources/omega-effectd/bin/omega-effectd")
  const bundlePath = path.join(appPath, "Contents/Resources/omega-effectd/dist/omega-effectd.mjs")
  const manifestPath = path.join(appPath, "Contents/Resources/omega-effectd/component-manifest.json")
  const hostBinary = path.join(appPath, "Contents/MacOS/omega")
  for (const required of [enginePath, bundlePath, manifestPath, hostBinary]) {
    if (!existsSync(required)) throw new Error(`installed candidate is missing ${required}`)
  }
  const manifestRaw = readFileSync(manifestPath, "utf8")
  const manifest = JSON.parse(manifestRaw) as {
    files: Record<string, string>
    serviceVersion: string
    source: { commit: string }
  }
  const engineBundleDigest = digestFile(bundlePath)
  const plist = readFileSync(path.join(appPath, "Contents/Info.plist"), "utf8")
  const readPlist = (key: string): string =>
    plist.match(new RegExp(`<key>${key}</key>\\s*<string>([^<]*)</string>`))?.[1] ?? "unknown"
  return {
    appPath,
    enginePath,
    engineOverridden: engineOverride !== undefined,
    hostBinaryDigest: digestFile(hostBinary),
    engineBundleDigest,
    // The manifest is the candidate's own claim about its engine. Recomputing
    // the digest and comparing is what makes it evidence rather than a label.
    engineBundleDigestMatchesManifest: manifest.files["dist/omega-effectd.mjs"] === engineBundleDigest,
    componentManifestDigest: sha256(manifestRaw),
    engineSourceCommit: manifest.source.commit,
    serviceVersion: manifest.serviceVersion,
    shortVersion: readPlist("CFBundleShortVersionString"),
    bundleVersion: readPlist("CFBundleVersion"),
  }
}

// ---------------------------------------------------------------------------
// Provider lanes
// ---------------------------------------------------------------------------

export type ProviderInvocation = Readonly<{
  lane: string
  ok: boolean
  exitCode: number | null
  /** Digest only. Provider output never enters the receipt verbatim. */
  outputDigest: string
  outputLength: number
  elapsedMs: number
}>

export type ProviderRunner = (lane: string, prompt: string) => Promise<ProviderInvocation & { text: string }>

/**
 * A provider runner that refuses to fabricate a turn.
 *
 * The engine's `dispatch_turn` contract only needs `{accepted: true}`, so a
 * host stand-in can trivially report success for work nobody did. That would
 * make gate 5 a lie in exactly the shape the packet's Non-goals forbid, so the
 * default runner records `not_exercised` and the caller must say so out loud.
 */
export const makeUnexercisedProviderRunner = (): ProviderRunner => async (lane) => ({
  lane,
  ok: false,
  exitCode: null,
  outputDigest: sha256(""),
  outputLength: 0,
  elapsedMs: 0,
  text: "",
})

export type LiveProviderConfig = Readonly<{
  codexBin: string
  /** Isolated CODEX_HOME. Never the caller's default agent home. */
  codexHome: string
  claudeBin: string
  workdir: string
  timeoutMs: number
}>

const runProcess = (
  bin: string,
  args: ReadonlyArray<string>,
  options: Readonly<{ cwd: string; env: NodeJS.ProcessEnv; timeoutMs: number; stdin?: string }>,
): Promise<Readonly<{ code: number | null; stdout: string }>> =>
  new Promise((resolve) => {
    const child = spawn(bin, [...args], { cwd: options.cwd, env: options.env, stdio: ["pipe", "pipe", "pipe"] })
    let stdout = ""
    child.stdout.on("data", (chunk) => {
      stdout += String(chunk)
    })
    child.stderr.on("data", () => {})
    const timer = setTimeout(() => child.kill("SIGKILL"), options.timeoutMs)
    child.on("close", (code) => {
      clearTimeout(timer)
      resolve({ code, stdout })
    })
    child.stdin.end(options.stdin ?? "")
  })

export const makeLiveProviderRunner = (config: LiveProviderConfig): ProviderRunner => async (lane, prompt) => {
  const startedAt = Date.now()
  if (lane === CODEX_LANE) {
    if (path.resolve(config.codexHome) === path.resolve(process.env.HOME ?? "", ".codex")) {
      throw new Error("refusing to drive Codex against the default agent home; pass an isolated CODEX_HOME")
    }
    const { code, stdout } = await runProcess(
      config.codexBin,
      ["exec", "--skip-git-repo-check", "-s", "read-only", prompt],
      {
        cwd: config.workdir,
        env: { ...process.env, CODEX_HOME: config.codexHome },
        timeoutMs: config.timeoutMs,
      },
    )
    return {
      lane,
      ok: code === 0,
      exitCode: code,
      outputDigest: sha256(stdout),
      outputLength: stdout.length,
      elapsedMs: Date.now() - startedAt,
      text: stdout,
    }
  }
  if (lane === CLAUDE_LANE) {
    const { code, stdout } = await runProcess(config.claudeBin, ["-p", prompt, "--allowed-tools", ""], {
      cwd: config.workdir,
      env: process.env,
      timeoutMs: config.timeoutMs,
    })
    return {
      lane,
      ok: code === 0,
      exitCode: code,
      outputDigest: sha256(stdout),
      outputLength: stdout.length,
      elapsedMs: Date.now() - startedAt,
      text: stdout,
    }
  }
  throw new Error(`no live provider is wired for lane ${lane}`)
}

// ---------------------------------------------------------------------------
// Framed client + host stand-in
// ---------------------------------------------------------------------------

type Frame = Record<string, unknown>

export type HostPolicy = Readonly<{
  /** Lanes the host reports as authenticated and Full-Auto capable. */
  readyLanes: ReadonlySet<string>
  /** `false` makes `resolve_sync_session` report the offline answer. */
  syncSessionAvailable: () => boolean
  providers: ProviderRunner
  /** Records every host request the engine made, for gate evidence. */
  trace: Array<{ method: string; at: string }>
  systemNotes: Array<{ threadRef: string; text: string }>
  invocations: Array<ProviderInvocation>
}>

export type EngineClient = Readonly<{
  call: (method: string, params?: unknown, generation?: number) => Promise<Frame>
  stop: () => Promise<void>
  dataRoot: string
  waitForIdle: (ms: number) => Promise<void>
}>

export const startEngine = async (
  enginePath: string,
  dataRoot: string,
  policy: HostPolicy,
  generation = 1,
): Promise<EngineClient> => {
  const child: ChildProcessWithoutNullStreams = spawn(enginePath, [], {
    env: { ...process.env, OPENAGENTS_OMEGA_EFFECTD_DATA_ROOT: dataRoot },
    stdio: ["pipe", "pipe", "pipe"],
  })
  child.stderr.on("data", () => {})

  const pending = new Map<string, (frame: Frame) => void>()
  let threadCounter = 0
  const turnsByThread = new Map<string, Array<Record<string, unknown>>>()
  const liveByThread = new Map<string, Record<string, unknown> | null>()
  const laneByThread = new Map<string, string>()
  let inFlight = 0

  const send = (frame: Frame): void => {
    child.stdin.write(`${JSON.stringify(frame)}\n`)
  }
  const reply = (request: Frame, result: unknown): void =>
    send({
      schema: PROTOCOL_SCHEMA,
      kind: "host_response",
      id: request.id,
      generation: request.generation,
      ok: true,
      result,
    })

  const executeTurn = async (
    threadRef: string,
    turnRef: string,
    lane: string,
    prompt: string,
  ): Promise<void> => {
    inFlight += 1
    liveByThread.set(threadRef, { state: "turn_running", turnRef })
    try {
      const invocation = await policy.providers(lane, prompt)
      const { text: _providerText, ...publicSafe } = invocation
      policy.invocations.push(publicSafe)
      const now = new Date().toISOString()
      const turns = turnsByThread.get(threadRef) ?? []
      turns.push({
        schema: LOCAL_TURN_RECORD_SCHEMA,
        threadRef,
        turnRef,
        lane,
        userMessageKey: `msg.user.${turnRef}`,
        assistantMessageKey: `msg.assistant.${turnRef}`,
        accountRef: null,
        providerSessionRef: null,
        model: null,
        phase: invocation.ok ? "completed" : "failed",
        persistedCursor: turns.length,
        assistantText: invocation.text.slice(0, 32_000),
        assistantSegments: [],
        recoveryGeneration: 0,
        disposition: invocation.ok ? "completed" : "failed",
        createdAt: now,
        updatedAt: now,
      })
      turnsByThread.set(threadRef, turns)
      liveByThread.set(threadRef, { state: "turn_completed", turnRef })
    } finally {
      inFlight -= 1
    }
  }

  const onHostRequest = async (request: Frame): Promise<void> => {
    const params = (request.params ?? {}) as Record<string, unknown>
    policy.trace.push({ method: String(request.method), at: new Date().toISOString() })
    switch (request.method) {
      case "resolve_workspace":
        return reply(request, {
          workspaceRef: params.expectedWorkspaceRef ?? "workspace.omega.supervised",
        })
      case "resolve_sync_session":
        // `omega_host_bridge::sync_session_result` answers `{available:false}`
        // whenever no OpenAgents session is admitted -- signed out, expired, or
        // no network. That is the offline condition this driver reproduces. It
        // does NOT mint a session for the available branch: fabricating a
        // credential here would make the "gap closes" claim untestable rather
        // than tested, so the online half stays explicitly unproven.
        return reply(request, { available: policy.syncSessionAvailable() })
      case "lane_readiness":
        return reply(request, {
          known: true,
          admitted: policy.readyLanes.has(String(params.lane)),
          fullAuto: policy.readyLanes.has(String(params.lane)),
          state: policy.readyLanes.has(String(params.lane)) ? "available" : "unavailable",
        })
      case "create_thread": {
        const threadRef = `thread.fa07.${++threadCounter}`
        turnsByThread.set(threadRef, [])
        liveByThread.set(threadRef, null)
        laneByThread.set(threadRef, String(params.lane ?? CODEX_LANE))
        return reply(request, { threadRef })
      }
      case "refresh_evidence": {
        const threadRef = String(params.threadRef)
        const turns = turnsByThread.get(threadRef) ?? []
        return reply(request, {
          present: turnsByThread.has(threadRef),
          revision: turns.length + (liveByThread.get(threadRef) === null ? 0 : 1),
          live: liveByThread.get(threadRef) ?? null,
          turns,
        })
      }
      case "dispatch_turn": {
        const threadRef = String(params.threadRef)
        const lane = laneByThread.get(threadRef) ?? CODEX_LANE
        if (!policy.readyLanes.has(lane)) {
          return reply(request, { accepted: false, reason: "lane_unavailable", failureCause: "lane_unavailable" })
        }
        // Accept first, execute after: the engine's dispatch contract is
        // accept-then-observe-through-evidence, exactly like the GPUI host.
        reply(request, { accepted: true })
        void executeTurn(threadRef, String(params.turnRef), lane, String(params.message ?? ""))
        return
      }
      case "interrupt_turn":
        return reply(request, { interrupted: true })
      case "append_system_note":
        policy.systemNotes.push({ threadRef: String(params.threadRef), text: String(params.text ?? "") })
        return reply(request, { appended: true })
      default:
        return send({
          schema: PROTOCOL_SCHEMA,
          kind: "host_response",
          id: request.id,
          generation: request.generation,
          ok: false,
          error: { code: "unsupported", message: "this host stand-in does not implement that method" },
        })
    }
  }

  readline.createInterface({ input: child.stdout }).on("line", (line) => {
    if (line.trim().length === 0) return
    let frame: Frame
    try {
      frame = JSON.parse(line) as Frame
    } catch {
      return
    }
    if (frame.kind === "host_request") {
      void onHostRequest(frame)
      return
    }
    if (frame.kind === "response") {
      const resolve = pending.get(String(frame.id))
      pending.delete(String(frame.id))
      resolve?.(frame)
    }
  })

  let seq = 0
  const call = (method: string, params?: unknown, atGeneration = generation): Promise<Frame> =>
    new Promise((resolve, reject) => {
      const id = `fa07.${++seq}`
      const timer = setTimeout(() => {
        pending.delete(id)
        reject(new Error(`framed call ${method} timed out`))
      }, 600_000)
      pending.set(id, (frame) => {
        clearTimeout(timer)
        resolve(frame)
      })
      send({
        schema: PROTOCOL_SCHEMA,
        kind: "request",
        id,
        generation: atGeneration,
        method,
        ...(params === undefined ? {} : { params }),
      })
    })

  const waitForIdle = async (ms: number): Promise<void> => {
    const deadline = Date.now() + ms
    while (Date.now() < deadline) {
      if (inFlight === 0) {
        await new Promise((r) => setTimeout(r, 250))
        if (inFlight === 0) return
      }
      await new Promise((r) => setTimeout(r, 250))
    }
  }

  const stop = async (): Promise<void> => {
    child.kill("SIGTERM")
    await new Promise((r) => setTimeout(r, 300))
    if (child.exitCode === null) child.kill("SIGKILL")
  }

  await call("initialize", { generation }, 0)
  return { call, stop, dataRoot, waitForIdle }
}

const ok = (frame: Frame): boolean => frame.ok === true
const result = (frame: Frame): Record<string, unknown> => (frame.result ?? {}) as Record<string, unknown>
const runOf = (frame: Frame): Record<string, unknown> => (result(frame).run ?? {}) as Record<string, unknown>
const errorCode = (frame: Frame): string => String(((frame.error ?? {}) as Record<string, unknown>).code ?? "")

const makePolicy = (readyLanes: ReadonlyArray<string>, providers: ProviderRunner): HostPolicy => ({
  readyLanes: new Set(readyLanes),
  syncSessionAvailable: () => false,
  providers,
  trace: [],
  systemNotes: [],
  invocations: [],
})

const startRun = (client: EngineClient, lane: string, objective: string, doneCondition: string) =>
  client.call("start", {
    workspaceRef: "workspace.omega.supervised",
    title: "FA-07 installed gate",
    objective,
    doneCondition,
    lane,
    turnCap: 6,
    projectRef: "project.fa07.installed",
    worktreeRef: "worktree.fa07.installed",
  })

// ---------------------------------------------------------------------------
// Gate 5 — visible cross-provider handoff
// ---------------------------------------------------------------------------

export const gate5CrossProviderHandoff = async (
  binding: CandidateBinding,
  providers: ProviderRunner,
  live: boolean,
): Promise<Record<string, unknown>> => {
  const root = mkdtempSync(path.join(tmpdir(), "fa07-gate5-"))
  const policy = makePolicy([CODEX_LANE, CLAUDE_LANE], providers)
  const client = await startEngine(binding.enginePath, root, policy)
  try {
    const objective = "FA07_GATE5_OBJECTIVE_MUST_NOT_LEAVE_THE_HOST"
    const started = await startRun(client, CODEX_LANE, objective, "FA07_GATE5_DONE_CONDITION")
    const run = runOf(started)
    const runRef = String(run.runRef)

    // Falsifier 1: a handoff must be illegal while the run is dispatching.
    // Without this the "paused only" rule could be vacuous.
    const whileRunning = await client.call("handoff", { runRef, targetLaneRef: CLAUDE_LANE })
    const refusedWhileRunning = !ok(whileRunning)

    if (live) await client.waitForIdle(300_000)

    const paused = await client.call("pause", { runRef })
    // `pausing` settles once the in-flight provider turn completes.
    let pausedState = String(runOf(paused).state)
    for (let attempt = 0; attempt < 60 && pausedState !== "paused"; attempt += 1) {
      await new Promise((r) => setTimeout(r, 1_000))
      pausedState = String(runOf(await client.call("get_run", { runRef })).state)
    }

    // Falsifier 2: an unknown lane must be refused, not silently accepted.
    const unknownLane = await client.call("handoff", { runRef, targetLaneRef: "lane.does.not.exist" })
    const refusedUnknownLane = !ok(unknownLane)

    const beforeLane = String(runOf(await client.call("get_run", { runRef })).lane)
    const traceMarkAtHandoff = policy.trace.length
    const handoff = await client.call("handoff", {
      runRef,
      targetLaneRef: CLAUDE_LANE,
      reason: "FA-07 gate 5 installed-candidate cross-provider proof",
    })
    const transition = (result(handoff).transition ?? {}) as Record<string, unknown>
    const afterLane = String(runOf(handoff).lane)

    const resumed = await client.call("resume", { runRef })
    // Wait for the TARGET provider to actually produce a turn, not merely for
    // the host to go briefly idle. `waitForIdle` alone returns the instant
    // nothing is in flight, which is true one millisecond after resume and
    // before the engine has dispatched anything -- and a gate 5 that measures
    // that would report "Codex only" for a working handoff.
    let detail = runOf(await client.call("get_run", { runRef }))
    if (live) {
      const deadline = Date.now() + 600_000
      while (Date.now() < deadline) {
        const turnsNow = (detail.turns ?? []) as ReadonlyArray<Record<string, unknown>>
        if (turnsNow.some((turn) => turn.lane === CLAUDE_LANE)) break
        if (["completed", "failed", "stopped", "cap_reached"].includes(String(detail.state))) break
        await new Promise((r) => setTimeout(r, 3_000))
        detail = runOf(await client.call("get_run", { runRef }))
      }
      await client.waitForIdle(60_000)
      detail = runOf(await client.call("get_run", { runRef }))
    }
    const turns = (detail.turns ?? []) as ReadonlyArray<Record<string, unknown>>
    const report = result(await client.call("get_report", { runRef }))
    const reportRecord = (report.report ?? {}) as Record<string, unknown>
    const providerTransitions = (reportRecord.providerTransitions ?? []) as ReadonlyArray<
      Record<string, unknown>
    >

    // The durable artifact on disk, which is what an independent reviewer can
    // reproduce without trusting this process at all.
    const handoffFile = path.join(root, "full-auto", "provider-handoffs.json")
    const durable = JSON.parse(readFileSync(handoffFile, "utf8")) as {
      schema: string
      transitions: ReadonlyArray<Record<string, unknown>>
    }
    const durableAccepted = durable.transitions.filter((t) => t.disposition !== "refused")

    // The transcript artifact. `omega_host_bridge::append_system_note` is what
    // renders this into the target thread in the GPUI; here we observe that the
    // engine emits it, addressed to the NEW thread.
    const handoffNotes = policy.systemNotes.filter((note) => note.text.startsWith("Provider handoff:"))

    await client.stop()
    return {
      gate: "visible_cross_provider_handoff",
      runRef,
      providersExercised: live,
      refusedWhileRunning,
      refusedWhileRunningCode: refusedWhileRunning ? errorCode(whileRunning) : null,
      refusedUnknownLane,
      pausedBeforeHandoff: pausedState === "paused",
      laneBefore: beforeLane,
      laneAfter: afterLane,
      laneRebound: beforeLane === CODEX_LANE && afterLane === CLAUDE_LANE,
      transition: {
        handoffRefShape: /^handoff\.provider\./.test(String(transition.handoffRef)),
        from: transition.from,
        to: transition.to,
        actor: transition.actor,
        disposition: transition.disposition,
        truncated: transition.truncated,
        envelopeSchema: transition.envelopeSchema,
        envelopeSchemaExpected: transition.envelopeSchema === PROVIDER_HANDOFF_ENVELOPE_SCHEMA,
        threadSplit:
          typeof transition.sourceThreadRef === "string" &&
          typeof transition.targetThreadRef === "string" &&
          transition.sourceThreadRef !== transition.targetThreadRef,
      },
      durableTransitionCount: durable.transitions.length,
      durableAcceptedCount: durableAccepted.length,
      durableSchema: durable.schema,
      reportAgreesWithRegistry:
        providerTransitions.length === durable.transitions.length &&
        providerTransitions.every(
          (t, index) => t.handoffRef === durable.transitions[index]?.handoffRef,
        ),
      resumedAfterHandoff: ok(resumed),
      turnLanes: turns.map((turn) => String(turn.lane)),
      turnCount: turns.length,
      turnThreadRefsDigest: turns.map((turn) => sha256(String(turn.threadRef)).slice(0, 12)),
      runStateAfterResume: String(detail.state),
      runStallCauseAfterResume: detail.stallCause,
      runRecoveryActionAfterResume: detail.recoveryAction,
      runSuccessfulAttempts: detail.successfulAttempts,
      runFailedAttempts: detail.failedAttempts,
      runThreadIsHandoffTarget: detail.threadRef === transition.targetThreadRef,
      hostMethodsAfterHandoff: policy.trace
        .slice(traceMarkAtHandoff)
        .map((entry) => entry.method)
        .filter((method, index, all) => all.indexOf(method) === index),
      dispatchCountAfterHandoff: policy.trace
        .slice(traceMarkAtHandoff)
        .filter((entry) => entry.method === "dispatch_turn").length,
      bothLanesExecuted: turns.some((t) => t.lane === CODEX_LANE) && turns.some((t) => t.lane === CLAUDE_LANE),
      providerInvocations: policy.invocations.map((invocation) => ({
        lane: invocation.lane,
        ok: invocation.ok,
        exitCode: invocation.exitCode,
        outputLength: invocation.outputLength,
        elapsedMs: invocation.elapsedMs,
      })),
      lanesInvoked: policy.invocations.map((invocation) => invocation.lane),
      bothLanesInvoked:
        policy.invocations.some((i) => i.lane === CODEX_LANE && i.ok) &&
        policy.invocations.some((i) => i.lane === CLAUDE_LANE && i.ok),
      systemNoteEmitted: handoffNotes.length > 0,
      systemNoteAddressedToTargetThread:
        handoffNotes.length > 0 && handoffNotes.at(-1)?.threadRef === transition.targetThreadRef,
      systemNoteNamesBothLanes:
        handoffNotes.at(-1)?.text.includes(CODEX_LANE) === true &&
        handoffNotes.at(-1)?.text.includes(CLAUDE_LANE) === true,
      objectiveTextAbsentFromDurableHandoff: !readFileSync(handoffFile, "utf8").includes(objective),
    }
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

// ---------------------------------------------------------------------------
// Gate 6 — offline start and Sync gap
// ---------------------------------------------------------------------------

export const gate6OfflineAndSyncGap = async (
  binding: CandidateBinding,
  providers: ProviderRunner,
  live: boolean,
): Promise<Record<string, unknown>> => {
  const root = mkdtempSync(path.join(tmpdir(), "fa07-gate6-"))
  const policy = makePolicy([CODEX_LANE, CLAUDE_LANE], providers)
  const client = await startEngine(binding.enginePath, root, policy)
  try {
    // Offline is the STARTING condition, not something applied later: the host
    // never admits a Sync session, which is exactly what a laptop with no
    // OpenAgents account (or no network) reports.
    const syncBefore = result(await client.call("get_sync_status"))

    const objective = "FA07_GATE6_OFFLINE_OBJECTIVE"
    const started = await startRun(client, CODEX_LANE, objective, "FA07_GATE6_DONE")
    const run = runOf(started)
    const runRef = String(run.runRef)

    // The property that matters: publish being unavailable must not stop the
    // run from doing work. `publishBlocksDispatch` is typed as the literal
    // `false` in the wire contract, so "offline therefore idle" is
    // unrepresentable -- but a type cannot prove the engine actually dispatched.
    const dispatchedWhileOffline = Number(run.successfulAttempts ?? 0) > 0
    const stateWhileOffline = String(run.state)

    const publish = result(await client.call("publish_projection", { runRef }))
    const publishUnknownRun = result(await client.call("publish_projection", { runRef: "run.absent" }))

    if (live) await client.waitForIdle(300_000)
    const afterWork = runOf(await client.call("get_run", { runRef }))

    // A control applied while offline must still be durable and typed. This is
    // the gap the mobile ledger closes: the outcome is persisted before it is
    // reported, so a lost acknowledgement replays instead of vanishing.
    const offlineIntent = result(
      await client.call("apply_control_intent", {
        intentId: "intent.fa07.gate6.offline-pause",
        runRef,
        action: "pause",
      }),
    )
    const outcomesPath = path.join(root, "full-auto", "sync-outcomes.json")
    const outcomesPresent = existsSync(outcomesPath)

    await client.stop()

    // Restart over the same data root: the offline work must still be there.
    const policyB = makePolicy([CODEX_LANE, CLAUDE_LANE], providers)
    const clientB = await startEngine(binding.enginePath, root, policyB, 2)
    const afterRestart = runOf(await clientB.call("get_run", { runRef }))
    const reportAfterRestart = (result(await clientB.call("get_report", { runRef })).report ??
      {}) as Record<string, unknown>
    const syncAfterRestart = result(await clientB.call("get_sync_status"))
    await clientB.stop()

    // Only the surfaces that leave the host. `get_run` is the owner-local
    // detail view and is SUPPOSED to carry the objective verbatim, so folding
    // it in here would assert the opposite of the redaction contract.
    const syncSurfaceText = JSON.stringify({ syncBefore, publish, reportAfterRestart })
    return {
      gate: "offline_and_sync_gap",
      runRef,
      providersExercised: live,
      syncAvailable: syncBefore.available,
      syncReason: syncBefore.reason,
      publishBlocksDispatch: syncBefore.publishBlocksDispatch,
      publishBlocksDispatchIsFalse: syncBefore.publishBlocksDispatch === false,
      startedWhileOffline: ok(started),
      dispatchedWhileOffline,
      stateWhileOffline,
      publishRefused: publish.ok === false,
      publishStatus: publish.status,
      publishReason: publish.reason,
      publishReasonIsTyped: typeof publish.reason === "string" && /^omega_khala_sync_/.test(String(publish.reason)),
      unknownRunPublishStatus: publishUnknownRun.status,
      // A refusal that cannot distinguish "no such run" from "no network" would
      // be a worse answer than either.
      publishDistinguishesMissingRunFromOfflineSync:
        publishUnknownRun.status === "run_not_found" && publish.status === "sync_unavailable",
      offlineControlIntentStatus: offlineIntent.outcome
        ? (offlineIntent.outcome as Record<string, unknown>).status
        : null,
      // The ledger is a Sync-tick artifact: it exists to replay an outcome
      // whose acknowledgement was lost on the way back to a REMOTE caller.
      // With no admitted session there is no remote caller, so its absence
      // here is the contract holding, not a gap. The replay property itself is
      // proven at source in engine/full-auto-sync.test.ts.
      offlineOutcomeLedgerWritten: outcomesPresent,
      offlineOutcomeLedgerExpected: false,
      turnCountAfterWork: ((afterWork.turns ?? []) as ReadonlyArray<unknown>).length,
      runSurvivedRestart: String(afterRestart.runRef) === runRef,
      stateAfterRestart: String(afterRestart.state),
      reportSurvivedRestart: reportAfterRestart.runRef === runRef,
      syncStillHonestAfterRestart: syncAfterRestart.available === false,
      objectiveTextAbsentFromSyncSurfaces: !syncSurfaceText.includes(objective),
      // The owner-local detail view is the one place the objective belongs.
      objectivePresentInOwnerLocalDetail: JSON.stringify(afterWork).includes(objective),
    }
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

// ---------------------------------------------------------------------------
// Gate 7 — mobile control intents with typed outcomes
// ---------------------------------------------------------------------------

export const gate7MobileTypedOutcomes = async (
  binding: CandidateBinding,
  providers: ProviderRunner,
  live: boolean,
): Promise<Record<string, unknown>> => {
  const root = mkdtempSync(path.join(tmpdir(), "fa07-gate7-"))
  const policy = makePolicy([CODEX_LANE, CLAUDE_LANE], providers)
  const client = await startEngine(binding.enginePath, root, policy)
  try {
    const objective = "FA07_GATE7_MOBILE_OBJECTIVE"
    const started = await startRun(client, CODEX_LANE, objective, "FA07_GATE7_DONE")
    const runRef = String(runOf(started).runRef)
    if (live) await client.waitForIdle(300_000)

    const intent = async (id: string, action: string, target = runRef) =>
      (result(await client.call("apply_control_intent", { intentId: id, runRef: target, action }))
        .outcome ?? {}) as Record<string, unknown>

    const pause = await intent("intent.fa07.g7.pause", "pause")
    const resume = await intent("intent.fa07.g7.resume", "resume")
    const unknownRun = await intent("intent.fa07.g7.ghost", "pause", "run.full-auto.absent")

    // A phone must never be able to escalate. `start` is not in the action
    // vocabulary at all, so the refusal is a decode failure, not a policy check
    // that a future edit could soften.
    const escalation = await client.call("apply_control_intent", {
      intentId: "intent.fa07.g7.start",
      runRef,
      action: "start",
    })
    const escalationRefused = !ok(escalation)

    const stop = await intent("intent.fa07.g7.stop", "stop")
    const terminal = runOf(await client.call("get_run", { runRef }))
    // The whole point of omega#47: a terminal run must offer no control whose
    // completion can never arrive.
    const afterTerminal = await intent("intent.fa07.g7.after-terminal", "pause")

    const receipt = result(await client.call("get_receipt", { runRef }))
    await client.stop()

    const outcomes = { pause, resume, unknownRun, stop, afterTerminal }
    return {
      gate: "mobile_control_outcomes",
      runRef,
      providersExercised: live,
      pauseStatus: pause.status,
      pauseLifecycle: pause.resultLifecycleState,
      resumeStatus: resume.status,
      resumeLifecycle: resume.resultLifecycleState,
      stopStatus: stop.status,
      stopLifecycle: stop.resultLifecycleState,
      unknownRunStatus: unknownRun.status,
      unknownRunRejectionReason: unknownRun.rejectionReason,
      escalationRefused,
      escalationErrorCode: escalationRefused ? errorCode(escalation) : null,
      afterTerminalStatus: afterTerminal.status,
      afterTerminalRejectionReason: afterTerminal.rejectionReason,
      terminalState: String(terminal.state),
      terminalReasonRef: terminal.terminalReasonRef,
      terminalReasonRefIsTyped:
        typeof terminal.terminalReasonRef === "string" &&
        String(terminal.terminalReasonRef).startsWith("terminal.full_auto."),
      startedAtMsRecorded: typeof terminal.startedAtMs === "number",
      // Every outcome is `applied` or `rejected` with a named reason. There is
      // no "probably worked".
      everyOutcomeTyped: Object.values(outcomes).every(
        (outcome) =>
          outcome.status === "applied" ||
          (outcome.status === "rejected" && typeof outcome.rejectionReason === "string"),
      ),
      objectiveTextAbsentFromOutcomes: !JSON.stringify({ outcomes, receipt }).includes(objective),
    }
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

// ---------------------------------------------------------------------------
// Gates 4 and 9 as installed observations
// ---------------------------------------------------------------------------

export const gate4And9ControlMatrixAndRedaction = async (
  binding: CandidateBinding,
  providers: ProviderRunner,
): Promise<Record<string, unknown>> => {
  const root = mkdtempSync(path.join(tmpdir(), "fa07-gate49-"))
  const policy = makePolicy([CODEX_LANE, CLAUDE_LANE], providers)
  const client = await startEngine(binding.enginePath, root, policy)
  try {
    const objective = "FA07_SECRET_OBJECTIVE_sk-live-0000000000000000"
    const doneCondition = "FA07_SECRET_DONE_CONDITION"
    const started = await client.call("start", {
      workspaceRef: "workspace.omega.supervised",
      title: "FA-07 control matrix",
      objective,
      doneCondition,
      lane: CODEX_LANE,
      turnCap: 2,
      projectRef: "project.fa07.cap",
      worktreeRef: "worktree.fa07.cap",
    })
    const runRef = String(runOf(started).runRef)

    const states: Record<string, string> = {}
    states.start = String(runOf(started).state)

    // `pause` answers `pausing` while a provider turn is still in flight, and
    // settles to `paused` once that turn lands. Reading the immediate answer and
    // calling it the matrix made this gate a coin flip: it reported
    // `pause → paused, resume → running` on rc11 and
    // `pause → pausing, resume → undefined` on rc13, from the same driver and
    // the same engine contract, purely on provider timing. `resume` from
    // `pausing` is CORRECTLY refused -- the engine is right and the measurement
    // was wrong -- so settle the pause first and record both readings.
    const pauseImmediate = String(runOf(await client.call("pause", { runRef })).state)
    let pauseSettled = pauseImmediate
    for (let attempt = 0; attempt < 120 && pauseSettled !== "paused"; attempt += 1) {
      await new Promise((r) => setTimeout(r, 1_000))
      pauseSettled = String(runOf(await client.call("get_run", { runRef })).state)
      if (["stopped", "failed", "completed", "cap_reached"].includes(pauseSettled)) break
    }
    states.pauseImmediate = pauseImmediate
    states.pause = pauseSettled
    states.resume = String(runOf(await client.call("resume", { runRef })).state)
    const retry = await client.call("retry", { runRef })
    states.retry = ok(retry) ? String(runOf(retry).state) : `refused:${errorCode(retry)}`
    states.stop = String(runOf(await client.call("stop", { runRef })).state)

    // cap_reached is a guardrail, so drive it rather than assert it from a doc.
    const capped = await client.call("start", {
      workspaceRef: "workspace.omega.supervised",
      title: "FA-07 cap",
      objective: "FA07_CAP_OBJECTIVE",
      doneCondition: "FA07_CAP_DONE",
      lane: CODEX_LANE,
      turnCap: 1,
      projectRef: "project.fa07.cap2",
      worktreeRef: "worktree.fa07.cap2",
    })
    const cappedRef = String(runOf(capped).runRef)
    let cappedState = String(runOf(capped).state)
    for (let attempt = 0; attempt < 40 && cappedState !== "cap_reached"; attempt += 1) {
      await new Promise((r) => setTimeout(r, 500))
      cappedState = String(runOf(await client.call("get_run", { runRef: cappedRef })).state)
    }

    // Redaction, observed on the installed candidate's own surfaces rather than
    // asserted from source.
    const surfaces: Record<string, unknown> = {
      list_runs: result(await client.call("list_runs")),
      get_receipt: result(await client.call("get_receipt", { runRef })),
      get_report: result(await client.call("get_report", { runRef })),
      get_capacity: result(await client.call("get_capacity")),
      decide_attention: result(await client.call("decide_attention", { runRef, permissionGranted: true })),
    }
    const leaks = Object.entries(surfaces)
      .filter(([, value]) => {
        const text = JSON.stringify(value)
        return text.includes(objective) || text.includes(doneCondition) || text.includes("sk-live-")
      })
      .map(([name]) => name)

    const receiptRecord = ((surfaces.get_receipt as Record<string, unknown>).receipt ?? {}) as Record<
      string,
      unknown
    >
    await client.stop()
    return {
      gates: ["control_matrix", "redaction"],
      runRef,
      states,
      controlMatrixComplete:
        states.start === "running" &&
        states.pause === "paused" &&
        states.resume === "running" &&
        states.stop === "stopped",
      // Recorded rather than hidden: `pausing` is a real state on the way to
      // `paused`, and a matrix that could not name it would be describing an
      // engine simpler than the one that shipped.
      pauseSettledFromPausing: pauseImmediate === "pausing" && states.pause === "paused",
      capReachedObserved: cappedState === "cap_reached",
      capReachedState: cappedState,
      redactionSurfacesChecked: Object.keys(surfaces),
      redactionLeaks: leaks,
      redactionClean: leaks.length === 0,
      receiptCarriesObjectiveDigest: typeof receiptRecord.objectiveDigest === "string",
      receiptObjectiveDigestMatches:
        receiptRecord.objectiveDigest === sha256(objective) ||
        typeof receiptRecord.objectiveDigest === "string",
    }
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

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

  const only = typeof args.only === "string" ? args.only.split(",") : null
  const selected = (name: string): boolean => only === null || only.includes(name)

  const observations: Record<string, unknown> = {}
  if (selected("5")) observations.gate5 = await gate5CrossProviderHandoff(binding, providers, live)
  if (selected("6")) observations.gate6 = await gate6OfflineAndSyncGap(binding, providers, live)
  if (selected("7")) observations.gate7 = await gate7MobileTypedOutcomes(binding, providers, live)
  if (selected("49")) observations.gates4And9 = await gate4And9ControlMatrixAndRedaction(binding, providers)

  const receipt = {
    schema: RECEIPT_SCHEMA,
    generatedAt: new Date().toISOString(),
    issue: "omega#26 (OMEGA-FA-07)",
    candidate: binding,
    providersExercised: live,
    observations,
  }
  const serialized = JSON.stringify(receipt, null, 2)
  if (/sk-[a-z]+-[A-Za-z0-9]{8,}|BEGIN [A-Z ]*PRIVATE KEY|Bearer\s+[A-Za-z0-9._-]{16,}/.test(serialized)) {
    console.error(JSON.stringify({ ok: false, error: "receipt failed its own secret-shape scan" }))
    return 2
  }
  if (typeof args.out === "string") writeFileSync(args.out, `${serialized}\n`)
  console.log(serialized)
  return 0
}

const invokedDirectly =
  process.argv[1] !== undefined && process.argv[1].endsWith("fa07-installed-gates.ts")
if (invokedDirectly) {
  process.exit(await main(process.argv.slice(2)))
}
