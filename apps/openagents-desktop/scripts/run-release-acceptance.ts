#!/usr/bin/env node
import { Runtime } from "@openagentsinc/runtime-platform"
/** Exact-candidate macOS update/rollback/reinstall acceptance without deployment. */
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { isExportSafe } from "../src/diagnostics-contract.ts"
import { makeDiagnosticsHost } from "../src/diagnostics-host.ts"
import { openMacOSUpdateApplier } from "../src/macos-update-applier.ts"
import { computeDesktopReleasePublish } from "../src/release-publish.ts"
import { openDesktopUpdateStagingHost } from "../src/update-staging-host.ts"

type Step = Readonly<{ step: string; ok: boolean; detail: string }>

const argument = (name: string): string => {
  const prefix = `--${name}=`
  const found = Runtime.argv.slice(2).find(arg => arg.startsWith(prefix))?.slice(prefix.length).trim() ?? ""
  if (found === "") throw new Error(`missing ${prefix}<value>`)
  return found
}
const fileArgument = (name: string): string => path.resolve(argument(name))
const previousDmg = fileArgument("previous-dmg")
const candidateDmg = fileArgument("candidate-dmg")
const secretsFile = fileArgument("signing-secrets")
const outDir = fileArgument("out-dir")
const previousVersion = argument("previous-version")
const candidateVersion = argument("candidate-version")
const artifactUrl = `https://updates.openagents.invalid/artifacts/${path.basename(candidateDmg)}`
const feedBase = "https://updates.openagents.invalid/desktop/openagents/rc"
const root = mkdtempSync(path.join(tmpdir(), "openagents-release-acceptance-"))
const installedApp = "/Applications/OpenAgents Release Update Proof.app"
const updateRoot = path.join(root, "update")
const diagnosticsDir = path.join(root, "diagnostics")
const journal: Step[] = []

mkdirSync(outDir, { recursive: true, mode: 0o700 })
chmodSync(outDir, 0o700)
const persist = (): void => writeFileSync(path.join(outDir, "journal.json"), `${JSON.stringify(journal, null, 2)}\n`, { mode: 0o600 })
const record = (step: string, detail: string): void => {
  journal.push({ step, ok: true, detail: detail.slice(0, 400) })
  persist()
  console.log(`[openagents-desktop release-acceptance] ${step} OK ${detail}`)
}
const assert: (condition: unknown, message: string) => asserts condition = (condition, message) => {
  if (!condition) throw new Error(message)
}
const command = (executable: string, args: string[]): string => {
  const result = Runtime.spawnSync([executable, ...args], { stdout: "pipe", stderr: "pipe" })
  if (result.exitCode !== 0) throw new Error(`${path.basename(executable)} failed`)
  return result.stdout.toString("utf8").trim()
}
const mountedCopy = (dmg: string, destination: string): void => {
  const mount = path.join(root, `mount-${Math.random().toString(16).slice(2)}`)
  mkdirSync(mount, { recursive: true })
  command("/usr/bin/hdiutil", ["attach", "-readonly", "-nobrowse", "-mountpoint", mount, dmg])
  try {
    rmSync(destination, { recursive: true, force: true })
    command("/usr/bin/ditto", ["--rsrc", "--extattr", path.join(mount, "OpenAgents.app"), destination])
  } finally {
    command("/usr/bin/hdiutil", ["detach", mount])
    rmSync(mount, { recursive: true, force: true })
  }
}
const appVersion = (): string => command("/usr/libexec/PlistBuddy", ["-c", "Print :CFBundleShortVersionString", path.join(installedApp, "Contents", "Info.plist")])
const verifyInstalled = (expected: string): void => {
  assert(appVersion() === expected, `installed version is not ${expected}`)
  command("/usr/bin/codesign", ["--verify", "--deep", "--strict", installedApp])
  command("/usr/bin/xcrun", ["stapler", "validate", installedApp])
}

try {
  const secrets = Object.fromEntries(readFileSync(secretsFile, "utf8").split("\n").flatMap(line => {
    if (line.trim() === "" || line.startsWith("#") || !line.includes("=")) return []
    const at = line.indexOf("=")
    return [[line.slice(0, at).trim(), line.slice(at + 1).trim()]]
  }))
  const d = secrets.OPENAGENTS_RELEASE_SIGNING_PRIVATE_JWK_D
  const kid = secrets.OPENAGENTS_RELEASE_SIGNING_KID
  assert(typeof d === "string" && d !== "" && typeof kid === "string" && kid !== "", "release signing key unavailable")

  mountedCopy(previousDmg, installedApp)
  verifyInstalled(previousVersion)
  record("install-previous", `stapled ${previousVersion} installed as the reversible update source`)

  const artifactBytes = new Uint8Array(readFileSync(candidateDmg))
  const publish = computeDesktopReleasePublish({
    existingManifest: null,
    channel: "rc",
    version: candidateVersion,
    artifactName: path.basename(candidateDmg),
    artifactBytes,
    artifactUrl,
    releasedAt: "2026-07-13T22:09:10.663Z",
    notesRef: `mvp-8756-${candidateVersion}`,
    key: { d, kid },
  })
  artifactBytes.fill(0)
  const release = {
    channel: "rc",
    version: candidateVersion,
    artifactName: path.basename(candidateDmg),
    artifactUrl,
  }
  const fetchImpl = async (input: URL | RequestInfo): Promise<Response> => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.toString() : input.url
    if (url === `${feedBase}/manifest.json`) return new Response(Buffer.from(publish.payloadBytes))
    if (url === `${feedBase}/manifest.sig.json`) return Response.json(publish.envelope)
    if (url === `${feedBase}/release.json`) return Response.json(release)
    if (url === artifactUrl) return new Response(Buffer.from(await Runtime.file(candidateDmg).bytes()))
    return new Response("not found", { status: 404 })
  }
  record("signed-feed", `production-pinned manifest self-verified for ${candidateVersion}`)

  const applier7 = openMacOSUpdateApplier({
    root: updateRoot,
    installedAppPath: installedApp,
    installedVersion: previousVersion,
    channel: "rc",
    packaged: true,
  })
  const host = () => openDesktopUpdateStagingHost({
    root: updateRoot,
    installedVersion: previousVersion,
    channel: "rc",
    fetch: fetchImpl as typeof globalThis.fetch,
    openPath: async () => "",
    applier: applier7,
    baseUrl: feedBase,
  })
  const first = host()
  assert((await first.check()).phase === "available", `signed feed did not expose ${candidateVersion}`)
  assert((await first.download()).phase === "staged", `${candidateVersion} did not stage`)
  const recovered = host()
  assert(recovered.snapshot().phase === "staged", "staged update did not survive process interruption")
  record("interrupted-update", `digest-verified staged ${candidateVersion} survived host destruction and reopen`)
  const applied = await recovered.apply()
  assert(applied.phase === "restarting", `${candidateVersion} apply did not request restart: ${JSON.stringify(applied)}`)
  verifyInstalled(candidateVersion)
  record("signed-update", `real notarized ${previousVersion} app atomically replaced by notarized ${candidateVersion}`)

  const applier8 = openMacOSUpdateApplier({
    root: updateRoot,
    installedAppPath: installedApp,
    installedVersion: candidateVersion,
    channel: "rc",
    packaged: true,
  })
  const downgrade = await applier8.install(previousDmg, previousVersion)
  assert(!downgrade.ok && downgrade.reason === "candidate_not_monotonic", "downgrade was not refused")
  record("downgrade-refused", `${candidateVersion} refused ${previousVersion} as a candidate outside the rollback slot`)
  assert(applier8.rollbackAvailable() && applier8.rollbackVersion() === previousVersion, "rollback slot unavailable")
  const rolledBack = await applier8.rollback()
  assert(rolledBack.ok && rolledBack.action === "rolled_back", "rollback failed")
  verifyInstalled(previousVersion)
  record("rollback", `one retained rollback slot restored exact notarized ${previousVersion}`)

  const diagnostics = makeDiagnosticsHost({
    exportDir: diagnosticsDir,
    now: () => Date.parse("2026-07-13T22:30:00Z"),
    collectInputs: () => ({
      appVersion: candidateVersion,
      generatedAt: Date.parse("2026-07-13T22:30:00Z"),
      provider: { state: "ok", accounts: [] },
      runtimeGateway: { state: "present", lifecycle: "ready", sessionPhase: "local_ready", capabilities: [] },
      sync: { state: "local_ready", syncPhase: "local", pendingMutationCount: 0 },
      workspace: { state: "selected", git: "clean", entryCount: 2 },
      pty: { state: "available", sessionCount: 0 },
      extensions: { state: "ok", enabledCount: 0, totalCount: 0, dropped: 0 },
    }),
  })
  assert((await diagnostics.exportRedacted()).ok, "diagnostics export failed")
  const diagnosticFile = path.join(diagnosticsDir, readdirSync(diagnosticsDir)[0] ?? "")
  const report = JSON.parse(readFileSync(diagnosticFile, "utf8"))
  assert(isExportSafe(report), "diagnostics export was not public-safe")
  assert((statSync(diagnosticFile).mode & 0o777) === 0o600, "diagnostics export was not owner-only")
  record("diagnostics-export", "schema-valid public-safe owner-only diagnostic receipt written")

  rmSync(installedApp, { recursive: true, force: true })
  assert(!(await Runtime.file(path.join(installedApp, "Contents", "Info.plist")).exists()), "uninstall did not remove app")
  record("uninstall", "reversible proof app removed")
  mountedCopy(candidateDmg, installedApp)
  verifyInstalled(candidateVersion)
  record("reinstall", `exact stapled ${candidateVersion} reinstalled from the accepted DMG`)
  rmSync(installedApp, { recursive: true, force: true })
  rmSync(root, { recursive: true, force: true })
  record("cleanup", "proof app, rollback slot, staged bytes, diagnostics, mounts, and private state removed")
  record("summary", `exact ${previousVersion}-to-${candidateVersion} release acceptance sequence passed without deployment`)
} catch (error) {
  journal.push({ step: "summary", ok: false, detail: error instanceof Error ? error.message.slice(0, 400) : "release acceptance failed" })
  persist()
  rmSync(installedApp, { recursive: true, force: true })
  rmSync(root, { recursive: true, force: true })
  console.error(`[openagents-desktop release-acceptance] FAILED ${journal.at(-1)?.detail}`)
  process.exit(1)
}
