/**
 * Apple Foundation Models packaged sidecar helper (AFM-6/AFM-7, #9075/#9076).
 *
 * Mirrors `voice-native-helper.ts` for the Swift `foundation-bridge` sidecar:
 * resolve the helper from `process.resourcesPath` (packaged) or `dist` (dev),
 * verify a `manifest.json` (protocolVersion, architecture === process.arch,
 * executable bit, sha256) plus a `codesign --verify --strict` signature when
 * packaged, then spawn it with a hardened, stripped environment.
 *
 * Canonical packaged path (AFM-7 §8.5, deliverable 5): the sidecar ships via
 * Forge `extraResource: ["dist/native"]`, so it lands at
 * `<Resources>/native/<arch>/foundation-bridge` — exactly like the voice
 * helper. Helper resolution and bundling both use THIS path; the desktop app
 * does not depend on Pylon's separate opt-in supervised-launcher discovery
 * (`<resources>/app/apple-fm-bridge/foundation-bridge`).
 *
 * UNLIKE the voice helper, the bridge speaks loopback HTTP: after spawn we poll
 * `GET /health` through the in-process Pylon FM client until ready or a typed
 * timeout, and we support ADOPT mode — if an operator-run bridge is already
 * healthy at the configured base URL we adopt it and never stop it.
 */
import { createHash } from "node:crypto"
import { spawn, type ChildProcess } from "node:child_process"
import { readFileSync, statSync } from "node:fs"
import path from "node:path"
import { appleFmComplete, appleFmProbe } from "@openagentsinc/apple-fm-runtime/node"
import type {
  AppleFmLaunchOutcome,
  AppleFmLauncher,
  AppleFmLauncherSession,
  AppleFmLauncherTurn,
  AppleFmProbe,
} from "./apple-fm-host.ts"

export const APPLE_FM_HELPER_BASENAME = "foundation-bridge" as const
export const APPLE_FM_DEFAULT_PORT = 11435 as const
export const AppleFmHelperRelativePath = path.join("native", process.arch, APPLE_FM_HELPER_BASENAME)
/**
 * The bridge carries its OWN manifest so it never collides with the voice
 * helper's `native/<arch>/manifest.json`. `dist/native/<arch>/` (dev) and
 * `<Resources>/native/<arch>/` (packaged) hold both helpers side by side.
 */
export const AppleFmHelperManifestRelativePath = path.join("native", process.arch, "foundation-bridge.manifest.json")
export type AppleFmHelperManifest = Readonly<{
  protocolVersion: 1
  helperVersion: string
  architecture: string
  sha256: string
}>

/** macOS Apple Silicon gate. Any other platform is `not_supported`. */
export const appleFmHelperSupported = (): boolean =>
  process.platform === "darwin" && process.arch === "arm64"

export const resolveAppleFmHelperPath = (resourcesPath: string): string =>
  path.join(resourcesPath, AppleFmHelperRelativePath)

/**
 * Resolve+verify the packaged helper. Throws a typed reason on any failure so
 * the launcher can classify it (missing vs tampered). Verification checks the
 * manifest shape, architecture, and executable bit, then makes the OS code
 * signature the authoritative integrity check.
 *
 * Integrity model (fix for #9155): electron-forge codesigns the in-bundle
 * Mach-O AFTER `build.ts` records its sha256 into the manifest, so a signed
 * binary's runtime bytes never match the pinned pre-sign digest. In a signed,
 * notarized bundle the `codesign --verify --strict` check (plus Gatekeeper over
 * the whole bundle) is the authoritative tamper protection, so a valid
 * signature ACCEPTS the helper without comparing the (expected-to-differ)
 * sha256. Only when the binary is unsigned or its signature is broken (dev
 * builds) do we fall back to the sha256 pin, which still detects tampering
 * against the exact unsigned build output.
 */
export const verifyAppleFmHelper = (input: Readonly<{
  resourcesPath: string
  manifest: AppleFmHelperManifest
  verifySignature: (absolutePath: string) => boolean
}>): string => {
  const absolutePath = resolveAppleFmHelperPath(input.resourcesPath)
  if (input.manifest.protocolVersion !== 1 || input.manifest.architecture !== process.arch) {
    throw new Error("apple_fm_helper_manifest_mismatch")
  }
  const stats = statSync(absolutePath)
  if (!stats.isFile() || (stats.mode & 0o111) === 0) throw new Error("apple_fm_helper_not_executable")
  // A valid, intact OS code signature is authoritative (the production, signed
  // case). Do NOT compare sha256 here: the pinned pre-sign digest is expected
  // to differ from the codesigned binary.
  if (input.verifySignature(absolutePath)) return absolutePath
  // Unsigned/dev build (or a genuinely broken signature): fall back to the
  // sha256 pin against the exact unsigned build output. This preserves
  // dev-time tamper detection; the typed error name is unchanged.
  const digest = createHash("sha256").update(readFileSync(absolutePath)).digest("hex")
  if (digest !== input.manifest.sha256) throw new Error("apple_fm_helper_digest_mismatch")
  return absolutePath
}

/**
 * Spawn the bridge on an explicit loopback port with a hardened environment.
 * No shell, no detach, no ambient PATH — identical hygiene to the voice helper.
 */
export const spawnAppleFmHelper = (absolutePath: string, port: number): ChildProcess =>
  spawn(absolutePath, ["--port", String(port)], {
    cwd: path.dirname(absolutePath),
    env: { LANG: "C", LC_ALL: "C", HOME: "/var/empty", PATH: "" },
    stdio: ["ignore", "pipe", "pipe"],
    detached: false,
    windowsHide: true,
  })

// ---------------------------------------------------------------------------
// Loopback FM client adapters (health + one bounded turn).
//
// AFS-02: these delegate to the neutral `@openagentsinc/apple-fm-runtime`
// loopback client, removing the Desktop dependency on the nested Pylon runtime.
// The bounded, public-safe `AppleFmProbe`/`AppleFmLauncherTurn` shapes are
// preserved so the supervisor and the existing tests are unchanged.
// ---------------------------------------------------------------------------

/**
 * Probe live readiness through the neutral loopback client. Never throws; a
 * transport or shape failure maps to a bounded public-safe probe.
 */
export const appleFmClientProbe = (
  baseUrl: string,
  fetchImpl: typeof fetch = fetch,
): Promise<AppleFmProbe> => appleFmProbe(baseUrl, fetchImpl)

/**
 * Run one bounded read-only completion through the neutral loopback client. When
 * `routeCandidates` is a non-empty set, the bridge runs GUIDED generation and
 * returns a well-formed route-recommendation JSON (owner directive 2026-07-20).
 */
export const appleFmClientComplete = (
  baseUrl: string,
  prompt: string,
  fetchImpl: typeof fetch = fetch,
  routeCandidates?: ReadonlyArray<string>,
): Promise<AppleFmLauncherTurn> => appleFmComplete(baseUrl, prompt, fetchImpl, undefined, routeCandidates)

// ---------------------------------------------------------------------------
// Packaged launcher — adopt an existing healthy bridge, else verify + spawn.
// ---------------------------------------------------------------------------

/** The minimal child-process surface the launcher needs (for test injection). */
export type AppleFmChildProcess = Readonly<{
  once: (event: "exit", listener: (code: number | null, signal: NodeJS.Signals | null) => void) => unknown
  kill: (signal?: NodeJS.Signals) => unknown
}>

export type PackagedAppleFmLauncherOptions = Readonly<{
  resourcesPath: string
  verifySignature: (absolutePath: string) => boolean
  /** Loads `native/<arch>/manifest.json`; throws when the helper is absent. */
  loadManifest?: () => AppleFmHelperManifest
  port?: number
  baseUrl?: string
  fetchImpl?: typeof fetch
  supported?: () => boolean
  spawnHelper?: (absolutePath: string, port: number) => AppleFmChildProcess
  probe?: (baseUrl: string, fetchImpl: typeof fetch) => Promise<AppleFmProbe>
  complete?: (
    baseUrl: string,
    prompt: string,
    fetchImpl: typeof fetch,
    routeCandidates?: ReadonlyArray<string>,
  ) => Promise<AppleFmLauncherTurn>
  readinessTimeoutMs?: number
  pollIntervalMs?: number
  sleep?: (ms: number) => Promise<void>
}>

const defaultSleep = (ms: number): Promise<void> => new Promise((resolve) => setTimeout(resolve, ms))

export const createPackagedAppleFmLauncher = (options: PackagedAppleFmLauncherOptions): AppleFmLauncher => {
  const port = options.port ?? APPLE_FM_DEFAULT_PORT
  const baseUrl = options.baseUrl ?? `http://127.0.0.1:${port}`
  const fetchImpl = options.fetchImpl ?? fetch
  const supported = options.supported ?? appleFmHelperSupported
  const probe = options.probe ?? appleFmClientProbe
  const complete = options.complete ?? appleFmClientComplete
  const spawnHelper = options.spawnHelper ?? spawnAppleFmHelper
  const loadManifest =
    options.loadManifest ??
    ((): AppleFmHelperManifest =>
      JSON.parse(
        readFileSync(path.join(options.resourcesPath, AppleFmHelperManifestRelativePath), "utf8"),
      ) as AppleFmHelperManifest)
  const readinessTimeoutMs = options.readinessTimeoutMs ?? 20_000
  const pollIntervalMs = options.pollIntervalMs ?? 250
  const sleep = options.sleep ?? defaultSleep

  const sessionFor = (mode: "launched" | "adopted", child: AppleFmChildProcess | null): AppleFmLauncherSession => ({
    mode,
    probe: () => probe(baseUrl, fetchImpl),
    complete: (prompt, routeCandidates) => complete(baseUrl, prompt, fetchImpl, routeCandidates),
    // Never stop an adopted operator bridge; only kill a child we launched.
    stop: () => {
      if (mode === "launched" && child !== null) {
        try {
          child.kill("SIGTERM")
        } catch {
          /* the child may already be gone; killing is best-effort */
        }
      }
    },
  })

  return {
    supported,
    launch: async ({ onCrash }): Promise<AppleFmLaunchOutcome> => {
      // 1. ADOPT: a bridge already healthy at the configured base URL wins.
      const adoptProbe = await probe(baseUrl, fetchImpl)
      if (adoptProbe.ready) {
        return { kind: "session", session: sessionFor("adopted", null) }
      }

      // 2. Resolve + verify the packaged helper. A missing manifest/file is
      //    `helper_missing`; a tampered/mismatched binary is `failed`.
      let absolutePath: string
      try {
        const manifest = loadManifest()
        absolutePath = verifyAppleFmHelper({ resourcesPath: options.resourcesPath, manifest, verifySignature: options.verifySignature })
      } catch (error) {
        const reason = error instanceof Error ? error.message : "apple_fm_helper_unavailable"
        if (
          reason === "apple_fm_helper_digest_mismatch" ||
          reason === "apple_fm_helper_signature_invalid" ||
          reason === "apple_fm_helper_manifest_mismatch" ||
          reason === "apple_fm_helper_not_executable"
        ) {
          return { kind: "failed", blockerRef: `blocker.apple_fm.${reason}`, failureClass: reason }
        }
        return { kind: "helper_missing", blockerRef: "blocker.apple_fm.helper_missing" }
      }

      // 3. Spawn and poll /health until ready or the typed timeout. A crash
      //    before readiness resolves the launch as failed; a later crash is
      //    reported to the host through onCrash.
      let child: AppleFmChildProcess
      try {
        child = spawnHelper(absolutePath, port)
      } catch {
        return { kind: "failed", blockerRef: "blocker.apple_fm.spawn_failed", failureClass: "spawn_failed" }
      }
      let crashed = false
      let adopted = false
      child.once("exit", () => {
        crashed = true
        // Only surface a crash to the host once we have handed it a session.
        if (adopted) onCrash("helper_crashed")
      })

      const deadline = Date.now() + readinessTimeoutMs
      while (Date.now() < deadline) {
        if (crashed) {
          return { kind: "failed", blockerRef: "blocker.apple_fm.helper_crashed", failureClass: "helper_crashed" }
        }
        const readyProbe = await probe(baseUrl, fetchImpl)
        if (readyProbe.ready) {
          adopted = true
          return { kind: "session", session: sessionFor("launched", child) }
        }
        await sleep(pollIntervalMs)
      }
      try {
        child.kill("SIGTERM")
      } catch {
        /* best-effort */
      }
      return { kind: "failed", blockerRef: "blocker.apple_fm.readiness_timeout", failureClass: "readiness_timeout" }
    },
  }
}
