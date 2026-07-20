import { execFileSync, spawn } from "node:child_process"
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { chromium, type Browser, type Page } from "playwright"
import { Schema } from "effect"

import {
  IdePortableClientCommandResultSchema,
  IdePortableClientSnapshotSchema,
} from "../src/ide/portable-client-contract.ts"
import {
  packagedArtifactTreeDigest,
  resolvePackagedApp,
} from "./ide-packaged-artifact.ts"

const appRoot = path.resolve(import.meta.dirname, "..")
const repositoryRoot = path.resolve(appRoot, "../..")
const evidenceRoot = path.join(appRoot, "benchmarks", "ide")
const receiptPath = path.join(
  evidenceRoot,
  "2026-07-20-ide-13-portability-packaged.json",
)
const screenshotPath = path.join(
  evidenceRoot,
  "2026-07-20-ide-13-portability-packaged.png",
)
const tracePath = path.join(
  evidenceRoot,
  "2026-07-20-ide-13-portability-packaged-trace.json",
)
const screenshotRef =
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-portability-packaged.png"
const traceRef =
  "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-portability-packaged-trace.json"

const Sha40 = Schema.String.check(Schema.isPattern(/^[a-f0-9]{40}$/u))
const Sha256 = Schema.String.check(Schema.isPattern(/^[a-f0-9]{64}$/u))
const BoundedText = Schema.String.check(Schema.isMaxLength(1_000))
const PublicRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._/-]*$/u),
)
const ArtifactRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(512),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._/ -]*$/u),
)

const PackagedReceiptSchema = Schema.Struct({
  schemaVersion: Schema.Literal(
    "openagents.desktop.ide-portability-packaged-fail-closed.v1",
  ),
  issue: Schema.Literal("IDE-13"),
  proofClass: Schema.Literal("packaged_fail_closed_unavailable"),
  candidateCommitSha: Sha40,
  capturedAt: Schema.String,
  target: Schema.Literal("darwin-arm64"),
  artifact: Schema.Struct({
    ref: ArtifactRef,
    treeSha256: Sha256,
    files: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
    bytes: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  }),
  environment: Schema.Struct({
    platform: Schema.Literal("darwin"),
    architecture: Schema.Literal("arm64"),
    node: Schema.String,
    electron: Schema.String,
  }),
  projection: Schema.Struct({
    phase: Schema.Literal("unavailable"),
    cursor: Schema.Null,
    pendingCommandCount: Schema.Literal(0),
    sessionCount: Schema.Literal(0),
    targetDirectoryCount: Schema.Literal(0),
    attachmentCount: Schema.Literal(0),
    commandCount: Schema.Literal(0),
    issueCount: Schema.Literal(0),
  }),
  invalidCommand: Schema.Struct({
    tag: Schema.Literal("Refused"),
    reason: Schema.Literal("invalid_input"),
  }),
  visibleChecks: Schema.Struct({
    portableSurfaceVisible: Schema.Literal(true),
    confirmedSyncUnavailableVisible: Schema.Literal(true),
    noConfirmedSessionVisible: Schema.Literal(true),
    zeroQueuedVisible: Schema.Literal(true),
  }),
  network: Schema.Struct({
    declaredDestinations: Schema.Array(Schema.String).check(Schema.isMaxLength(0)),
    observedRendererExternalDestinations: Schema.Array(Schema.String).check(
      Schema.isMaxLength(0),
    ),
    observationScope: Schema.Literal(
      "renderer_cdp_after_attach_plus_resource_timing",
    ),
    mainProcessGlobalNetworkClaimed: Schema.Literal(false),
  }),
  security: Schema.Struct({
    forbiddenMaterialFound: Schema.Literal(false),
    workspacePathProjected: Schema.Literal(false),
    profilePathProjected: Schema.Literal(false),
    homePathProjected: Schema.Literal(false),
    credentialShapeProjected: Schema.Literal(false),
  }),
  diagnostics: Schema.Struct({
    consoleErrorCount: Schema.Literal(0),
    pageErrorCount: Schema.Literal(0),
  }),
  teardown: Schema.Struct({
    applicationPidCaptured: Schema.Literal(true),
    descendantCountBeforeStop: Schema.Number.check(
      Schema.isInt(),
      Schema.isGreaterThanOrEqualTo(0),
    ),
    survivingProcessCount: Schema.Literal(0),
    termination: Schema.Literals(["sigterm", "sigkill_fallback"]),
  }),
  screenshotRef: PublicRef,
  traceRef: PublicRef,
  authenticatedSyncClaimed: Schema.Literal(false),
  moveClaimed: Schema.Literal(false),
  passed: Schema.Literal(true),
  limitations: Schema.Array(BoundedText).check(
    Schema.isMinLength(1),
    Schema.isMaxLength(8),
  ),
})

const PackagedTraceSchema = Schema.Struct({
  schemaVersion: Schema.Literal(
    "openagents.desktop.ide-portability-packaged-fail-closed-trace.v1",
  ),
  issue: Schema.Literal("IDE-13"),
  candidateCommitSha: Sha40,
  artifactTreeSha256: Sha256,
  events: Schema.Array(Schema.Struct({
    kind: Schema.String.check(Schema.isMaxLength(80)),
    message: Schema.String.check(Schema.isMaxLength(500)),
  })).check(Schema.isMaxLength(500)),
  observedRendererExternalDestinations: Schema.Array(Schema.String).check(
    Schema.isMaxLength(0),
  ),
  privateMaterialIncluded: Schema.Literal(false),
  authenticatedSyncClaimed: Schema.Literal(false),
  moveClaimed: Schema.Literal(false),
})

const git = (...args: ReadonlyArray<string>): string =>
  execFileSync("git", [...args], {
    cwd: repositoryRoot,
    encoding: "utf8",
  }).trim()

const waitFor = async (
  predicate: () => boolean,
  failure: string,
  timeoutMs = 30_000,
): Promise<void> => {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (predicate()) return
    await new Promise(resolve => setTimeout(resolve, 50))
  }
  throw new Error(failure)
}

const waitForRenderer = async (browser: Browser): Promise<Page> => {
  const deadline = Date.now() + 30_000
  while (Date.now() < deadline) {
    const page = browser.contexts().flatMap(context => context.pages())
      .find(candidate => candidate.url().startsWith("openagents-app://renderer/"))
    if (page !== undefined) return page
    await new Promise(resolve => setTimeout(resolve, 50))
  }
  throw new Error("IDE-13 packaged renderer did not appear")
}

const processTable = (): ReadonlyArray<Readonly<{ pid: number; parentPid: number }>> =>
  execFileSync("ps", ["-axo", "pid=,ppid="], { encoding: "utf8" })
    .split("\n")
    .map(line => line.trim().split(/\s+/u))
    .filter(parts => parts.length === 2)
    .map(parts => ({
      pid: Number.parseInt(parts[0] ?? "", 10),
      parentPid: Number.parseInt(parts[1] ?? "", 10),
    }))
    .filter(value => Number.isSafeInteger(value.pid) && Number.isSafeInteger(value.parentPid))

const descendantPids = (rootPid: number): ReadonlyArray<number> => {
  const rows = processTable()
  const descendants = new Set<number>()
  let frontier = [rootPid]
  while (frontier.length > 0) {
    const parents = new Set(frontier)
    frontier = rows
      .filter(row => parents.has(row.parentPid) && !descendants.has(row.pid))
      .map(row => row.pid)
    for (const pid of frontier) descendants.add(pid)
  }
  return [...descendants]
}

const isRunning = (pid: number): boolean => {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

const destinationFor = (value: string): string | null => {
  try {
    const url = new URL(value)
    if (!["http:", "https:", "ws:", "wss:"].includes(url.protocol)) return null
    if (["127.0.0.1", "localhost", "::1"].includes(url.hostname)) return null
    return `${url.protocol}//${url.host}`
  } catch {
    return null
  }
}

const main = async (): Promise<void> => {
  if (process.platform !== "darwin" || process.arch !== "arm64") {
    throw new Error("IDE-13 packaged evidence requires macOS arm64")
  }
  const workspace = mkdtempSync(path.join(tmpdir(), "openagents-ide13-workspace-"))
  const profile = mkdtempSync(path.join(tmpdir(), "openagents-ide13-profile-"))
  const sourcePath = path.join(workspace, "portable-proof.txt")
  writeFileSync(sourcePath, "packaged fail-closed portability proof\n", { mode: 0o600 })

  const candidateCommitSha = git("rev-parse", "HEAD")
  const appPath = resolvePackagedApp()
  const artifact = packagedArtifactTreeDigest(appPath)
  const artifactRef = path.relative(repositoryRoot, appPath).split(path.sep).join("/")
  const packageJson = JSON.parse(
    readFileSync(path.join(appRoot, "package.json"), "utf8"),
  ) as Readonly<{ version?: unknown }>
  const electronJson = JSON.parse(
    readFileSync(path.join(repositoryRoot, "node_modules", "electron", "package.json"), "utf8"),
  ) as Readonly<{ version?: unknown }>
  const events: Array<{ kind: string; message: string }> = []
  const observedExternalDestinations = new Set<string>()
  const forbiddenCredential =
    /(?:github_pat|gh[pousr]_|sk-|AKIA|xox[baprs]-)[A-Za-z0-9_-]{8,}|(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*/gu
  const containsCredential = (value: string): boolean => {
    forbiddenCredential.lastIndex = 0
    return forbiddenCredential.test(value)
  }
  const home = process.env.HOME ?? "__no_home__"
  const sanitize = (value: string): string => value
    .replaceAll(workspace, "«workspace»")
    .replaceAll(profile, "«profile»")
    .replaceAll(home, "«home»")
    .replace(forbiddenCredential, "«redacted»")
    .slice(0, 500)

  const appProcess = spawn("open", [
    "-n",
    "-W",
    "-a",
    appPath,
    sourcePath,
    "--args",
    "--remote-debugging-port=0",
  ], {
    cwd: workspace,
    env: {
      ...process.env,
      OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF: "1",
      OPENAGENTS_DESKTOP_USER_DATA: profile,
      OPENAGENTS_DESKTOP_LAUNCH_CWD: workspace,
      OA_DESKTOP_SKIP_DEV_VOICE_HELPER: "1",
    },
    stdio: "ignore",
  })

  let browser: Browser | null = null
  let applicationPid: number | null = null
  let processIds: ReadonlyArray<number> = []
  let termination: "sigterm" | "sigkill_fallback" = "sigterm"
  let receiptInput: Omit<typeof PackagedReceiptSchema.Type, "teardown"> | null = null
  let traceInput: typeof PackagedTraceSchema.Type | null = null
  try {
    const portPath = path.join(profile, "DevToolsActivePort")
    await waitFor(
      () => existsSync(portPath),
      "IDE-13 packaged DevTools port did not appear",
    )
    const port = readFileSync(portPath, "utf8").split("\n")[0] ?? ""
    const pidText = execFileSync(
      "lsof",
      ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"],
      { encoding: "utf8" },
    ).trim().split("\n")[0] ?? ""
    applicationPid = Number.parseInt(pidText, 10)
    if (!Number.isSafeInteger(applicationPid)) {
      throw new Error("IDE-13 packaged application PID is unavailable")
    }
    processIds = [applicationPid, ...descendantPids(applicationPid)]

    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`)
    const page = await waitForRenderer(browser)
    page.on("console", message => {
      const kind = `console:${message.type()}`
      events.push({ kind, message: sanitize(message.text()) })
    })
    page.on("pageerror", error => {
      events.push({ kind: "pageerror", message: sanitize(error.message) })
    })
    page.on("request", request => {
      const destination = destinationFor(request.url())
      if (destination !== null) observedExternalDestinations.add(destination)
    })

    await page.locator('[data-react-workspace="files"]')
      .waitFor({ state: "visible", timeout: 30_000 })
    const surface = page.getByLabel("Portable coding placement")
    await surface.waitFor({ state: "visible", timeout: 30_000 })
    const surfaceText = await surface.textContent() ?? ""
    const phase = await surface.getAttribute("data-phase")
    const bridge = await page.evaluate(async () => {
      const api = (globalThis as unknown as Readonly<{
        openagentsDesktop?: Readonly<{
          idePortability?: Readonly<{
            snapshot?: () => Promise<unknown>
            command?: (value: unknown) => Promise<unknown>
          }>
        }>
      }>).openagentsDesktop?.idePortability
      return {
        snapshot: await api?.snapshot?.(),
        invalidCommand: await api?.command?.({ invalid: true }),
        resourceUrls: performance.getEntriesByType("resource").map(entry => entry.name),
      }
    })
    for (const resourceUrl of bridge.resourceUrls) {
      const destination = destinationFor(resourceUrl)
      if (destination !== null) observedExternalDestinations.add(destination)
    }

    const snapshot = Schema.decodeUnknownSync(IdePortableClientSnapshotSchema)(
      bridge.snapshot,
    )
    const invalidCommand = Schema.decodeUnknownSync(
      IdePortableClientCommandResultSchema,
    )(bridge.invalidCommand)
    if (phase !== "unavailable" ||
        snapshot.status.phase !== "unavailable" ||
        snapshot.status.cursor !== null ||
        snapshot.status.pendingCommandCount !== 0 ||
        snapshot.sessions.length !== 0 ||
        snapshot.targetDirectories.length !== 0 ||
        snapshot.attachments.length !== 0 ||
        snapshot.commands.length !== 0 ||
        snapshot.issues.length !== 0 ||
        invalidCommand._tag !== "Refused" ||
        invalidCommand.reason !== "invalid_input") {
      throw new Error("IDE-13 packaged preload/main boundary did not fail closed")
    }
    if (!surfaceText.includes("Unavailable until confirmed Sync is live") ||
        !surfaceText.includes("No confirmed portable session is attached.") ||
        !surfaceText.includes("0 queued")) {
      throw new Error("IDE-13 packaged unavailable placement truth is not visible")
    }
    if (observedExternalDestinations.size > 0) {
      throw new Error(
        `IDE-13 packaged renderer used undeclared network: ${JSON.stringify([...observedExternalDestinations])}`,
      )
    }
    const publicText = `${surfaceText}\n${JSON.stringify({ snapshot, invalidCommand })}`
    const securityFindings = {
      forbiddenMaterialFound: containsCredential(publicText),
      workspacePathProjected: publicText.includes(workspace),
      profilePathProjected: publicText.includes(profile),
      homePathProjected: publicText.includes(home),
      credentialShapeProjected: containsCredential(publicText),
    }
    if (Object.values(securityFindings).some(Boolean)) {
      throw new Error("IDE-13 packaged portability projection contains private material")
    }
    const security = {
      forbiddenMaterialFound: false,
      workspacePathProjected: false,
      profilePathProjected: false,
      homePathProjected: false,
      credentialShapeProjected: false,
    } as const

    await surface.screenshot({ path: screenshotPath })
    const consoleErrors = events.filter(event => event.kind === "console:error")
    const pageErrors = events.filter(event => event.kind === "pageerror")
    if (consoleErrors.length > 0 || pageErrors.length > 0) {
      throw new Error(
        `IDE-13 packaged renderer diagnostics failed: ${JSON.stringify({ consoleErrors, pageErrors })}`,
      )
    }

    receiptInput = {
      schemaVersion: "openagents.desktop.ide-portability-packaged-fail-closed.v1",
      issue: "IDE-13",
      proofClass: "packaged_fail_closed_unavailable",
      candidateCommitSha,
      capturedAt: new Date().toISOString(),
      target: "darwin-arm64",
      artifact: {
        ref: artifactRef,
        treeSha256: artifact.sha256,
        files: artifact.files,
        bytes: artifact.bytes,
      },
      environment: {
        platform: "darwin",
        architecture: "arm64",
        node: process.version,
        electron: String(electronJson.version ?? packageJson.version ?? "unknown"),
      },
      projection: {
        phase: "unavailable",
        cursor: null,
        pendingCommandCount: 0,
        sessionCount: 0,
        targetDirectoryCount: 0,
        attachmentCount: 0,
        commandCount: 0,
        issueCount: 0,
      },
      invalidCommand: { tag: "Refused", reason: "invalid_input" },
      visibleChecks: {
        portableSurfaceVisible: true,
        confirmedSyncUnavailableVisible: true,
        noConfirmedSessionVisible: true,
        zeroQueuedVisible: true,
      },
      network: {
        declaredDestinations: [],
        observedRendererExternalDestinations: [],
        observationScope: "renderer_cdp_after_attach_plus_resource_timing",
        mainProcessGlobalNetworkClaimed: false,
      },
      security,
      diagnostics: { consoleErrorCount: 0, pageErrorCount: 0 },
      screenshotRef,
      traceRef,
      authenticatedSyncClaimed: false,
      moveClaimed: false,
      passed: true,
      limitations: [
        "The isolated packaged app has no authenticated Sync authority.",
        "This journey proves the packaged fail-closed projection and IPC boundary only. It does not prove a move or failback.",
        "Network observation covers renderer CDP requests after attachment and renderer resource timing. It does not claim global main-process packet capture.",
        "Only macOS arm64 ran. Windows, Linux, iOS, Android, and real remote placements remain unclaimed.",
      ],
    }
    traceInput = {
      schemaVersion: "openagents.desktop.ide-portability-packaged-fail-closed-trace.v1",
      issue: "IDE-13",
      candidateCommitSha,
      artifactTreeSha256: artifact.sha256,
      events,
      observedRendererExternalDestinations: [],
      privateMaterialIncluded: false,
      authenticatedSyncClaimed: false,
      moveClaimed: false,
    }
  } finally {
    await browser?.close().catch(() => undefined)
    if (applicationPid !== null && isRunning(applicationPid)) {
      try {
        process.kill(applicationPid, "SIGTERM")
      } catch {
        // The process ended between the observation and signal.
      }
    }
    if (applicationPid !== null) {
      await waitFor(
        () => !processIds.some(isRunning),
        "IDE-13 packaged application did not stop after SIGTERM",
        10_000,
      ).catch(async () => {
        termination = "sigkill_fallback"
        for (const pid of [...processIds].reverse()) {
          if (!isRunning(pid)) continue
          try {
            process.kill(pid, "SIGKILL")
          } catch {
            // The process ended between the observation and signal.
          }
        }
        await waitFor(
          () => !processIds.some(isRunning),
          "IDE-13 packaged application processes survived SIGKILL",
          5_000,
        )
      })
    }
    appProcess.kill("SIGTERM")
    await Promise.race([
      new Promise<void>(resolve => appProcess.once("exit", () => resolve())),
      new Promise<void>(resolve => setTimeout(resolve, 5_000)),
    ])
    if (appProcess.exitCode === null) appProcess.kill("SIGKILL")
    rmSync(workspace, { recursive: true, force: true, maxRetries: 8, retryDelay: 100 })
    rmSync(profile, { recursive: true, force: true, maxRetries: 8, retryDelay: 100 })
  }

  if (applicationPid === null || receiptInput === null || traceInput === null) {
    throw new Error("IDE-13 packaged evidence did not complete")
  }
  const survivingProcessCount = processIds.filter(isRunning).length
  const receipt = Schema.decodeUnknownSync(PackagedReceiptSchema)({
    ...receiptInput,
    teardown: {
      applicationPidCaptured: true,
      descendantCountBeforeStop: Math.max(0, processIds.length - 1),
      survivingProcessCount,
      termination,
    },
  })
  const trace = Schema.decodeUnknownSync(PackagedTraceSchema)(traceInput)
  const serialized = `${JSON.stringify(receipt)}\n${JSON.stringify(trace)}`
  if (serialized.includes(workspace) || serialized.includes(profile) ||
      serialized.includes(home) || containsCredential(serialized)) {
    throw new Error("IDE-13 packaged evidence artifacts contain private material")
  }
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, { mode: 0o600 })
  writeFileSync(tracePath, `${JSON.stringify(trace, null, 2)}\n`, { mode: 0o600 })
  process.stdout.write(`[openagents-desktop] IDE-13 packaged fail-closed evidence: ${receiptPath}\n`)
}

await main()
