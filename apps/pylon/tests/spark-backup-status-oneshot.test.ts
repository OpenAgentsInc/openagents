import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

// #5312: end-to-end regression for the one-shot `wallet backup-status --json`
// CONTRACT. On rc.30 (Spark backup default-ON, #5304) a concurrent daemon
// (warm session + background provisioning + payout auto-register) contended
// with this one-shot's read on the SAME `storage.sql`; the read stalled past
// the operator's external alarm and emitted no JSON (exit 142). The fix wraps
// the whole read in a HARD wall-clock bound, ALWAYS emits bounded public-safe
// JSON, and FORCES the process to exit even if a dangling SDK handle would
// otherwise keep the event loop alive.
//
// We use the in-process test seam (`PYLON_SPARK_BACKUP_STATUS_TEST_HANG=1` +
// `PYLON_SPARK_BACKUP_STATUS_TIMEOUT_MS`) so the regression is deterministic
// and fast: the live read never resolves, so ONLY the bound + forced exit can
// save the command. Production never sets these env vars.

const INDEX = join(import.meta.dir, "..", "src", "index.ts")
const CWD = join(import.meta.dir, "..")

async function runOneShotBackupStatus(env: Record<string, string>): Promise<{
  exitCode: number | null
  stdout: string
  stderr: string
  elapsedMs: number
}> {
  const started = Date.now()
  const proc = Bun.spawn(["bun", INDEX, "wallet", "backup-status", "--json"], {
    cwd: CWD,
    stdout: "pipe",
    stderr: "pipe",
    env: {
      ...process.env,
      // Keep the run hermetic; the seam short-circuits before any daemon route.
      PYLON_DISABLE_DAEMON_ROUTING: "1",
      PYLON_DISABLE_OPENCODE_STARTUP: "1",
      ...env,
    },
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  return { exitCode, stdout, stderr, elapsedMs: Date.now() - started }
}

describe("#5312 one-shot wallet backup-status is hard-bounded and always exits", () => {
  test("a hung read on a FRESH home still emits bounded public-safe timeout JSON and exits promptly", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-5312-oneshot-fresh-"))
    try {
      const { exitCode, stdout, elapsedMs } = await runOneShotBackupStatus({
        PYLON_HOME: home,
        PYLON_CONTROL_PORT: "47931",
        PYLON_SPARK_BACKUP_STATUS_TEST_HANG: "1",
        PYLON_SPARK_BACKUP_STATUS_TIMEOUT_MS: "800",
      })
      // BOUNDED + EXITS: would hang forever without the fix (Trigger saw exit 142
      // from an external 30s/45s alarm). Generous ceiling absorbs bun cold start.
      expect(elapsedMs).toBeLessThan(20_000)
      // A timed-out read is reported as non-ok.
      expect(exitCode).toBe(1)
      // It EMITTED JSON (the rc.30 bug emitted none).
      const body = JSON.parse(stdout)
      expect(body.ok).toBe(false)
      expect(body.timedOut).toBe(true)
      // No cached target on a fresh home -> helper-unavailable with a timeout reason.
      expect(body.projection.state).toBe("helper-unavailable")
      expect(body.projection.helperUnavailableReason).toBe("timeout")
      expect(body.projection.blockerRefs).toContain("blocker.wallet.spark_backup.read_timed_out")
      // Redaction: no raw spark address material in the public-safe output.
      expect(stdout).not.toMatch(/spark1[0-9a-z]{20,}/)
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  }, 30_000)

  test("a hung read still exits with a public-safe body — never a raw spark1 target", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-5312-oneshot-redact-"))
    try {
      const { exitCode, stdout } = await runOneShotBackupStatus({
        PYLON_HOME: home,
        PYLON_CONTROL_PORT: "47932",
        PYLON_SPARK_BACKUP_STATUS_TEST_HANG: "1",
        PYLON_SPARK_BACKUP_STATUS_TIMEOUT_MS: "800",
      })
      expect(exitCode).toBe(1)
      // --show-local-target is NOT set here, so there is no localTarget block.
      const body = JSON.parse(stdout)
      expect(body.localTarget).toBeUndefined()
      expect(stdout).not.toMatch(/spark1[0-9a-z]{20,}/)
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  }, 30_000)
})
