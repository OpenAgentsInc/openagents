import { describe, expect, test } from "bun:test"
import { mkdtempSync } from "node:fs"
import { mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect, Deferred } from "effect"
import { createControlSessionActions, type ControlSessionExecutor } from "../src/node/control-sessions"
import {
  ControlCommandValidationError,
  isControlCommandValidationError,
} from "../src/node/control-command-error"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"

async function withControlSessionFixture<T>(fn: (fixture: {
  accountHome: string
  proofDir: string
  pylonHome: string
  summary: ReturnType<typeof createBootstrapSummary>
  worktree: string
}) => Promise<T>) {
  const root = mkdtempSync(join(tmpdir(), "pylon-control-session-receipts-"))
  try {
    const pylonHome = join(root, "pylon-home")
    const accountHome = join(root, "codex-home")
    const worktree = join(root, "worktree")
    const proofDir = join(root, "proofs")
    await mkdir(pylonHome, { recursive: true })
    await mkdir(accountHome, { recursive: true })
    await mkdir(worktree, { recursive: true })
    await writeFile(
      join(pylonHome, "config.json"),
      `${JSON.stringify({ dev: { accounts: [{ ref: "codex-a", provider: "codex", home: accountHome }] } })}\n`,
    )
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    return await fn({ accountHome, proofDir, pylonHome, summary, worktree })
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

describe("control session receipts", () => {
  test("session.spawn followed by session.cancel ends in cancelled terminal state", async () => {
    await withControlSessionFixture(async ({ proofDir, summary, worktree }) => {
      await Effect.runPromise(
        Effect.gen(function* () {
          const started = yield* Deferred.make<void>()
          const executor: ControlSessionExecutor = async (input) => {
            Effect.runFork(Deferred.succeed(started, undefined))
            return await new Promise<never>((_resolve, reject) => {
              input.abortSignal.addEventListener("abort", () => reject(new Error("cancelled")), { once: true })
            })
          }
          const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
          const spawned = yield* Effect.promise(() =>
            actions.spawn({
              type: "session.spawn",
              adapter: "codex",
              worktreePath: worktree,
              objective: "cancel this deterministic fake session",
              verify: ["bun", "--version"],
            }),
          )
          expect(spawned.sessionRef).toStartWith("session.pylon.control.")

          yield* Deferred.await(started)
          const cancelCommand = { type: "session.cancel", sessionRef: spawned.sessionRef } as const
          const cancelled = yield* Effect.promise(() => actions.cancel(cancelCommand.sessionRef))
          expect(cancelled.state).toBe("cancelled")
          expect(cancelled.errorClass).toBe("cancelled")

          const list = yield* Effect.promise(() => actions.list())
          expect(list).toContainEqual(
            expect.objectContaining({
              sessionRef: spawned.sessionRef,
              state: "cancelled",
              errorClass: "cancelled",
            }),
          )
        }),
      )
    })
  })

  // #4998: the requested lane defaults to `auto`, accepts cloud lanes, and is
  // surfaced on the session projection so it round-trips to clients.
  test("session.spawn records and surfaces the requested execution lane", async () => {
    await withControlSessionFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async () => {
        // Never resolves; we cancel to reach a terminal state deterministically.
        return await new Promise<never>((_resolve, reject) => {
          setTimeout(() => reject(new Error("cancelled")), 0)
        })
      }
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })

      const defaulted = await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        worktreePath: worktree,
        objective: "default lane session",
        verify: ["bun", "--version"],
      })
      const gce = await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        worktreePath: worktree,
        objective: "gce lane session",
        verify: ["bun", "--version"],
        lane: "cloud-gcp",
      })

      const list = await actions.list()
      const defaultedRow = list.find((s) => s.sessionRef === defaulted.sessionRef)
      const gceRow = list.find((s) => s.sessionRef === gce.sessionRef)
      expect(defaultedRow?.lane).toBe("auto")
      expect(gceRow?.lane).toBe("cloud-gcp")

      await expect(
        actions.spawn({
          type: "session.spawn",
          adapter: "codex",
          worktreePath: worktree,
          objective: "bad lane session",
          verify: ["bun", "--version"],
          // @ts-expect-error invalid lane is rejected at parse time
          lane: "cloud-aws",
        }),
      ).rejects.toThrow(/lane must be one of/)

      await actions.cancel(defaulted.sessionRef)
      await actions.cancel(gce.sessionRef)
    })
  })

  test("executor failures retain typed refs without raw thrown error text", async () => {
    await withControlSessionFixture(async ({ proofDir, summary, worktree }) => {
      const rawErrorSentence = "raw provider failure sentence must not be retained"
      const executor: ControlSessionExecutor = async () => {
        throw new Error(rawErrorSentence)
      }
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const spawned = await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        worktreePath: worktree,
        objective: "fail this deterministic fake session",
        verify: ["bun", "--version"],
      })

      let list = [] as Array<{
        sessionRef: string
        state: string
        artifactRef: string | null
        errorClass: string | null
        errorDigestRef: string | null
      }>
      for (let attempt = 0; attempt < 20; attempt += 1) {
        list = await actions.list()
        if (list[0]?.state === "failed" && list[0]?.artifactRef !== null) break
        await Bun.sleep(10)
      }
      expect(list[0]).toEqual(
        expect.objectContaining({
          sessionRef: spawned.sessionRef,
          state: "failed",
          errorClass: "execution_error",
        }),
      )
      expect(list[0]?.artifactRef).toStartWith("artifact.pylon.control_session.failure.")
      expect(list[0]?.errorDigestRef).toStartWith("digest.pylon.session.error.")

      const serialized = await readFile(join(proofDir, `${spawned.sessionRef}-failure.json`), "utf8")
      expect(serialized).not.toContain(rawErrorSentence)
      expect(serialized).not.toContain("provider failure")
      expect(serialized).not.toContain("Error:")
      const failure = JSON.parse(serialized) as Record<string, unknown>
      expect(failure.schema).toBe("openagents.pylon.control_session_failure.v0.1")
      expect(failure.sessionRef).toBe(spawned.sessionRef)
      expect(failure.workspaceRef).toStartWith("workspace.pylon.control_session.injected.")
      expect(failure.errorClass).toBe("execution_error")
      expect(failure.errorDigestRef).toStartWith("digest.pylon.session.error.")
      expect(Object.keys(failure).sort()).toEqual([
        "account",
        "adapter",
        "errorClass",
        "errorDigestRef",
        "generatedAt",
        "redactionScan",
        "schema",
        "sessionRef",
        "workspaceRef",
      ])
    })
  })

  // #5453: a Blueprint chat turn dispatches session.spawn with NO workspace
  // selector. That used to throw "session.spawn requires repoRef or
  // worktreePath" and the control server turned it into a raw HTTP 500
  // (`control 500`). It now materializes a private ephemeral scratch workspace
  // and spawns successfully.
  describe("workspace-less chat turn (#5453)", () => {
    test("session.spawn with no repoRef/worktreePath spawns in an ephemeral scratch workspace", async () => {
      await withControlSessionFixture(async ({ proofDir, summary }) => {
        const executor: ControlSessionExecutor = async (input) =>
          await new Promise<never>((_resolve, reject) => {
            input.abortSignal.addEventListener("abort", () => reject(new Error("cancelled")), {
              once: true,
            })
          })
        const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })

        const spawned = await actions.spawn({
          type: "session.spawn",
          adapter: "codex",
          objective: "Blueprint chat turn with no workspace selector",
          verify: ["true"],
        })
        expect(spawned.sessionRef).toStartWith("session.pylon.control.")

        const list = await actions.list()
        const row = list.find((s) => s.sessionRef === spawned.sessionRef)
        expect(row?.workspaceRef).toStartWith("workspace.pylon.control_session.scratch.")

        await actions.cancel(spawned.sessionRef)
      })
    })

    test("session.spawn with both repoRef and worktreePath is a typed validation error", async () => {
      await withControlSessionFixture(async ({ proofDir, summary, worktree }) => {
        const executor: ControlSessionExecutor = async () =>
          await new Promise<never>((_resolve, reject) => {
            setTimeout(() => reject(new Error("cancelled")), 0)
          })
        const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })

        let thrown: unknown
        try {
          await actions.spawn({
            type: "session.spawn",
            adapter: "codex",
            objective: "conflicting selectors",
            verify: ["true"],
            worktreePath: worktree,
            // @ts-expect-error force the conflict the parser rejects
            repoRef: {
              provider: "github",
              visibility: "public",
              fullName: "OpenAgentsInc/openagents",
              branch: "main",
              commitSha: "a".repeat(40),
            },
          })
        } catch (error) {
          thrown = error
        }
        expect(isControlCommandValidationError(thrown)).toBe(true)
        expect((thrown as ControlCommandValidationError).reason).toBe(
          "workspace_selector_conflict",
        )
      })
    })
  })
})
