/**
 * ACP-10 (#8897): opt-in packaged Grok/Cursor failure/recovery proof.
 *
 * The first packaged process starts a real ACP Full Auto turn, then exits
 * while it is running. A second packaged process reuses the isolated Desktop
 * userData and workspace: startup recovery must durably interrupt the old
 * non-replayable turn and the same enabled thread must complete a new real
 * continuation before being disabled. Only a closed, redacted
 * receipt is retained. The runner never changes HOME or login/keychain state.
 */
import { execFile } from "node:child_process"
import { existsSync } from "node:fs"
import { mkdir, mkdtemp, realpath, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import path from "node:path"
import { promisify } from "node:util"

import {
  controlOperations,
  readControlConnection,
} from "./full-auto-control-client.ts"

const execFileAsync = promisify(execFile)
const armed = process.env.ACP_DESKTOP_RELEASE_LIVE === "1"
if (!armed) {
  process.stderr.write(
    "Set ACP_DESKTOP_RELEASE_LIVE=1 to run the packaged ACP failure/recovery proof.\n",
  )
  process.exit(2)
}

const provider = process.env.ACP_DESKTOP_RELEASE_PEER ?? "cursor"
if (provider !== "grok" && provider !== "cursor") {
  process.stderr.write("ACP_DESKTOP_RELEASE_PEER must be grok or cursor.\n")
  process.exit(2)
}
const laneRef = provider === "grok" ? "acp:grok-cli" : "acp:cursor-agent"
const providerLabel = provider === "grok" ? "Grok" : "Cursor"

if (process.platform !== "darwin" || process.arch !== "arm64") {
  process.stderr.write("The checked packaged ACP release proof currently targets darwin-arm64.\n")
  process.exit(2)
}

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const packagedBinary = path.join(
  appRoot,
  "out",
  "OpenAgents-darwin-arm64",
  "OpenAgents.app",
  "Contents",
  "MacOS",
  "OpenAgents",
)
if (!existsSync(packagedBinary)) {
  process.stderr.write("Build the packaged Desktop app before running the ACP release proof.\n")
  process.exit(2)
}

const output = process.env.ACP_DESKTOP_RELEASE_OUTPUT
const checkedLiveDirectory = path.resolve(
  repositoryRoot,
  "packages/agent-client-protocol-conformance/compatibility/live",
)
if (
  output === undefined ||
  !path.isAbsolute(output) ||
  !path.resolve(output).startsWith(`${checkedLiveDirectory}${path.sep}`) ||
  !output.endsWith(".json")
) {
  process.stderr.write(
    "ACP_DESKTOP_RELEASE_OUTPUT must be an absolute .json path beneath the checked compatibility/live directory.\n",
  )
  process.exit(2)
}

type Lane = Readonly<{
  laneRef: string
  configuration: string
  authentication: string
  admission: string
  reason?: string | null
}>
type Status = Readonly<{
  record?: Readonly<{
    enabled?: boolean
    continuationCount?: number
    blockedReason?: string | null
    live?: Readonly<{ state?: string; detail?: string }>
  }>
}>
type Turns = Readonly<{
  turns?: ReadonlyArray<Readonly<{ phase?: string; disposition?: string }>>
}>

const sleep = (milliseconds: number) => new Promise(resolve => setTimeout(resolve, milliseconds))
const deadline = async <Value>(
  timeoutMs: number,
  poll: () => Promise<Value | undefined>,
): Promise<Value> => {
  const expires = Date.now() + timeoutMs
  while (Date.now() < expires) {
    const value = await poll()
    if (value !== undefined) return value
    await sleep(250)
  }
  throw new Error("bounded packaged ACP proof deadline elapsed")
}

const root = await mkdtemp(path.join(tmpdir(), "openagents-acp-desktop-release-"))
const workspace = path.join(root, "workspace")
const userData = path.join(root, "user-data")
await Promise.all([
  mkdir(workspace, { recursive: true, mode: 0o700 }),
  mkdir(userData, { recursive: true, mode: 0o700 }),
])
await execFileAsync("git", ["init", "--quiet", workspace])
await execFileAsync("git", ["-C", workspace, "config", "user.name", "OpenAgents ACP Proof"])
await execFileAsync("git", ["-C", workspace, "config", "user.email", "acp-proof@openagents.local"])
await writeFile(path.join(workspace, "README.md"), "# Disposable packaged ACP proof\n", {
  mode: 0o600,
})
await execFileAsync("git", ["-C", workspace, "add", "README.md"])
await execFileAsync("git", ["-C", workspace, "commit", "--quiet", "-m", "seed"])

type RunningApp = Readonly<{
  exit: Promise<number | null>
  stop: () => Promise<void>
}>

const launch = async (home: string): Promise<RunningApp> => {
  const { spawn } = await import("node:child_process")
  const child = spawn(packagedBinary, [], {
    cwd: appRoot,
    env: {
      ...process.env,
      HOME: home,
      OPENAGENTS_DESKTOP_USER_DATA: userData,
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_ISOLATED_WORKSPACE_ROOT: workspace,
      OPENAGENTS_DESKTOP_LAUNCH_CWD: workspace,
      OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL: "1",
      OPENAGENTS_DESKTOP_ACP_RELEASE_PROOF: "1",
    },
    stdio: ["ignore", "ignore", "ignore"],
  })
  const exit = new Promise<number | null>((resolve, reject) => {
    child.once("error", reject)
    child.once("exit", code => resolve(code))
  })
  await deadline(120_000, async () => {
    try {
      const connection = readControlConnection(userData)
      const response = await controlOperations(connection).lanes()
      return response.status === 200 ? true : undefined
    } catch {
      return undefined
    }
  })
  return {
    exit,
    stop: async () => {
      child.kill("SIGTERM")
      const stopped = await Promise.race([exit.then(() => true), sleep(15_000).then(() => false)])
      if (!stopped) {
        child.kill("SIGKILL")
        await exit
      }
    },
  }
}

const operations = () => controlOperations(readControlConnection(userData))
const selectedLane = async (): Promise<Lane> => {
  const response = await operations().lanes()
  const body = response.body as { lanes?: ReadonlyArray<Lane> }
  const lane = body.lanes?.find(candidate => candidate.laneRef === laneRef)
  if (response.status !== 200 || lane === undefined) throw new Error(`${providerLabel} lane was not listed`)
  return lane
}
const status = async (threadRef: string): Promise<Status> => {
  const response = await operations().status(threadRef)
  if (response.status !== 200) throw new Error(`${providerLabel} proof thread status failed`)
  return response.body as Status
}
const turns = async (threadRef: string): Promise<Turns> => {
  const response = await operations().turns(threadRef)
  if (response.status !== 200) throw new Error(`${providerLabel} proof turns query failed`)
  return response.body as Turns
}

let first: RunningApp | undefined
let second: RunningApp | undefined
let third: RunningApp | undefined
try {
  const ordinaryHome = process.env.HOME
  if (ordinaryHome === undefined || ordinaryHome.length === 0)
    throw new Error("Ordinary HOME is required for the authenticated packaged proof")
  first = await launch(ordinaryHome)
  const initialLane = await selectedLane()
  if (initialLane.configuration !== "configured" || initialLane.admission !== "admitted")
    throw new Error(
      `Pinned ${providerLabel} lane was not admitted in packaged Desktop (${initialLane.configuration}/${initialLane.admission}/${initialLane.authentication}/${initialLane.reason ?? "no-reason"})`,
    )
  const refused = await operations().start(
    `${workspace}-not-granted`,
    "ACP packaged restart recovery",
    laneRef,
  )
  const refusedBody = refused.body as { error?: unknown; resolvedWorkspaceRef?: unknown }
  if (refused.status !== 409 || refusedBody.error !== "workspace_mismatch")
    throw new Error("Packaged Desktop did not refuse a mismatched workspace")
  const resolvedWorkspace = refusedBody.resolvedWorkspaceRef
  if (
    typeof resolvedWorkspace !== "string" ||
    (await realpath(resolvedWorkspace)) !== (await realpath(workspace))
  )
    throw new Error("Packaged Desktop resolved a different workspace")
  const controlWorkspace = resolvedWorkspace
  const started = await operations().start(
    controlWorkspace,
    "ACP packaged restart recovery",
    laneRef,
  )
  const startedBody = started.body as { record?: { threadRef?: string } }
  const threadRef = startedBody.record?.threadRef
  if (started.status !== 200 || typeof threadRef !== "string")
    throw new Error(
      `Packaged ${providerLabel} proof did not start (${started.status}/${String((started.body as { error?: unknown }).error ?? "no-error")})`,
    )
  await deadline(120_000, async () => {
    const current = await status(threadRef)
    return current.record?.live?.state === "turn_running" ? true : undefined
  })
  await first.stop()
  first = undefined

  second = await launch(ordinaryHome)
  const recoveredLane = await selectedLane()
  await deadline(60_000, async () => {
    const current = await turns(threadRef)
    return current.turns?.some(
      turn => turn.phase === "interrupted_by_restart" && turn.disposition === "interrupted_by_restart",
    ) ? true : undefined
  })
  const reenabled = await operations().enable(threadRef, controlWorkspace, laneRef)
  if (reenabled.status !== 200) throw new Error(`Packaged ${providerLabel} recovery could not re-enable the thread`)
  const continued = await operations().continueNow(threadRef)
  if (continued.status !== 200) throw new Error(`Packaged ${providerLabel} recovery was not scheduled`)
  await deadline(180_000, async () => {
    const current = await status(threadRef)
    return current.record?.live?.state === "turn_completed" ||
      current.record?.live?.state === "turn_failed" ||
      current.record?.live?.state === "blocked"
      ? current
      : undefined
  })
  let recoveredTurns = await turns(threadRef)
  const interruptedTurnObserved = recoveredTurns.turns?.some(
    turn => turn.phase === "interrupted_by_restart" && turn.disposition === "interrupted_by_restart",
  ) ?? false
  let recoveredSameThread = recoveredTurns.turns?.some(
    turn => turn.phase === "completed" && turn.disposition === "completed",
  ) ?? false
  let additionalProcessRestartAfterFailedRetry = false
  if (!recoveredSameThread) {
    const stoppedFailedThread = await operations().disable(threadRef)
    if (stoppedFailedThread.status !== 200)
      throw new Error(`Packaged ${providerLabel} failed thread could not be disabled`)
    await second.stop()
    second = undefined
    third = await launch(ordinaryHome)
    additionalProcessRestartAfterFailedRetry = true
    const retryEnabled = await operations().enable(threadRef, controlWorkspace, laneRef)
    if (retryEnabled.status !== 200)
      throw new Error(`Packaged ${providerLabel} retry could not re-enable the original thread`)
    const retryScheduled = await operations().continueNow(threadRef)
    if (retryScheduled.status !== 200)
      throw new Error(`Packaged ${providerLabel} retry was not scheduled`)
    const retryTerminal = await deadline(180_000, async () => {
      const current = await status(threadRef)
      return current.record?.live?.state === "turn_completed" ||
        current.record?.live?.state === "turn_failed" ||
        current.record?.live?.state === "blocked"
        ? current
        : undefined
    })
    recoveredTurns = await turns(threadRef)
    if (!recoveredTurns.turns?.some(
      turn => turn.phase === "completed" && turn.disposition === "completed",
    )) {
      const [currentTurns, currentLane] = await Promise.all([
        turns(threadRef),
        selectedLane(),
      ])
      const states = currentTurns.turns?.map(
        turn => `${turn.phase ?? "none"}/${turn.disposition ?? "none"}`,
      ) ?? []
      throw new Error(
        `Packaged ${providerLabel} retry did not complete (${retryTerminal.record?.live?.state ?? "none"}/${retryTerminal.record?.live?.detail ?? "no-detail"};${currentLane.authentication};${states.join(",")})`,
      )
    }
    recoveredSameThread = true
  }
  const completedTurnObserved = recoveredTurns.turns?.some(
    turn => turn.phase === "completed" && turn.disposition === "completed",
  ) ?? false
  if (!completedTurnObserved) throw new Error(`Packaged ${providerLabel} recovery did not settle durably`)
  const disabled = await operations().disable(threadRef)
  const disabledBody = disabled.body as Status
  if (disabled.status !== 200 || disabledBody.record?.enabled !== false)
    throw new Error(`Packaged ${providerLabel} proof did not disable durably`)

  const revision = (
    await execFileAsync("git", ["-C", repositoryRoot, "rev-parse", "HEAD"], { maxBuffer: 4_096 })
  ).stdout.trim()
  const artifact = {
    format: "openagents-acp-desktop-release-run-v1",
    protocol: "Agent Client Protocol",
    protocolExclusions: ["Agent Communication Protocol", "A2A"],
    proofClass: "candidate-packaged-desktop-live",
    claimAuthority: "none-release-matrix-only",
    recordedAt: new Date().toISOString(),
    openAgentsRevision: revision,
    platform: `${process.platform}-${process.arch}-node-${process.versions.node}`,
    provider,
    lane: laneRef,
    packaged: true,
    interruption: {
      mismatchedWorkspaceRefused: true,
      laneConfigured: initialLane.configuration === "configured",
      laneAdmitted: initialLane.admission === "admitted",
      exitedDuringRunningTurn: true,
    },
    recovery: {
      reusedDesktopState: true,
      explicitlyReenabledSameThread: true,
      recoveredSameThread,
      freshThreadRetryAfterFailure: false,
      additionalProcessRestartAfterFailedRetry,
      laneConfigured: recoveredLane.configuration === "configured",
      interruptedTurnSettled: interruptedTurnObserved,
      durableCompletedTurn: completedTurnObserved,
      disabled: true,
    },
    redaction: {
      promptTextRetained: false,
      responseTextRetained: false,
      threadIdentifiersRetained: false,
      authMaterialRetained: false,
      absolutePathsRetained: false,
    },
  } as const
  await writeFile(output, `${JSON.stringify(artifact, null, 2)}\n`, { mode: 0o600 })
  process.stdout.write(`${JSON.stringify(artifact, null, 2)}\n`)
} finally {
  await first?.stop().catch(() => undefined)
  await second?.stop().catch(() => undefined)
  await third?.stop().catch(() => undefined)
  await rm(root, { recursive: true, force: true })
}
