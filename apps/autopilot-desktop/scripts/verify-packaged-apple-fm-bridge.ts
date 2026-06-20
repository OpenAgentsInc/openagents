/**
 * Pre-notarization gate for the local Apple FM signed-installer recut.
 *
 * Confirms a built macOS Autopilot Desktop `.app` actually bundles the Apple FM
 * Foundation Models bridge helper at the path Pylon's
 * `discoverAppleFmBridgeHelper` looks for in a packaged install — and that the
 * helper is non-empty and executable. Without this, a deep code-sign +
 * notarization could ship a green-looking installer that can never start a local
 * Apple FM session.
 *
 * Advances (does not clear):
 *   blocker.product_promises.local_apple_fm_signed_installer_recut_missing
 *
 * Exit 0 when a usable bundled helper is found; exit 1 (with secret-free
 * diagnostics) otherwise. Run after `electrobun build` and before
 * `scripts/notarize-macos.sh`.
 */
import { existsSync, statSync } from "node:fs"
import {
  verifyPackagedAppleFmBridge,
  type AppleFmBridgeProbe,
} from "../src/shared/apple-fm-packaging"

const fsProbe: AppleFmBridgeProbe = (helperPath) => {
  if (!existsSync(helperPath)) {
    return { exists: false, nonEmpty: false, executable: false }
  }
  const stat = statSync(helperPath)
  const ownerExecutable = (stat.mode & 0o100) !== 0
  return {
    exists: true,
    nonEmpty: stat.size > 0,
    executable: stat.isFile() && ownerExecutable,
  }
}

const result = verifyPackagedAppleFmBridge({ probe: fsProbe })

if (result.ok) {
  console.log(
    `Packaged Apple FM bridge helper verified (${result.verifiedEnv}): ${result.verifiedPath}`,
  )
  process.exit(0)
}

console.error(
  [
    "Packaged Apple FM bridge helper is missing or unusable.",
    "A signed-installer recut for autopilot.local_apple_fm_tool_chat.v1 must",
    "bundle the foundation-bridge helper so a from-install local session works.",
    "Checked candidates:",
    ...result.failures.map((f) => `- ${f.bundleDir}: ${f.reason}`),
  ].join("\n"),
)
process.exit(1)
