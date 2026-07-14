/**
 * Release preflight (CUT-26, #8706) — the CI/release oracle set that must be
 * GREEN before any OpenAgents Desktop artifact is packaged, signed, or
 * published. Every check is a pure function over injected inputs (unit-tested
 * in `tests/release-preflight.test.ts`); the CLI wrapper gathers the real
 * inputs and fails closed (exit 1) on ANY red row.
 *
 * What it proves without a signing ceremony:
 *   - the tree is clean and exactly `origin/main` (publish-only-from-main law)
 *   - the candidate version parses and is strictly newer than the last
 *     released version for the channel (monotonicity)
 *   - the pinned upstream MIT attribution is intact (UPSTREAM.md)
 *   - the app identity is stable (name/productName/entry)
 *   - the built artifact set is complete and carries NO upstream-updater
 *     remnants, NO legacy Desktop UI entrypoints/assets, and NO absolute
 *     source-checkout paths (source-checkout runtime dependencies)
 *
 * Gatekeeper release oracles (DMG-1, #8786 — from the T3 Gatekeeper-dead DMG
 * and the ChatGPT dead-update incidents of 2026-07-13): the release lane
 * REFUSES when the Developer ID identity or notary credentials are absent
 * (no unsigned fallback; `--allow-unsigned-dev` is the only, honest escape
 * valve and its artifacts are named `-UNSIGNED-DEV`), and — when pointed at
 * built artifacts via `--dmg`/`--app` — gates on `codesign --verify --deep
 * --strict` (app), `spctl -a -t open --context context:primary-signature`
 * (image), `spctl -a -t exec` (app), and `xcrun stapler validate` (both).
 * Every verdict is a pure interpreter over recorded command output
 * (`scripts/macos-gatekeeper.ts`), unit-tested without owner credentials.
 *
 * What it deliberately does NOT do: touch the ed25519 private key, the Apple
 * Developer ID identity, or notarization — those are owner-gated ceremonies
 * (see the workspace NEEDS_OWNER ledger). This script never reads `.secrets`.
 *
 * Usage:
 *   bun scripts/release-preflight.ts [--channel stable|rc] [--latest-released X.Y.Z]
 *     [--dmg <path/to/OpenAgents.dmg>] [--app <path/to/OpenAgents.app>]
 *     [--allow-unsigned-dev] [--json]
 */
import { existsSync, readFileSync } from "node:fs"
import path from "node:path"
import {
  type UpdateChannel,
  isMonotonicUpgrade,
  parseReleaseVersion,
  updateChannels,
} from "../src/update-contract.ts"
import {
  checkArtifactNotUnsignedDev,
  checkSigningCredentialsPresent,
  gatekeeperAppChecks,
  gatekeeperImageChecks,
  readMacSigningCredentials,
} from "./macos-gatekeeper.ts"

export interface PreflightCheck {
  readonly id: string
  readonly ok: boolean
  readonly detail: string
}

// ---------------------------------------------------------------------------
// Pure checks
// ---------------------------------------------------------------------------

export const checkCleanOriginMain = (input: {
  readonly statusPorcelain: string
  readonly headSha: string
  readonly originMainSha: string
}): PreflightCheck => {
  const dirty = input.statusPorcelain.trim().length > 0
  const detached = input.headSha.trim() !== input.originMainSha.trim() || input.headSha.trim() === ""
  return {
    id: "clean_origin_main",
    ok: !dirty && !detached,
    detail: dirty
      ? "working tree has uncommitted changes — publish only from a clean origin/main"
      : detached
        ? `HEAD ${input.headSha.trim().slice(0, 10)} is not origin/main ${input.originMainSha.trim().slice(0, 10)}`
        : `clean at origin/main ${input.headSha.trim().slice(0, 10)}`,
  }
}

export const checkVersionMonotonic = (input: {
  readonly candidate: string
  readonly latestReleased: string | null
  readonly channel: UpdateChannel
}): PreflightCheck => {
  if (parseReleaseVersion(input.candidate) === null) {
    return {
      id: "version_monotonic",
      ok: false,
      detail: `candidate version "${input.candidate}" is not a valid release version`,
    }
  }
  if (input.channel === "stable" && input.candidate.includes("-rc.")) {
    return {
      id: "version_monotonic",
      ok: false,
      detail: `pre-release ${input.candidate} may not publish on the stable channel`,
    }
  }
  if (input.latestReleased === null) {
    return {
      id: "version_monotonic",
      ok: true,
      detail: `candidate ${input.candidate} valid; no prior release provided (first release or pass --latest-released)`,
    }
  }
  const verdict = isMonotonicUpgrade(input.latestReleased, input.candidate, input.channel)
  return {
    id: "version_monotonic",
    ok: verdict.admissible,
    detail: verdict.admissible
      ? `${input.latestReleased} -> ${input.candidate} is a strict ${input.channel} upgrade`
      : `${input.latestReleased} -> ${input.candidate} refused: ${verdict.admissible === false ? verdict.reason : ""}`,
  }
}

/** The pinned upstream template commit recorded at scaffold time (#8574). */
export const UPSTREAM_PINNED_COMMIT = "a02e7bbfe0c196db22b76f40ec23b5c265d24215"

export const checkAttributionIntact = (upstreamMd: string): PreflightCheck => {
  const missing: Array<string> = []
  if (!upstreamMd.includes("MIT")) missing.push("MIT license notice")
  if (!upstreamMd.includes("LuanRoger/electron-shadcn")) missing.push("upstream repo attribution")
  if (!upstreamMd.includes(UPSTREAM_PINNED_COMMIT)) missing.push("pinned upstream commit")
  return {
    id: "attribution_intact",
    ok: missing.length === 0,
    detail: missing.length === 0
      ? "UPSTREAM.md attribution (MIT, repo, pinned commit) intact"
      : `UPSTREAM.md missing: ${missing.join(", ")}`,
  }
}

export const checkAppIdentity = (packageJson: {
  readonly name?: unknown
  readonly productName?: unknown
  readonly main?: unknown
  readonly version?: unknown
}): PreflightCheck => {
  const problems: Array<string> = []
  if (packageJson.name !== "@openagentsinc/openagents-desktop") problems.push(`name=${String(packageJson.name)}`)
  if (packageJson.productName !== "OpenAgents") problems.push(`productName=${String(packageJson.productName)}`)
  if (packageJson.main !== "dist/main.js") problems.push(`main=${String(packageJson.main)}`)
  if (typeof packageJson.version !== "string" || parseReleaseVersion(packageJson.version) === null) {
    problems.push(`version=${String(packageJson.version)}`)
  }
  return {
    id: "app_identity_stable",
    ok: problems.length === 0,
    detail: problems.length === 0
      ? "app identity stable (@openagentsinc/openagents-desktop / OpenAgents / dist/main.js)"
      : `identity drift: ${problems.join(", ")}`,
  }
}

/** The complete required artifact set for one release build. */
export const REQUIRED_ARTIFACTS = [
  "main.js",
  "preload.cjs",
  "workers/codex-history-worker.js",
  "workers/workspace-search-worker.js",
  "renderer/boot.js",
  "renderer/index.html",
  "renderer/app.css",
  "assets/openagents-icon.png",
  "builtin-skills/manifest.json",
  "builtin-skills/productspec-work/SKILL.md",
  "builtin-skills/assurancespec-work/SKILL.md",
] as const

/**
 * Upstream updater/publisher remnants that must NEVER re-enter the artifact
 * (the template's auto-update + GitHub publisher wiring was removed at
 * scaffold time; the owned update path is the signed updates.openagents.com
 * feed via the typed contract in src/update-contract.ts).
 */
export const FORBIDDEN_UPDATER_MARKERS = [
  "updateElectronApp",
  "update-electron-app",
  "electron-updater",
  "@electron-forge/publisher",
  "LuanRoger/electron-shadcn",
] as const

/** Legacy Desktop UI entrypoints/assets that must never ship in this app. */
export const FORBIDDEN_LEGACY_UI_MARKERS = [
  "khala-code-desktop",
  "autopilot-desktop",
  "clients/khala-mobile",
  "Electrobun",
  "electrobun",
] as const

export interface ArtifactFile {
  readonly relativePath: string
  readonly text: string
}

export const checkArtifactSet = (present: ReadonlyArray<string>): PreflightCheck => {
  const missing = REQUIRED_ARTIFACTS.filter((artifact) => !present.includes(artifact))
  return {
    id: "artifact_set_complete",
    ok: missing.length === 0,
    detail: missing.length === 0
      ? `all ${REQUIRED_ARTIFACTS.length} required artifacts present`
      : `missing artifacts: ${missing.join(", ")}`,
  }
}

export const checkNoUpdaterRemnants = (files: ReadonlyArray<ArtifactFile>): PreflightCheck => {
  const hits: Array<string> = []
  for (const file of files) {
    for (const marker of FORBIDDEN_UPDATER_MARKERS) {
      if (file.text.includes(marker)) hits.push(`${file.relativePath}:${marker}`)
    }
  }
  return {
    id: "no_upstream_updater_remnants",
    ok: hits.length === 0,
    detail: hits.length === 0
      ? "no template updater/publisher remnants in the artifact"
      : `updater remnants found: ${hits.join(", ")}`,
  }
}

export const checkNoLegacyUiEntrypoints = (files: ReadonlyArray<ArtifactFile>): PreflightCheck => {
  const hits: Array<string> = []
  for (const file of files) {
    for (const marker of FORBIDDEN_LEGACY_UI_MARKERS) {
      if (file.text.includes(marker)) hits.push(`${file.relativePath}:${marker}`)
    }
  }
  return {
    id: "no_legacy_ui_entrypoints",
    ok: hits.length === 0,
    detail: hits.length === 0
      ? "no legacy Desktop UI entrypoints/assets in the artifact"
      : `legacy UI markers found: ${hits.join(", ")}`,
  }
}

/**
 * The packaged artifact must not depend on this source checkout at runtime:
 * no absolute developer-machine paths may be baked into any bundle. (Bare
 * externals like `electron` are resolved by the packager; absolute paths are
 * how a bundle silently keeps working on the build machine only.)
 */
export const checkNoSourceCheckoutPaths = (
  files: ReadonlyArray<ArtifactFile>,
  repoRoot: string,
): PreflightCheck => {
  const forbidden = ["/Users/", "/home/", repoRoot]
  const hits: Array<string> = []
  for (const file of files) {
    for (const marker of forbidden) {
      if (marker.length > 1 && file.text.includes(marker)) {
        hits.push(`${file.relativePath}:${marker === repoRoot ? "<repo-root>" : marker}`)
      }
    }
  }
  return {
    id: "no_source_checkout_paths",
    ok: hits.length === 0,
    detail: hits.length === 0
      ? "no absolute source-checkout paths baked into the artifact"
      : `source-checkout paths found: ${hits.join(", ")}`,
  }
}

// ---------------------------------------------------------------------------
// CLI wrapper — gathers real inputs, prints the table, fails closed
// ---------------------------------------------------------------------------

const appRoot = path.resolve(import.meta.dir, "..")

const git = (...args: Array<string>): string => {
  const result = Bun.spawnSync(["git", ...args], { cwd: appRoot, stdout: "pipe", stderr: "pipe" })
  if (result.exitCode !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${result.stderr.toString().trim()}`)
  }
  return result.stdout.toString()
}

export const gatherArtifactFiles = (distDir: string): ReadonlyArray<ArtifactFile> =>
  REQUIRED_ARTIFACTS
    .filter((artifact) => artifact !== "assets/openagents-icon.png")
    .filter((artifact) => existsSync(path.join(distDir, artifact)))
    .map((artifact) => ({
      relativePath: artifact,
      text: readFileSync(path.join(distDir, artifact), "utf8"),
    }))

export const runPreflight = (options: {
  readonly channel: UpdateChannel
  readonly latestReleased: string | null
  /** Built disk image to assess with the image-side Gatekeeper oracles. */
  readonly dmgPath?: string
  /** Packaged `.app` bundle to assess with the app-side Gatekeeper oracles. */
  readonly appPath?: string
  /** Dev escape valve — softens ONLY the credentials row, never the artifact oracles. */
  readonly allowUnsignedDev?: boolean
}): ReadonlyArray<PreflightCheck> => {
  const packageJson = JSON.parse(readFileSync(path.join(appRoot, "package.json"), "utf8")) as {
    version?: string
    name?: string
    productName?: string
    main?: string
  }
  const upstreamMd = readFileSync(path.join(appRoot, "UPSTREAM.md"), "utf8")
  const dist = path.join(appRoot, "dist")
  const presentArtifacts = REQUIRED_ARTIFACTS.filter((artifact) => existsSync(path.join(dist, artifact)))
  const artifactFiles = gatherArtifactFiles(dist)
  const repoRoot = git("rev-parse", "--show-toplevel").trim()

  return [
    checkCleanOriginMain({
      statusPorcelain: git("status", "--porcelain"),
      headSha: git("rev-parse", "HEAD"),
      originMainSha: git("rev-parse", "origin/main"),
    }),
    checkVersionMonotonic({
      candidate: packageJson.version ?? "",
      latestReleased: options.latestReleased,
      channel: options.channel,
    }),
    checkAttributionIntact(upstreamMd),
    checkAppIdentity(packageJson),
    checkArtifactSet(presentArtifacts),
    checkNoUpdaterRemnants(artifactFiles),
    checkNoLegacyUiEntrypoints(artifactFiles),
    checkNoSourceCheckoutPaths(artifactFiles, repoRoot),
    // Gatekeeper release oracles (#8786). The credentials row is fail-closed
    // by default: a release preflight with no Developer ID/notary credentials
    // is RED unless the caller explicitly engages the -UNSIGNED-DEV escape
    // valve. Artifact oracles run only when the caller points at real bytes;
    // an -UNSIGNED-DEV name is refused UNCONDITIONALLY (the valve never
    // greenlights publishing what it produced).
    checkSigningCredentialsPresent(readMacSigningCredentials(), options.allowUnsignedDev === true),
    ...(options.dmgPath === undefined
      ? []
      : [checkArtifactNotUnsignedDev(options.dmgPath), ...gatekeeperImageChecks(options.dmgPath)]),
    ...(options.appPath === undefined ? [] : gatekeeperAppChecks(options.appPath)),
  ]
}

if (import.meta.main) {
  const args = process.argv.slice(2)
  const readFlag = (flag: string): string | null => {
    const index = args.indexOf(flag)
    return index >= 0 && index + 1 < args.length ? args[index + 1]! : null
  }
  const channelArg = readFlag("--channel") ?? "rc"
  if (!(updateChannels as ReadonlyArray<string>).includes(channelArg)) {
    console.error(`unknown --channel ${channelArg}; expected ${updateChannels.join("|")}`)
    process.exit(1)
  }
  const dmgPath = readFlag("--dmg")
  const appPath = readFlag("--app")
  const checks = runPreflight({
    channel: channelArg as UpdateChannel,
    latestReleased: readFlag("--latest-released"),
    ...(dmgPath === null ? {} : { dmgPath }),
    ...(appPath === null ? {} : { appPath }),
    allowUnsignedDev: args.includes("--allow-unsigned-dev"),
  })
  if (args.includes("--json")) {
    console.log(JSON.stringify({ ok: checks.every((check) => check.ok), checks }, null, 2))
  } else {
    for (const check of checks) {
      console.log(`${check.ok ? "PASS" : "FAIL"}  ${check.id}  ${check.detail}`)
    }
  }
  process.exit(checks.every((check) => check.ok) ? 0 : 1)
}
