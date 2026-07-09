/**
 * Re-export shim — moved to `@openagentsinc/pylon-core/custody/account-connect`
 * (issue #8578, PY-1).
 *
 * `defaultCodexAuthValidityProbe` stays here (not a pure re-export) because
 * it depends on the vendored Codex CLI resolver in `./codex-composer.js`,
 * which is not (yet) in pylon-core. pylon-core's `runPylonAccountsConnect`
 * never calls it directly — it only invokes an injected
 * `options.codexAuthValidityProbe`, so this is the only piece that needs to
 * stay app-side; see the header comment in
 * `packages/pylon-core/src/custody/account-connect.ts` for the full
 * diagnosis. Everything else is a faithful pass-through.
 */
import { delimiter as pathDelimiter } from "node:path"
import { classifyCodexAuthProbeOutput, type PylonCodexAuthValidityProbe } from "@openagentsinc/pylon-core/custody/account-connect"

export * from "@openagentsinc/pylon-core/custody/account-connect"

const CODEX_AUTH_PROBE_DEFAULT_TIMEOUT_MS = 45_000

function codexAuthProbeTimeoutMs(env: Record<string, string | undefined>): number {
  const raw = (env.PYLON_CODEX_AUTH_PROBE_TIMEOUT_MS ?? "").trim()
  const parsed = Number.parseInt(raw, 10)
  return Number.isFinite(parsed) && parsed > 0 ? parsed : CODEX_AUTH_PROBE_DEFAULT_TIMEOUT_MS
}

/**
 * Default Codex credential validity probe. Runs the SAME vendored Codex binary
 * the executor uses, against the isolated account `CODEX_HOME`, with any BYOK
 * key (`OPENAI_API_KEY`/`CODEX_API_KEY`) unset so it tests the account's own
 * login. It uses a minimal read-only `codex exec` which forces a token refresh
 * and therefore surfaces a revoked refresh token, then classifies the result.
 * It is bounded by a short timeout and fail-safe: any spawn failure, missing
 * binary, or timeout returns `valid: true` so reconnect is never blocked by the
 * probe itself. It never touches `~/.codex`.
 */
export const defaultCodexAuthValidityProbe: PylonCodexAuthValidityProbe = async input => {
  try {
    const { resolveCodexCliPath } = await import("./codex-composer.js")
    const { executablePath, pathDirs } = resolveCodexCliPath()
    const env: Record<string, string> = {}
    for (const [key, value] of Object.entries({ ...process.env, ...input.env })) {
      if (value !== undefined) env[key] = value
    }
    // Probe the account's own ChatGPT login, never a BYOK API key.
    delete env.OPENAI_API_KEY
    delete env.CODEX_API_KEY
    env.CODEX_HOME = input.home
    if (pathDirs.length > 0) {
      env.PATH = `${pathDirs.join(pathDelimiter)}${env.PATH ? `${pathDelimiter}${env.PATH}` : ""}`
    }
    const child = Bun.spawn(
      [executablePath, "exec", "--skip-git-repo-check", "--sandbox", "read-only", "--color", "never", "ping"],
      { cwd: input.home, env, stdin: "ignore", stdout: "pipe", stderr: "pipe" },
    )
    let timedOut = false
    const timer = setTimeout(() => {
      timedOut = true
      try {
        child.kill()
      } catch {
        // best-effort kill
      }
    }, codexAuthProbeTimeoutMs(input.env))
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(child.stdout).text().catch(() => ""),
      new Response(child.stderr).text().catch(() => ""),
      child.exited,
    ])
    clearTimeout(timer)
    if (timedOut) {
      return { valid: true, reason: "probe_timeout" }
    }
    return classifyCodexAuthProbeOutput({ exitCode, stdout, stderr })
  } catch {
    // Fail-safe: never block a reconnect because the probe could not run.
    return { valid: true, reason: "probe_unavailable" }
  }
}
