/**
 * Packaging contract for the local Apple FM Foundation Models bridge helper.
 *
 * For `autopilot.local_apple_fm_tool_chat.v1` to reach green, a signed/notarized
 * Autopilot Desktop installer recut must *actually ship* the bridge helper at the
 * exact path Pylon's `discoverAppleFmBridgeHelper` looks for in a packaged app
 * (the `packaged-resource` source). Today nothing enforces that coupling: the
 * `notarize-macos.sh` flow would happily code-sign and notarize an installer that
 * contains no helper, producing a green-looking build that can never start a
 * local Apple FM session.
 *
 * This module makes that coupling explicit and testable. It declares:
 *   - the helper basename and its packaged sub-path under the app Resources,
 *   - the electrobun `build.copy` destination that lands it there,
 *   - per-build candidate locations inside a built macOS `.app` bundle,
 *   - a pure verifier (`verifyPackagedAppleFmBridge`) that checks the helper is
 *     present, executable, and *inside* the `.app` bundle root (so a deep
 *     code-sign + notarization covers it).
 *
 * It advances (does NOT clear):
 *   blocker.product_promises.local_apple_fm_signed_installer_recut_missing
 *
 * The verifier reads no clock and spawns nothing; the caller injects a `probe`
 * so it stays reproducible in tests and so it never embeds prompts, secrets, or
 * file contents — only structural packaging facts.
 */

/** Built helper binary name produced by the Swift `foundation-bridge` package. */
export const APPLE_FM_BRIDGE_HELPER_BASENAME = "foundation-bridge" as const

/**
 * Path of the bundled helper relative to the macOS app Resources directory
 * (electrobun's `PATHS.RESOURCES_FOLDER`). This MUST match Pylon's
 * `discoverAppleFmBridgeHelper` packaged-resource lookup, which joins
 * `resourcesDir + app/apple-fm-bridge/foundation-bridge`.
 */
export const APPLE_FM_BRIDGE_RESOURCES_SUBPATH =
  "app/apple-fm-bridge/foundation-bridge" as const

/**
 * The electrobun `build.copy` *destination* for the helper. electrobun copies
 * every `build.copy` dest under `<RESOURCES_FOLDER>/app/`, so this dest lands the
 * helper at `<RESOURCES_FOLDER>/app/apple-fm-bridge/foundation-bridge` — exactly
 * `APPLE_FM_BRIDGE_RESOURCES_SUBPATH`. Add an electrobun copy entry
 * `"<built helper>": APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST` once the helper is
 * built into `resources/` on the macOS build host.
 */
export const APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST =
  "apple-fm-bridge/foundation-bridge" as const

/**
 * macOS `.app` Resources directory, relative to the bundle root, where electrobun
 * places copied resources.
 */
const MACOS_APP_RESOURCES_RELATIVE = "Contents/Resources" as const

/**
 * Apple Foundation Models requires Apple Silicon, so packaged-helper candidates
 * are macOS arm64 only. Each entry is a built `.app` bundle directory relative to
 * the desktop package root, keyed by electrobun build env (dev/canary/stable).
 */
export const PACKAGED_APP_BUNDLE_CANDIDATES: ReadonlyArray<{
  readonly env: "dev" | "canary" | "stable"
  readonly bundleDir: string
}> = [
  { env: "dev", bundleDir: "build/dev-macos-arm64/Autopilot-dev.app" },
  { env: "canary", bundleDir: "build/canary-macos-arm64/Autopilot-canary.app" },
  { env: "stable", bundleDir: "build/stable-macos-arm64/Autopilot.app" },
]

/** Join POSIX path segments without importing node:path (pure, testable). */
function joinPosix(...segments: ReadonlyArray<string>): string {
  return segments
    .map((s, i) => (i === 0 ? s.replace(/\/+$/, "") : s.replace(/^\/+|\/+$/g, "")))
    .filter((s) => s.length > 0)
    .join("/")
}

/**
 * Absolute-within-bundle location of the helper for a given built `.app`
 * directory: `<bundleDir>/Contents/Resources/app/apple-fm-bridge/foundation-bridge`.
 */
export function packagedAppleFmBridgePath(bundleDir: string): string {
  return joinPosix(
    bundleDir,
    MACOS_APP_RESOURCES_RELATIVE,
    APPLE_FM_BRIDGE_RESOURCES_SUBPATH,
  )
}

/** Structural facts about one candidate helper path the verifier inspects. */
export type AppleFmBridgeProbeResult = {
  /** The helper file exists at the candidate path. */
  readonly exists: boolean
  /** The helper file is non-empty (a real binary, not a placeholder). */
  readonly nonEmpty: boolean
  /** The helper file has an owner-executable bit (will run after install). */
  readonly executable: boolean
}

export type AppleFmBridgeProbe = (helperPath: string) => AppleFmBridgeProbeResult

export type VerifyPackagedAppleFmBridgeInput = {
  /** Candidate built `.app` bundles to inspect. */
  readonly candidates?: ReadonlyArray<{ readonly env: string; readonly bundleDir: string }>
  /** Injected structural probe over a helper path. */
  readonly probe: AppleFmBridgeProbe
}

export type VerifyPackagedAppleFmBridgeResult = {
  /** True only when at least one candidate ships a usable, bundled helper. */
  readonly ok: boolean
  /** The verified helper path (within the `.app` bundle), when ok. */
  readonly verifiedPath: string | null
  /** The build env of the verified bundle, when ok. */
  readonly verifiedEnv: string | null
  /** Human-readable, secret-free reasons each candidate failed (when not ok). */
  readonly failures: ReadonlyArray<{ readonly bundleDir: string; readonly reason: string }>
}

/**
 * Pure verifier: a signed installer recut satisfies the helper-bundling
 * requirement iff some candidate `.app` contains a non-empty, executable helper
 * at the Pylon-discovery path inside the bundle root (so deep code-signing +
 * notarization will cover it). Returns structural facts only — no file contents.
 */
export function verifyPackagedAppleFmBridge(
  input: VerifyPackagedAppleFmBridgeInput,
): VerifyPackagedAppleFmBridgeResult {
  const candidates = input.candidates ?? PACKAGED_APP_BUNDLE_CANDIDATES
  const failures: Array<{ readonly bundleDir: string; readonly reason: string }> = []

  for (const candidate of candidates) {
    const helperPath = packagedAppleFmBridgePath(candidate.bundleDir)
    const result = input.probe(helperPath)
    if (!result.exists) {
      failures.push({ bundleDir: candidate.bundleDir, reason: "helper missing" })
      continue
    }
    if (!result.nonEmpty) {
      failures.push({ bundleDir: candidate.bundleDir, reason: "helper is empty" })
      continue
    }
    if (!result.executable) {
      failures.push({ bundleDir: candidate.bundleDir, reason: "helper not executable" })
      continue
    }
    return {
      ok: true,
      verifiedPath: helperPath,
      verifiedEnv: candidate.env,
      failures: [],
    }
  }

  return { ok: false, verifiedPath: null, verifiedEnv: null, failures }
}
