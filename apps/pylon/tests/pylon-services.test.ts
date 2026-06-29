import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { Deferred, Effect, Fiber } from "effect"

import {
  PylonLocalStateStore,
  PylonLocalStateStoreLive,
  PylonRuntimeConfig,
  makePylonRuntimeConfigLayer,
  PylonServiceError,
  PylonWorkspaceMaterializer,
  PylonWorkspaceMaterializerLive,
} from "../src/pylon-services"
import type { GitCheckoutWorkspace, WorkspaceCheckoutRunner } from "../src/workspace-materializer"

const checkout: GitCheckoutWorkspace = {
  kind: "git_checkout",
  repository: {
    branch: "main",
    commitSha: "3333333333333333333333333333333333333333",
    fullName: "OpenAgentsInc/public-sum-fixture",
    provider: "github",
    visibility: "public",
  },
  verificationCommand: {
    args: ["bun", "test", "sum.test.ts"],
    commandRef: "command.public.pylon_khala.verify.test",
  },
}

async function withTempDir<T>(prefix: string, fn: (dir: string) => Promise<T>): Promise<T> {
  const dir = await mkdtemp(join(tmpdir(), prefix))
  try {
    return await fn(dir)
  } finally {
    await rm(dir, { recursive: true, force: true })
  }
}

async function expectPylonServiceError(effect: Effect.Effect<unknown, PylonServiceError>): Promise<PylonServiceError> {
  try {
    await Effect.runPromise(effect)
  } catch (error) {
    expect(error).toBeInstanceOf(PylonServiceError)
    return error as PylonServiceError
  }
  throw new Error("expected PylonServiceError")
}

describe("PylonLocalStateStore", () => {
  test("distinguishes not-found and malformed runtime state reads", async () => {
    await withTempDir("pylon-services-state-", async (home) => {
      const paths = {
        home,
        cache: join(home, "cache"),
        config: join(home, "config.json"),
        releases: join(home, "releases"),
        identity: join(home, "identity.json"),
        identityMnemonic: join(home, "identity.mnemonic"),
        runtimeState: join(home, "runtime-state.json"),
        presenceState: join(home, "presence-state.json"),
        activeAssignmentRuns: join(home, "active-assignment-runs"),
        assignmentState: join(home, "assignment-state.json"),
        ledger: join(home, "ledger.jsonl"),
      }

      const missing = await expectPylonServiceError(
        Effect.gen(function* () {
          const store = yield* PylonLocalStateStore
          return yield* store.readRuntime(paths)
        }).pipe(Effect.provide(PylonLocalStateStoreLive)),
      )
      expect(missing.kind).toBe("not_found")
      expect(missing.operation).toBe("state.read_runtime")

      await writeFile(paths.runtimeState, "{not-json")
      const malformed = await expectPylonServiceError(
        Effect.gen(function* () {
          const store = yield* PylonLocalStateStore
          return yield* store.readRuntime(paths)
        }).pipe(Effect.provide(PylonLocalStateStoreLive)),
      )
      expect(malformed.kind).toBe("malformed")
      expect(malformed.reasonRef).toBe("reason.pylon.state.read_runtime.json_malformed")
    })
  })
})

describe("PylonRuntimeConfig", () => {
  test("redacts env snapshots and types missing required env as config diagnostics", async () => {
    const layer = makePylonRuntimeConfigLayer({
      OPENAGENTS_AGENT_TOKEN: "secret-token",
    })
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const config = yield* PylonRuntimeConfig
        const snapshot = yield* config.redactedEnvSnapshot(["OPENAGENTS_AGENT_TOKEN", "MISSING_TOKEN"])
        const missing = yield* Effect.exit(config.requireEnv("MISSING_TOKEN"))
        return { missing, snapshot }
      }).pipe(Effect.provide(layer)),
    )

    expect(result.snapshot.OPENAGENTS_AGENT_TOKEN).toStartWith("redacted.cause.pylon.")
    expect(result.snapshot.OPENAGENTS_AGENT_TOKEN).not.toContain("secret-token")
    expect(result.snapshot.MISSING_TOKEN).toBeNull()
    expect(result.missing._tag).toBe("Failure")
  })
})

describe("PylonWorkspaceMaterializer", () => {
  const runner: WorkspaceCheckoutRunner = async (workingDirectory) => {
    await mkdir(workingDirectory, { recursive: true })
    await writeFile(join(workingDirectory, "checked-out"), "ok\n")
  }

  test("scoped workspaces are released after normal use", async () => {
    await withTempDir("pylon-services-workspace-", async (cacheRoot) => {
      let workspacePath = ""
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const materializer = yield* PylonWorkspaceMaterializer
            const workspace = yield* materializer.scopedGitCheckout({
              cacheRoot,
              checkout,
              checkoutRunner: runner,
              leaseRef: "lease.public.pylon.services.normal",
              refPrefix: "workspace.pylon.codex_agent_task",
            })
            workspacePath = workspace.workingDirectory
            expect(existsSync(join(workspacePath, "checked-out"))).toBe(true)
          }),
        ).pipe(Effect.provide(PylonWorkspaceMaterializerLive)),
      )
      expect(workspacePath).not.toBe("")
      expect(existsSync(workspacePath)).toBe(false)
    })
  })

  test("scoped workspaces are released when the fiber is interrupted", async () => {
    await withTempDir("pylon-services-workspace-", async (cacheRoot) => {
      const workspacePath = await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const acquired = yield* Deferred.make<string>()
            const materializer = yield* PylonWorkspaceMaterializer
            const fiber = yield* Effect.forkScoped(
              Effect.scoped(
                Effect.gen(function* () {
                  const workspace = yield* materializer.scopedGitCheckout({
                    cacheRoot,
                    checkout,
                    checkoutRunner: runner,
                    leaseRef: "lease.public.pylon.services.interrupted",
                    refPrefix: "workspace.pylon.codex_agent_task",
                  })
                  yield* Deferred.succeed(acquired, workspace.workingDirectory)
                  yield* Effect.never
                }),
              ),
            )
            const path = yield* Deferred.await(acquired)
            expect(existsSync(join(path, "checked-out"))).toBe(true)
            yield* Fiber.interrupt(fiber)
            return path
          }),
        ).pipe(Effect.provide(PylonWorkspaceMaterializerLive)),
      )
      expect(existsSync(workspacePath)).toBe(false)
    })
  })
})
