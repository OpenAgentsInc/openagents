// WSL host detection — a dependency-free leaf module.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.windows_wsl_consumer_install_coverage_missing
//
// WSL reports `process.platform === "linux"`, so the supported-platform check
// (`isSupportedPlatform`) silently treats a WSL host as the proven `linux`
// target — contradicting the documented macOS/Linux-only scope-out. This module
// supplies the public-safe WSL signal both the consumer-install classifier and
// the bootstrap path consume to keep that scope-out enforced. It is deliberately
// a LEAF (no imports) so it can be shared by `bootstrap.ts` and
// `consumer-install-platform-support.ts` without a circular import.
//
// It is pure and side-effect-free: the caller supplies the environment and,
// optionally, the text of `/proc/version`. It never reads files itself and never
// emits any environment value, path, or machine identifier — only a boolean.

// Environment variables WSL sets in its Linux userland. Their mere presence is a
// reliable, public-safe WSL signal; we never read or emit their VALUES.
export const WSL_ENV_SIGNALS: readonly string[] = [
  "WSL_DISTRO_NAME",
  "WSL_INTEROP",
  "WSLENV",
]

/**
 * Detect whether the current host is running under WSL.
 *
 * Pure and side-effect-free. Returns a plain boolean derived from the presence
 * of WSL environment signals and, optionally, the "microsoft"/"WSL" marker that
 * WSL writes into `/proc/version`. It never reads files itself and never emits
 * any environment value, path, or machine identifier.
 */
export function detectWslHost(
  env: NodeJS.ProcessEnv = process.env,
  procVersionText?: string,
): boolean {
  for (const key of WSL_ENV_SIGNALS) {
    const value = env[key]
    if (typeof value === "string" && value.trim().length > 0) return true
  }
  if (
    typeof procVersionText === "string" &&
    /\b(?:microsoft|wsl)\b/i.test(procVersionText)
  ) {
    return true
  }
  return false
}
