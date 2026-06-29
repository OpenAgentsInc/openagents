import { describe, expect, test } from "bun:test"
import { existsSync, mkdtempSync } from "node:fs"
import { mkdir, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect, Deferred } from "effect"
import { logMessage, makePylonNodeRuntime, setWalletStatus } from "../src/node/runtime"
import { collectPylonAppleFmStatus } from "../src/node/apple-fm-status"
import {
  assertControlBindSafe,
  captureNodeSnapshot,
  ensureControlToken,
  isLoopbackHostname,
  startControlServer,
  type PylonSnapshot,
} from "../src/node/control-server"
import {
  consumeSseBuffer,
  nextBackoffMs,
  runControlClient,
  sendControlCommand,
} from "../src/node/control-client"
import {
  codexControlSessionExecutionSettings,
  createControlSessionActions,
  type ControlSessionActions,
  type ControlSessionExecutor,
} from "../src/node/control-sessions"
import type { PylonEvent } from "../src/node/state"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { PYLON_DEV_CHECK_SCHEMA, type PylonDevCheckProjection } from "../src/dev-loop"
import {
  workspaceLeaseRecordFor,
  type WorkspaceCheckoutRunner,
} from "../src/workspace-materializer"
import { projectVirtualMergeQueue } from "../src/blueprint-gates/virtual-merge-queue"

const appleFmControlEnv = {
  PYLON_HOME: "/tmp/pylon-apple-fm-control-test",
  PROBE_APPLE_FM_BASE_URL: "http://user:secret@127.0.0.1:11435/?token=hidden",
}

const stubActions = (calls: Array<Record<string, unknown>>) => ({
  walletSend: async (destinationRef: string, amountSats?: number) => {
    calls.push({ type: "send", destinationRef, amountSats })
    return { dispatched: true }
  },
  walletReceive: async (amountSats: number) => {
    calls.push({ type: "receive", amountSats })
    return { invoice: "lnbc-test" }
  },
  walletAdmitPayoutTarget: async (kind: string, ref: string) => {
    calls.push({ type: "admit", kind, ref })
    return { admitted: true }
  },
})

const appleFmStatusAction = (fetchImpl: typeof fetch) => {
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json", "--pylon-ref", "pylon.test.apple-fm"]), appleFmControlEnv)
  return () =>
    collectPylonAppleFmStatus({
      summary,
      env: appleFmControlEnv,
      fetch: fetchImpl,
      now: new Date("2026-06-15T00:00:00.000Z"),
    })
}

const appleFmFetch = (handler: (url: URL) => Response | Promise<Response>): typeof fetch =>
  (async (input: RequestInfo | URL) => {
    const url = new URL(input instanceof Request ? input.url : input.toString())
    return handler(url)
  }) as typeof fetch

async function runCommand(args: string[], cwd: string): Promise<void> {
  const proc = Bun.spawn(args, { cwd, stderr: "pipe", stdout: "pipe" })
  const [stderr, exitCode] = await Promise.all([new Response(proc.stderr).text(), proc.exited])
  if (exitCode !== 0) throw new Error(`command failed: ${args.join(" ")}\n${stderr}`)
}

const cleanGitCheckoutRunner: WorkspaceCheckoutRunner = async (workingDirectory) => {
  await mkdir(workingDirectory, { recursive: true })
  await runCommand(["git", "init"], workingDirectory)
  await runCommand(["git", "config", "user.email", "fixture@test.local"], workingDirectory)
  await runCommand(["git", "config", "user.name", "Fixture"], workingDirectory)
  await writeFile(join(workingDirectory, "README.md"), "managed workspace fixture\n")
  await runCommand(["git", "add", "."], workingDirectory)
  await runCommand(["git", "commit", "-m", "fixture"], workingDirectory)
}

function fakeDevCheck(state: PylonDevCheckProjection["state"] = "passed"): PylonDevCheckProjection {
  return {
    schema: PYLON_DEV_CHECK_SCHEMA,
    observedAt: "2026-06-13T00:00:00.000Z",
    action: "check",
    state,
    changeSummary: {
      repo: { state: "not_git", rootRef: null, branch: null, commit: null },
      dirty: {
        state: "clean",
        changedCount: 0,
        stagedCount: 0,
        unstagedCount: 0,
        untrackedCount: 0,
      },
      changedFileRefs: [],
      areaRefs: [],
      blockerRefs: [],
    },
    checkPlan: {
      state: "ready",
      commandRefs: ["command.pylon.control_session.test"],
      blockerRefs: [],
    },
    commandResults: [],
    latestRecordRef: null,
    branchUntouched: true,
    commitUntouched: true,
    pushPerformed: false,
    blockerRefs: [],
  }
}

async function withControlSessionFixture<T>(fn: (fixture: {
  accountHome: string
  proofDir: string
  pylonHome: string
  summary: ReturnType<typeof createBootstrapSummary>
  worktree: string
}) => Promise<T>) {
  const root = mkdtempSync(join(tmpdir(), "pylon-control-session-"))
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

describe("control protocol", () => {
  test("loopback hostname classifier treats wildcard binds as external", () => {
    expect(isLoopbackHostname("127.0.0.1")).toBe(true)
    expect(isLoopbackHostname("localhost")).toBe(true)
    expect(isLoopbackHostname("::1")).toBe(true)
    expect(isLoopbackHostname("203.0.113.10")).toBe(false)
    expect(isLoopbackHostname("0.0.0.0")).toBe(false)
  })

  test("control bind safety requires a token for non-loopback hosts", () => {
    expect(() => assertControlBindSafe({ hostname: "127.0.0.1" })).not.toThrow()
    expect(() => assertControlBindSafe({ hostname: "0.0.0.0", token: "test-token" })).not.toThrow()
    expect(() => assertControlBindSafe({ hostname: "0.0.0.0" })).toThrow(/without a bearer token/)
  })

  test("token file is created once with stable content", async () => {
    const dir = mkdtempSync(join(tmpdir(), "pylon-token-"))
    const first = await ensureControlToken(dir)
    const second = await ensureControlToken(dir)
    expect(first).toBe(second)
    expect(first.length).toBeGreaterThanOrEqual(32)
  })

  test("snapshot captures panes and the log tail", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const runtime = yield* makePylonNodeRuntime
        yield* setWalletStatus(runtime, { daemonOnline: true, balanceSats: 21, readiness: "receive-ready" })
        yield* logMessage(runtime, "info", "hello snapshot")
        const snapshot = yield* captureNodeSnapshot(runtime)
        expect(snapshot.wallet.balanceSats).toBe(21)
        expect(snapshot.logFeed.some((entry) => entry.message === "hello snapshot")).toBe(true)
      }),
    )
  })

  test("attach round trip: snapshot, live events, auth, and money commands", async () => {
    const calls: Array<Record<string, unknown>> = []
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          yield* logMessage(runtime, "info", "pre-attach line")
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: stubActions(calls),
            port: 0,
          })

          // Unauthorized requests are rejected.
          const bad = yield* Effect.promise(() =>
            fetch(`${server.url}/events`, { headers: { authorization: "Bearer wrong" } }),
          )
          expect(bad.status).toBe(401)

          // Client: snapshot then live tail.
          const snapshotReceived = yield* Deferred.make<PylonSnapshot>()
          const liveReceived = yield* Deferred.make<PylonEvent>()
          yield* runControlClient(server.url, "test-token-0123456789abcdef", {
            onSnapshot: (snapshot) => {
              Effect.runFork(Deferred.succeed(snapshotReceived, snapshot))
            },
            onEvent: (event) => {
              if (event.type === "log" && event.message === "live line") {
                Effect.runFork(Deferred.succeed(liveReceived, event))
              }
            },
            onStatus: () => {},
          })

          const snapshot = yield* Deferred.await(snapshotReceived)
          expect(snapshot.logFeed.some((entry) => entry.message === "pre-attach line")).toBe(true)

          yield* logMessage(runtime, "info", "live line")
          const live = yield* Deferred.await(liveReceived)
          expect(live.type).toBe("log")

          // Money command round-trips and executes node-side.
          const result = yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", {
              type: "wallet.send",
              destinationRef: "lno-test-offer",
              amountSats: 42,
            }),
          )
          expect(result).toEqual({ dispatched: true })
          expect(calls).toContainEqual({ type: "send", destinationRef: "lno-test-offer", amountSats: 42 })

          // Bad command surfaces a typed error.
          const bogus = yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", {
              type: "wallet.bogus",
            } as never).then(
              () => null,
              (error: unknown) => (error instanceof Error ? error.message : String(error)),
            ),
          )
          expect(bogus).toMatch(/unknown command/)
        }),
      ),
    )
  })

  test("control command plans supervisor virtual merge queue PR fast-forward", async () => {
    const actualHead = "a".repeat(40)
    const firstHead = "b".repeat(40)
    const secondHead = "c".repeat(40)
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: stubActions([]),
            port: 0,
          })
          const projection = projectVirtualMergeQueue({
            actualHead,
            candidates: [
              {
                candidateRef: "candidate.public.pylon_vmq.issue_6695",
                issueNumber: 6695,
                baseCommit: actualHead,
                patchCommit: firstHead,
                verificationPassed: true,
                issueOpen: true,
                hasOpenPullRequest: false,
                queuedAt: "2026-06-28T00:00:01.000Z",
              },
              {
                candidateRef: "candidate.public.pylon_vmq.issue_6696",
                issueNumber: 6696,
                baseCommit: firstHead,
                patchCommit: secondHead,
                verificationPassed: true,
                issueOpen: true,
                hasOpenPullRequest: false,
                queuedAt: "2026-06-28T00:00:02.000Z",
              },
            ],
          })

          const result = yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", {
              type: "virtual_merge_queue.pr_fast_forward.plan",
              projection,
              request: {
                supervisorRef: "supervisor.public.pylon_vmq.sig_abcdef",
                repository: "OpenAgentsInc/openagents",
                branch: "main",
                actualHead,
                prNumber: 6695,
                prBaseCommit: actualHead,
                prHeadCommit: firstHead,
                promotionRef: projection.nextActualPromotion?.promotionRef ?? "",
              },
            }),
          )

          expect(result).toMatchObject({
            schema: "openagents.pylon.virtual_merge_queue.pr_fast_forward.v1",
            state: "ready",
            issueNumber: 6695,
            expectedActualHeadBefore: actualHead,
            fastForwardHead: firstHead,
            nextVirtualHeadAfter: secondHead,
          })
          expect((result as { commands: Array<{ kind: string }> }).commands.map((command) => command.kind)).toEqual([
            "git_fetch_pr_head",
            "git_verify_fetched_pr_head",
            "git_checkout_branch",
            "git_reset_actual_head",
            "git_merge_ff_only",
            "git_push_branch",
          ])
        }),
      ),
    )
  })

  test("operator account status endpoint is bearer gated and no-store", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: {
              ...stubActions([]),
              accountsStatus: async () => ({
                schema: "openagents.pylon.operator_account_status.v0.1",
                observedAt: "2026-06-28T00:00:00.000Z",
                accounts: [
                  {
                    accountRefHash: "account.pylon.codex.abc",
                    provider: "codex",
                    isRateLimited: false,
                    quotaState: "available",
                    cooldownExpiresAt: null,
                    hourlyCap: null,
                    hourlyUsage: null,
                    weeklyCap: null,
                    weeklyUsage: null,
                    manualResetsRemaining: null,
                    resetAllowed: false,
                  },
                ],
              }),
            },
            port: 0,
          })

          const denied = yield* Effect.promise(() =>
            fetch(`${server.url}/api/operator/accounts/status`, {
              headers: { authorization: "Bearer wrong" },
            }),
          )
          expect(denied.status).toBe(403)
          expect(denied.headers.get("cache-control")).toBe("no-store")
          const deniedBody = yield* Effect.promise(() => denied.text())
          expect(deniedBody).not.toContain("account.pylon.codex.abc")

          const allowed = yield* Effect.promise(() =>
            fetch(`${server.url}/api/operator/accounts/status`, {
              headers: { authorization: "Bearer test-token-0123456789abcdef" },
            }),
          )
          expect(allowed.status).toBe(200)
          expect(allowed.headers.get("cache-control")).toBe("no-store")
          const body = yield* Effect.promise(() =>
            allowed.json() as Promise<{ accounts: Array<{ accountRefHash: string }> }>,
          )
          expect(body.accounts[0]?.accountRefHash).toBe("account.pylon.codex.abc")
        }),
      ),
    )
  })

  test("operator manual account reset endpoint is bearer gated and no-store", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const calls: Array<Record<string, unknown>> = []
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: {
              ...stubActions([]),
              accountsStatus: async (input?: { accountRef?: string; reset?: boolean }) => {
                calls.push({ input })
                return {
                  schema: "openagents.pylon.accounts_status.v0.1",
                  observedAt: "2026-06-28T00:00:00.000Z",
                  accounts: [
                    {
                      accountRefHash: "account.pylon.codex.abc",
                      provider: "codex",
                      accountRef: "codex",
                      quota: { state: "available", cooldownExpiresAt: null },
                      manualReset: { performed: true, manualResetsRemaining: 2 },
                    },
                  ],
                  blockerRefs: [],
                }
              },
            },
            port: 0,
          })

          const denied = yield* Effect.promise(() =>
            fetch(`${server.url}/api/operator/accounts/codex/manual-reset`, {
              method: "POST",
              headers: { authorization: "Bearer wrong" },
            }),
          )
          expect(denied.status).toBe(403)
          expect(denied.headers.get("cache-control")).toBe("no-store")
          const deniedBody = yield* Effect.promise(() => denied.text())
          expect(deniedBody).not.toContain("account.pylon.codex.abc")

          const allowed = yield* Effect.promise(() =>
            fetch(`${server.url}/api/operator/accounts/codex/manual-reset`, {
              method: "POST",
              headers: { authorization: "Bearer test-token-0123456789abcdef" },
            }),
          )
          expect(allowed.status).toBe(200)
          expect(allowed.headers.get("cache-control")).toBe("no-store")
          expect(calls).toEqual([{ input: { accountRef: "codex", reset: true } }])
          const body = yield* Effect.promise(() => allowed.json() as Promise<{ accounts: unknown[] }>)
          expect(body.accounts).toHaveLength(1)
        }),
      ),
    )
  })

  test("assignments commands round-trip and report unavailability", async () => {
    const calls: Array<Record<string, unknown>> = []
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: {
              ...stubActions(calls),
              assignmentsPoll: async () => [{ leaseRef: "lease-1", goal: "g", assignmentRef: "a", paymentMode: "no-spend", expiresAt: "x" }],
              assignmentsAccept: async (leaseRef: string) => {
                calls.push({ type: "accept", leaseRef })
                return { accepted: true }
              },
            },
            port: 0,
          })
          const leases = yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "assignments.poll" }),
          )
          expect(Array.isArray(leases)).toBe(true)
          yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "assignments.accept", leaseRef: "lease-1" }),
          )
          expect(calls).toContainEqual({ type: "accept", leaseRef: "lease-1" })

          // A node without assignment actions reports unavailability.
          const bare = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: stubActions(calls),
            port: 0,
          })
          const failure = yield* Effect.promise(() =>
            sendControlCommand(bare.url, "test-token-0123456789abcdef", { type: "assignments.poll" }).then(
              () => null,
              (error: unknown) => (error instanceof Error ? error.message : String(error)),
            ),
          )
          expect(failure).toMatch(/unavailable/)
        }),
      ),
    )
  })

  test("#5207 wallet.spark_send / wallet.spark_backup_status route to the warm-session actions (and report unavailability)", async () => {
    const calls: Array<Record<string, unknown>> = []
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: {
              ...stubActions(calls),
              walletSparkSend: async (input: { destination: string; amountSats?: number; confirmSend?: boolean }) => {
                calls.push({ type: "spark_send", ...input })
                return { ok: true, state: "sent" }
              },
              walletSparkBackupStatus: async (input: { showLocalTarget?: boolean }) => {
                calls.push({ type: "spark_backup_status", ...input })
                return { ok: true, projection: { state: "ready" } }
              },
            },
            port: 0,
          })

          const sent = yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", {
              type: "wallet.spark_send",
              destination: "lnbc100n1rawpaymentrequest",
              amountSats: 100,
              confirmSend: true,
            }),
          )
          expect(sent).toEqual({ ok: true, state: "sent" })
          expect(calls).toContainEqual({
            type: "spark_send",
            destination: "lnbc100n1rawpaymentrequest",
            amountSats: 100,
            confirmSend: true,
          })

          const status = yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", {
              type: "wallet.spark_backup_status",
              showLocalTarget: false,
            }),
          )
          expect(status).toEqual({ ok: true, projection: { state: "ready" } })
          expect(calls).toContainEqual({ type: "spark_backup_status", showLocalTarget: false })

          // A node without the warm-session actions reports unavailability (the
          // CLI then falls back to its local cold path).
          const bare = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: stubActions(calls),
            port: 0,
          })
          const failure = yield* Effect.promise(() =>
            sendControlCommand(bare.url, "test-token-0123456789abcdef", {
              type: "wallet.spark_send",
              destination: "lnbc100n1rawpaymentrequest",
              amountSats: 100,
              confirmSend: true,
            }).then(
              () => null,
              (error: unknown) => (error instanceof Error ? error.message : String(error)),
            ),
          )
          expect(failure).toMatch(/unavailable/)
        }),
      ),
    )
  })

  test("apple_fm.status advertises capability only after ready live health", async () => {
    let healthCalls = 0
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: {
              ...stubActions([]),
              appleFmStatus: appleFmStatusAction(
                appleFmFetch((url) => {
                  healthCalls += 1
                  expect(url.pathname).toBe("/health")
                  return Response.json({
                    ready: true,
                    modelId: "apple-foundation-model",
                    platform: "darwin-arm64",
                    version: "fake-bridge",
                  })
                }),
              ),
            },
            port: 0,
          })

          const status = (yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "apple_fm.status" }),
          )) as Record<string, unknown>

          expect(status.schema).toBe("openagents.pylon.apple_fm.status.v0.1")
          expect(status.kind).toBe("pylon_apple_fm_status")
          expect(status.backendKind).toBe("apple_fm_bridge")
          expect(status.status).toBe("ready")
          expect(status.available).toBe(true)
          expect(status.advertisedCapabilities).toContain("probe.backend.apple_fm_bridge")
          expect(status.blockerRefs).toEqual([])
          expect(status.baseUrl).toBe("http://127.0.0.1:11435")
          expect(JSON.stringify(status)).not.toContain("secret")
          expect(JSON.stringify(status)).not.toContain("hidden")
          expect(healthCalls).toBe(1)
        }),
      ),
    )
  })

  test("apple_fm.status reports unsupported hardware without advertising capability", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: {
              ...stubActions([]),
              appleFmStatus: appleFmStatusAction(
                appleFmFetch(() =>
                  Response.json({
                    ready: false,
                    unavailableReason: "unsupported_hardware",
                    message: "device not eligible",
                  }),
                ),
              ),
            },
            port: 0,
          })

          const status = (yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "apple_fm.status" }),
          )) as Record<string, unknown>

          expect(status.status).toBe("unsupported")
          expect(status.available).toBe(false)
          expect(status.advertisedCapabilities).not.toContain("probe.backend.apple_fm_bridge")
          expect(status.blockerRefs).toContain("blocker.pylon.apple_fm.unsupported_hardware")
          expect(status.blockerRefs).toContain("blocker.pylon.apple_fm.live_health_not_ready")
          expect(JSON.stringify(status)).not.toContain("secret")
          expect(JSON.stringify(status)).not.toContain("hidden")
        }),
      ),
    )
  })

  test("apple_fm.status reports unreachable bridge without advertising capability", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: {
              ...stubActions([]),
              appleFmStatus: appleFmStatusAction(
                (async () => {
                  throw new Error("bridge offline")
                }) as typeof fetch,
              ),
            },
            port: 0,
          })

          const status = (yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "apple_fm.status" }),
          )) as Record<string, unknown>

          expect(status.status).toBe("unreachable")
          expect(status.available).toBe(false)
          expect(status.advertisedCapabilities).not.toContain("probe.backend.apple_fm_bridge")
          expect(status.blockerRefs).toContain("blocker.pylon.apple_fm.bridge_unreachable")
          expect(status.blockerRefs).toContain("blocker.pylon.apple_fm.live_health_not_ready")
          expect(JSON.stringify(status)).not.toContain("secret")
          expect(JSON.stringify(status)).not.toContain("hidden")
        }),
      ),
    )
  })

  test("apple_fm.status reports malformed health without advertising capability", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: {
              ...stubActions([]),
              appleFmStatus: appleFmStatusAction(
                appleFmFetch(() => new Response("{", { headers: { "content-type": "application/json" } })),
              ),
            },
            port: 0,
          })

          const status = (yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "apple_fm.status" }),
          )) as Record<string, unknown>

          expect(status.status).toBe("malformed")
          expect(status.available).toBe(false)
          expect(status.advertisedCapabilities).not.toContain("probe.backend.apple_fm_bridge")
          expect(status.blockerRefs).toContain("blocker.pylon.apple_fm.malformed_health")
          expect(status.blockerRefs).toContain("blocker.pylon.apple_fm.live_health_not_ready")
          expect(JSON.stringify(status)).not.toContain("secret")
          expect(JSON.stringify(status)).not.toContain("hidden")
        }),
      ),
    )
  })

  test("local control sessions honor the owner supervised Codex dev overlay", () => {
    expect(
      codexControlSessionExecutionSettings(
        { sandboxMode: "workspace-write" },
        { codexExecutionMode: "local_supervised_danger" },
      ),
    ).toEqual({
      executionMode: "local_supervised_danger",
      networkAccessEnabled: true,
      sandboxMode: "danger-full-access",
    })

    expect(codexControlSessionExecutionSettings({}, {})).toEqual({
      executionMode: "local_bounded",
      networkAccessEnabled: false,
      sandboxMode: "workspace-write",
    })
  })

  test("desktop PYLON_CODEX_NO_SANDBOX opt-in runs full-access with network (owner mandate)", () => {
    // The desktop forwards this node-boot env so its Codex control sessions are
    // never sandboxed (git / GitHub / credentials work). It is a local env, not
    // a wire field, so the remote-spawn danger-mode rejection is unaffected.
    expect(
      codexControlSessionExecutionSettings({ sandboxMode: "workspace-write" }, {}, {
        PYLON_CODEX_NO_SANDBOX: "1",
      }),
    ).toEqual({
      executionMode: "local_supervised_danger",
      networkAccessEnabled: true,
      sandboxMode: "danger-full-access",
    })

    // Off / unset stays bounded + sandboxed.
    expect(
      codexControlSessionExecutionSettings({}, {}, { PYLON_CODEX_NO_SANDBOX: "0" }),
    ).toEqual({
      executionMode: "local_bounded",
      networkAccessEnabled: false,
      sandboxMode: "workspace-write",
    })
  })

  test("session commands spawn, list, retain artifacts, and replay per-session events", async () => {
    await withControlSessionFixture(async ({ accountHome, proofDir, summary, worktree }) => {
      const calls: Array<Record<string, unknown>> = []
      const executor: ControlSessionExecutor = async (input) => {
        calls.push({
          adapter: input.adapter,
          accountRefHash: input.account?.accountRefHash,
          cwd: input.cwd,
        })
        input.emit({ phase: "composer_event", message: "fake composer event", composerEventIndex: 1 })
        input.emit({ phase: "dev_check_started" })
        return {
          commandCount: 1,
          devCheck: fakeDevCheck("passed"),
          editedFileCount: 1,
          eventCount: 2,
          externalSessionRef: "session.pylon.fake.external",
          responseDigestRef: "digest.pylon.fake.response",
          totalTokens: 3,
        }
      }
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const runtime = yield* makePylonNodeRuntime
            const server = yield* startControlServer(runtime, {
              token: "test-token-0123456789abcdef",
              actions: {
                ...stubActions(calls),
                sessions: createControlSessionActions({
                  executor,
                  proofsDir: proofDir,
                  summary,
                }),
              },
              port: 0,
            })

            const spawned = (yield* Effect.promise(() =>
              sendControlCommand(server.url, "test-token-0123456789abcdef", {
                type: "session.spawn",
                adapter: "codex",
                accountRef: "codex-a",
                worktreePath: worktree,
                objective: "run a bounded fake control session",
                verify: ["bun", "--version"],
              }),
            )) as { sessionRef: string; state: string }
            expect(spawned.sessionRef).toStartWith("session.pylon.control.")

            let list = [] as Array<{ sessionRef: string; state: string; artifactRef: string | null }>
            for (let attempt = 0; attempt < 20; attempt += 1) {
              list = (yield* Effect.promise(() =>
                sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "session.list" }),
              )) as typeof list
              if (list[0]?.state === "completed") break
              yield* Effect.sleep("10 millis")
            }
            expect(list[0]?.state).toBe("completed")
            expect(list[0]?.artifactRef).toStartWith("artifact.pylon.control_session.proof.")
            expect(JSON.stringify(list)).not.toContain(accountHome)
            expect(JSON.stringify(list)).not.toContain(worktree)
            expect(calls).toContainEqual(expect.objectContaining({ adapter: "codex", cwd: worktree }))

            const eventsInfo = (yield* Effect.promise(() =>
              sendControlCommand(server.url, "test-token-0123456789abcdef", {
                type: "session.events",
                sessionRef: spawned.sessionRef,
              }),
            )) as { eventsPath: string; sessionRef: string; state: string }
            expect(eventsInfo.eventsPath).toContain(encodeURIComponent(spawned.sessionRef))

            const eventsResponse = yield* Effect.promise(() =>
              fetch(`${server.url}${eventsInfo.eventsPath}`, {
                headers: { authorization: "Bearer test-token-0123456789abcdef" },
              }),
            )
            expect(eventsResponse.ok).toBe(true)
            const eventFrames: string[] = []
            const eventText = yield* Effect.promise(() => eventsResponse.text())
            consumeSseBuffer(eventText, payload => eventFrames.push(payload))
            const phases = eventFrames.map(frame => (JSON.parse(frame) as { phase: string }).phase)
            expect(phases).toContain("queued")
            expect(phases).toContain("started")
            expect(phases).toContain("composer_event")
            expect(phases).toContain("completed")
            expect(eventFrames.join("\n")).not.toContain(worktree)
            expect(eventFrames.join("\n")).not.toContain(accountHome)

            const artifact = yield* Effect.promise(() =>
              readFile(join(proofDir, `${spawned.sessionRef}-proof.json`), "utf8"),
            )
            expect(artifact).toContain('"schema": "openagents.pylon.control_session_artifact.v0.1"')
            expect(artifact).not.toContain(worktree)
            expect(artifact).not.toContain(accountHome)
          }),
        ),
      )
    })
  })

  test("session.reply spawns a continuation session with parent context and safe projections", async () => {
    await withControlSessionFixture(async ({ accountHome, proofDir, summary, worktree }) => {
      const calls: Array<{
        accountRefHash: string | null | undefined
        cwd: string
        objective: string
        sessionRef: string
        verify: string[]
      }> = []
      const executor: ControlSessionExecutor = async (input) => {
        calls.push({
          accountRefHash: input.account?.accountRefHash,
          cwd: input.cwd,
          objective: input.objective,
          sessionRef: input.sessionRef,
          verify: input.verify,
        })
        return {
          commandCount: 1,
          devCheck: fakeDevCheck("passed"),
          editedFileCount: 1,
          eventCount: 1,
          externalSessionRef: null,
          responseDigestRef: null,
          totalTokens: 1,
        }
      }
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const runtime = yield* makePylonNodeRuntime
            const server = yield* startControlServer(runtime, {
              token: "test-token-0123456789abcdef",
              actions: {
                ...stubActions([]),
                sessions: createControlSessionActions({
                  executor,
                  proofsDir: proofDir,
                  summary,
                }),
              },
              port: 0,
            })

            const parent = (yield* Effect.promise(() =>
              sendControlCommand(server.url, "test-token-0123456789abcdef", {
                type: "session.spawn",
                adapter: "codex",
                accountRef: "codex-a",
                worktreePath: worktree,
                objective: "add a health route",
                verify: ["bun", "--version"],
              }),
            )) as { sessionRef: string; state: string }
            for (let attempt = 0; attempt < 20; attempt += 1) {
              const list = (yield* Effect.promise(() =>
                sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "session.list" }),
              )) as Array<{ sessionRef: string; state: string }>
              if (list.find((entry) => entry.sessionRef === parent.sessionRef)?.state === "completed") break
              yield* Effect.sleep("10 millis")
            }

            const reply = (yield* Effect.promise(() =>
              sendControlCommand(server.url, "test-token-0123456789abcdef", {
                type: "session.reply",
                sessionRef: parent.sessionRef,
                objective: "now add a regression test",
                timeoutSeconds: 5,
              }),
            )) as { sessionRef: string; parentSessionRef: string; state: string }
            expect(reply.sessionRef).toStartWith("session.pylon.control.")
            expect(reply.sessionRef).not.toBe(parent.sessionRef)
            expect(reply.parentSessionRef).toBe(parent.sessionRef)

            let list = [] as Array<{
              sessionRef: string
              parentSessionRef: string | null
              state: string
            }>
            for (let attempt = 0; attempt < 20; attempt += 1) {
              list = (yield* Effect.promise(() =>
                sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "session.list" }),
              )) as typeof list
              if (list.find((entry) => entry.sessionRef === reply.sessionRef)?.state === "completed") break
              yield* Effect.sleep("10 millis")
            }
            const child = list.find((entry) => entry.sessionRef === reply.sessionRef)
            expect(child?.state).toBe("completed")
            expect(child?.parentSessionRef).toBe(parent.sessionRef)
            expect(JSON.stringify(list)).not.toContain(accountHome)
            expect(JSON.stringify(list)).not.toContain(worktree)

            expect(calls).toHaveLength(2)
            expect(calls[1]?.cwd).toBe(worktree)
            expect(calls[1]?.verify).toEqual(["bun", "--version"])
            expect(calls[1]?.accountRefHash).toBe(calls[0]?.accountRefHash)
            expect(calls[1]?.objective).toContain("Continue the current coding session")
            expect(calls[1]?.objective).toContain("add a health route")
            expect(calls[1]?.objective).toContain("now add a regression test")
          }),
        ),
      )
    })
  })

  test("bridge turn.steer routes to session.reply under send_instruction capability", async () => {
    const calls: Array<{ sessionRef: string; objective: string; timeoutSeconds?: number }> = []
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          const sessions: ControlSessionActions = {
            spawn: async () => ({ sessionRef: "session.spawned", state: "queued" }),
            reply: async (command) => {
              calls.push({
                sessionRef: command.sessionRef,
                objective: command.objective,
                ...(command.timeoutSeconds === undefined ? {} : { timeoutSeconds: command.timeoutSeconds }),
              })
              return {
                sessionRef: "session.child.1",
                parentSessionRef: command.sessionRef,
                state: "queued",
              }
            },
            startAppleFm: async () => ({
              ok: false,
              sessionRef: "",
              blockerRefs: ["blocker.test.apple_fm.unavailable"],
              error: "unavailable",
            }),
            list: async () => [],
            cancel: async (sessionRef) => ({
              sessionRef,
              parentSessionRef: null,
              adapter: "codex",
              lane: "auto",
              account: null,
              accountRefHash: null,
              objectiveRef: "objective",
              workspaceRef: "workspace",
              workspaceCleanupRef: null,
              workspaceCleanupReceiptRef: null,
              workspaceRetentionReasonRef: null,
              objectiveDigestRef: "objective",
              verifyRef: "verify",
              state: "cancelled",
              artifactRef: null,
              resultRef: null,
              errorClass: null,
              errorDigestRef: null,
              createdAt: "2026-06-19T00:00:00.000Z",
              startedAt: null,
              completedAt: "2026-06-19T00:00:01.000Z",
              updatedAt: "2026-06-19T00:00:01.000Z",
              eventCount: 0,
              latestActivity: "",
              cloudRunner: null,
              resourceUsageReceiptRef: null,
            }),
            events: async (sessionRef) => ({
              sessionRef,
              eventsPath: "",
              state: "queued",
              recentEvents: [],
            }),
            artifact: async (sessionRef) => ({
              sessionRef,
              kind: "none",
              artifact: null,
            }),
            eventStream: () => new ReadableStream<Uint8Array>({
              start(controller) {
                controller.close()
              },
            }),
          }
          const server = yield* startControlServer(runtime, {
            token: "test-token-0123456789abcdef",
            actions: { ...stubActions([]), sessions },
            port: 0,
          })

          const boot = (yield* Effect.promise(() =>
            sendControlCommand(server.url, "test-token-0123456789abcdef", {
              type: "bridge.issueBootstrap",
            }),
          )) as { bootstrapId: string; secret: string }
          const pair = yield* Effect.promise(() =>
            fetch(`${server.url}/bridge/pair`, {
              method: "POST",
              headers: { "content-type": "application/json" },
              body: JSON.stringify({
                bootstrapId: boot.bootstrapId,
                secret: boot.secret,
                clientId: "mobile",
                deviceClass: "ios",
                capabilities: ["observe_public", "send_instruction"],
                projectionLevel: "public_safe",
              }),
            }),
          )
          expect(pair.status).toBe(200)
          const pairJson = (yield* Effect.promise(() => pair.json())) as {
            ok?: boolean
            claims?: { pairingRef?: unknown; jti?: unknown }
          }
          expect(pairJson.ok).toBe(true)
          const pairingRef = String(pairJson.claims?.pairingRef ?? "")
          const jti = String(pairJson.claims?.jti ?? "")

          const steer = yield* Effect.promise(() =>
            fetch(`${server.url}/bridge`, {
              method: "POST",
              headers: {
                authorization: `Bridge ${pairingRef}:${jti}`,
                "content-type": "application/json",
              },
              body: JSON.stringify({
                verb: "turn.steer",
                clientRequestId: "client.turn.1",
                idempotencyKey: "client.turn.1",
                pairingRef,
                capabilityRef: "send_instruction",
                sessionRef: "session.parent.1",
                instruction: "continue with the regression test",
                timeoutSeconds: 120,
              }),
            }),
          )
          expect(steer.status).toBe(200)
          const steerJson = (yield* Effect.promise(() => steer.json())) as {
            ok?: boolean
            result?: { sessionRef?: unknown; parentSessionRef?: unknown; state?: unknown }
          }
          expect(steerJson).toMatchObject({
            ok: true,
            result: {
              sessionRef: "session.child.1",
              parentSessionRef: "session.parent.1",
              state: "queued",
            },
          })
          expect(calls).toEqual([
            {
              sessionRef: "session.parent.1",
              objective: "continue with the regression test",
              timeoutSeconds: 120,
            },
          ])
        }),
      ),
    )
  })

  test("repoRef sessions release clean managed worktrees and retain dirty closeouts", async () => {
    await withControlSessionFixture(async ({ proofDir, summary }) => {
      const executor: ControlSessionExecutor = async (input) => {
        if (input.objective.includes("dirty")) {
          await writeFile(join(input.cwd, "dirty-output.txt"), "dirty\n")
        }
        return {
          commandCount: 1,
          devCheck: fakeDevCheck("passed"),
          editedFileCount: input.objective.includes("dirty") ? 1 : 0,
          eventCount: 1,
          externalSessionRef: null,
          responseDigestRef: null,
          totalTokens: 1,
        }
      }
      const actions = createControlSessionActions({
        executor,
        proofsDir: proofDir,
        summary,
        workspaceCheckoutRunner: cleanGitCheckoutRunner,
      })
      const repoRef = {
        provider: "github" as const,
        visibility: "public" as const,
        fullName: "OpenAgentsInc/openagents",
        branch: "main",
        commitSha: "3333333333333333333333333333333333333333",
      }
      const clean = await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        repoRef,
        objective: "clean managed session",
        verify: ["bun", "--version"],
      })
      const dirty = await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        repoRef,
        objective: "dirty managed session",
        verify: ["bun", "--version"],
      })

      let list = await actions.list()
      for (let attempt = 0; attempt < 20; attempt += 1) {
        list = await actions.list()
        const cleanState = list.find((entry) => entry.sessionRef === clean.sessionRef)?.state
        const dirtyState = list.find((entry) => entry.sessionRef === dirty.sessionRef)?.state
        if (cleanState === "completed" && dirtyState === "completed") break
        await Bun.sleep(10)
      }
      const cleanRow = list.find((entry) => entry.sessionRef === clean.sessionRef)
      const dirtyRow = list.find((entry) => entry.sessionRef === dirty.sessionRef)
      expect(cleanRow?.workspaceCleanupRef).toStartWith("cleanup.pylon.workspace.")
      expect(cleanRow?.workspaceCleanupReceiptRef).toStartWith("receipt.pylon.workspace_cleanup.")
      expect(cleanRow?.workspaceRetentionReasonRef).toBeNull()
      expect(dirtyRow?.workspaceCleanupRef).toStartWith("cleanup.pylon.workspace.")
      expect(dirtyRow?.workspaceCleanupReceiptRef).toBeNull()
      expect(dirtyRow?.workspaceRetentionReasonRef).toBe("retention.workspace.dirty")

      const workspaceStateRoot = join(summary.paths.cache, "workspace-leases")
      const cleanLease = await workspaceLeaseRecordFor({
        workspaceStateRoot,
        workspaceRef: cleanRow?.workspaceRef ?? "",
      })
      const dirtyLease = await workspaceLeaseRecordFor({
        workspaceStateRoot,
        workspaceRef: dirtyRow?.workspaceRef ?? "",
      })
      expect(cleanLease?.state).toBe("cleaned")
      expect(cleanLease?.cleanupReceiptRef).toBe(cleanRow?.workspaceCleanupReceiptRef)
      expect(cleanLease?.local.workingDirectory && existsSync(cleanLease.local.workingDirectory)).toBe(false)
      expect(dirtyLease?.state).toBe("materialized")
      expect(dirtyLease?.retentionReasonRef).toBe("retention.workspace.dirty")
      expect(dirtyLease?.local.workingDirectory && existsSync(dirtyLease.local.workingDirectory)).toBe(true)
    })
  })

  test("session.cancel aborts a running fake executor and records cancelled state", async () => {
    await withControlSessionFixture(async ({ proofDir, summary, worktree }) => {
      let started!: () => void
      const startedPromise = new Promise<void>(resolve => {
        started = resolve
      })
      const executor: ControlSessionExecutor = async (input) => {
        started()
        await new Promise((_resolve, reject) => {
          input.abortSignal.addEventListener("abort", () => reject(new Error("cancelled")), { once: true })
        })
        return {
          commandCount: 0,
          devCheck: fakeDevCheck("skipped"),
          editedFileCount: 0,
          eventCount: 0,
          externalSessionRef: null,
          responseDigestRef: null,
          totalTokens: 0,
        }
      }
      await Effect.runPromise(
        Effect.scoped(
          Effect.gen(function* () {
            const runtime = yield* makePylonNodeRuntime
            const server = yield* startControlServer(runtime, {
              token: "test-token-0123456789abcdef",
              actions: {
                ...stubActions([]),
                sessions: createControlSessionActions({ executor, proofsDir: proofDir, summary }),
              },
              port: 0,
            })
            const spawned = (yield* Effect.promise(() =>
              sendControlCommand(server.url, "test-token-0123456789abcdef", {
                type: "session.spawn",
                adapter: "codex",
                worktreePath: worktree,
                objective: "cancel this bounded session",
                verify: ["bun", "--version"],
              }),
            )) as { sessionRef: string }
            yield* Effect.promise(() => startedPromise)
            const cancelled = (yield* Effect.promise(() =>
              sendControlCommand(server.url, "test-token-0123456789abcdef", {
                type: "session.cancel",
                sessionRef: spawned.sessionRef,
              }),
            )) as { state: string; errorClass: string | null }
            expect(cancelled.state).toBe("cancelled")
            expect(cancelled.errorClass).toBe("cancelled")
            const list = (yield* Effect.promise(() =>
              sendControlCommand(server.url, "test-token-0123456789abcdef", { type: "session.list" }),
            )) as Array<{ state: string }>
            expect(list[0]?.state).toBe("cancelled")
          }),
        ),
      )
    })
  })

  test("session failure classifies dev check before redaction scan field text", async () => {
    await withControlSessionFixture(async ({ proofDir, summary, worktree }) => {
      const executor: ControlSessionExecutor = async () => {
        throw new Error("dev check did not pass; proof stdout included redactionScan")
      }
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        worktreePath: worktree,
        objective: "fail this bounded session",
        verify: ["bun", "--version"],
      })
      let list = [] as Array<{ state: string; errorClass: string | null }>
      for (let attempt = 0; attempt < 20; attempt += 1) {
        list = await actions.list()
        if (list[0]?.state === "failed") break
        await Bun.sleep(10)
      }
      expect(list[0]?.state).toBe("failed")
      expect(list[0]?.errorClass).toBe("verification_failed")
    })
  })

  test("backoff doubles to a 30s ceiling", () => {
    expect(nextBackoffMs(0)).toBe(1000)
    expect(nextBackoffMs(1000)).toBe(2000)
    expect(nextBackoffMs(16000)).toBe(30000)
    expect(nextBackoffMs(30000)).toBe(30000)
  })

  test("SSE buffer parser handles partial frames and comments", () => {
    const seen: string[] = []
    let rest = consumeSseBuffer('data: {"a":1}\n\n: ping\n\ndata: {"b"', (payload) => seen.push(payload))
    expect(seen).toEqual(['{"a":1}'])
    expect(rest).toBe('data: {"b"')
    rest = consumeSseBuffer(`${rest}:2}\n\n`, (payload) => seen.push(payload))
    expect(seen).toEqual(['{"a":1}', '{"b":2}'])
    expect(rest).toBe("")
  })
})
