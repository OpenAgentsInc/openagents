import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

const INDEX = join(import.meta.dir, "..", "src", "index.ts")
const CWD = join(import.meta.dir, "..")
const OPERATOR_SNAPSHOT_ONESHOT_TIMEOUT_MS = 10_000

async function runOperatorSnapshot(env: Record<string, string>): Promise<{
  elapsedMs: number
  exitCode: number | null
  stderr: string
  stdout: string
  timedOut: boolean
}> {
  const started = Date.now()
  const proc = Bun.spawn(["bun", INDEX, "operator", "snapshot", "--json"], {
    cwd: CWD,
    env: {
      ...process.env,
      PYLON_DISABLE_DAEMON_ROUTING: "1",
      PYLON_DISABLE_OPENCODE_STARTUP: "1",
      PYLON_SPARK_BACKUP_DISABLED: "1",
      ...env,
    },
    stderr: "pipe",
    stdout: "pipe",
  })
  let timeout: ReturnType<typeof setTimeout> | undefined
  const exit = await Promise.race([
    proc.exited.then((exitCode) => ({ exitCode, timedOut: false })),
    new Promise<{ exitCode: null; timedOut: true }>((resolve) => {
      timeout = setTimeout(() => {
        proc.kill()
        resolve({ exitCode: null, timedOut: true })
      }, OPERATOR_SNAPSHOT_ONESHOT_TIMEOUT_MS)
    }),
  ])
  if (timeout !== undefined) clearTimeout(timeout)
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ])

  return {
    elapsedMs: Date.now() - started,
    exitCode: exit.exitCode,
    stderr,
    stdout,
    timedOut: exit.timedOut,
  }
}

describe("#5401 operator snapshot one-shot clean exit", () => {
  test("operator snapshot --json exits after emitting JSON even with a dangling handle", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-5401-operator-snapshot-"))
    try {
      const result = await runOperatorSnapshot({
        PYLON_HOME: home,
        PYLON_OPERATOR_SNAPSHOT_TEST_DANGLING_HANDLE: "1",
      })

      expect(result.timedOut).toBe(false)
      expect(result.elapsedMs).toBeLessThan(OPERATOR_SNAPSHOT_ONESHOT_TIMEOUT_MS)
      expect(result.exitCode).toBe(0)
      expect(result.stdout).not.toContain("Breez SDK")

      const body = JSON.parse(result.stdout)
      expect(body.schema).toBe("openagents.pylon.operator_snapshot.v0.3")
      expect(body.recovery.headlessCommandRefs).toContain(
        "command.pylon.status_json",
      )
      expect(result.stdout).not.toMatch(/mnemonic|spark1[0-9a-z]{20,}|lnbc/i)
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  }, 15_000)
})
