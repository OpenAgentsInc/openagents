import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  collectKhalaProcessText,
  KhalaProcessNonZeroExit,
  spawnKhalaProcess,
} from "../src/bun/khala-process"

const nodeEval = (script: string): readonly string[] => ["--eval", script]

const processExists = (pid: number): boolean => {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

const waitUntilGone = async (pid: number): Promise<boolean> => {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (!processExists(pid)) return true
    await new Promise(resolve => setTimeout(resolve, 25))
  }
  return !processExists(pid)
}

describe("KhalaProcess", () => {
  test("kills a scoped child when the scope closes", async () => {
    const pid = await Effect.runPromise(
      Effect.scoped(
        Effect.map(
          spawnKhalaProcess(process.execPath, nodeEval("setInterval(() => {}, 1000)")),
          handle => handle.pid,
        ),
      ),
    )

    expect(pid).toBeGreaterThan(0)
    expect(await waitUntilGone(pid)).toBe(true)
  })

  test("force kills a scoped child that ignores the graceful kill signal", async () => {
    const pid = await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          const handle = yield* spawnKhalaProcess(
            process.execPath,
            nodeEval("process.on('SIGTERM', () => {}); setInterval(() => {}, 1000)"),
            { forceKillAfter: 25 },
          )
          yield* Effect.sleep(100)
          return handle.pid
        }),
      ),
    )

    const gone = await waitUntilGone(pid)
    if (!gone) {
      try {
        process.kill(pid, "SIGKILL")
      } catch {
        // The assertion below reports the failed cleanup condition.
      }
    }

    expect(pid).toBeGreaterThan(0)
    expect(gone).toBe(true)
  })

  test("decodes stdout and stderr streams as text", async () => {
    const result = await collectKhalaProcessText(
      spawnKhalaProcess(process.execPath, nodeEval(
        "process.stdout.write('hello stdout\\n'); process.stderr.write('hello stderr\\n')",
      )),
    )

    expect(result).toEqual({
      exitCode: 0,
      stderr: "hello stderr\n",
      stdout: "hello stdout\n",
    })
  })

  test("reports nonzero exit as a tagged failure", async () => {
    const failure = await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function*() {
          const handle = yield* spawnKhalaProcess(process.execPath, nodeEval("process.exit(7)"))
          return yield* handle.exit.pipe(
            Effect.match({
              onFailure: error => error,
              onSuccess: exitCode => exitCode,
            }),
          )
        }),
      ),
    )

    expect(failure).toBeInstanceOf(KhalaProcessNonZeroExit)
    expect(failure).toMatchObject({
      _tag: "KhalaProcessNonZeroExit",
      command: process.execPath,
      exitCode: 7,
    })
  })

  test("does not leak listeners across repeated short-lived children", async () => {
    const warnings: Error[] = []
    const onWarning = (warning: Error) => warnings.push(warning)
    process.on("warning", onWarning)
    try {
      for (let index = 0; index < 25; index += 1) {
        const result = await collectKhalaProcessText(
          spawnKhalaProcess(process.execPath, nodeEval(`process.stdout.write(String(${index}))`)),
        )
        expect(result.exitCode).toBe(0)
        expect(result.stdout).toBe(String(index))
      }
    } finally {
      process.off("warning", onWarning)
    }

    expect(warnings.filter(warning => warning.name === "MaxListenersExceededWarning")).toEqual([])
  })
})
