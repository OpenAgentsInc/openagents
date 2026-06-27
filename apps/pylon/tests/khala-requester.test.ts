import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import {
  buildPylonKhalaGitCheckoutWorkspace,
  buildPylonKhalaChatRequestBody,
  durableRequestIdFromUrl,
  evaluatePylonKhalaCloseoutChecklist,
  evaluatePylonKhalaProofChecklist,
  issuePylonKhalaRequest,
  readPylonKhalaAssignmentTraceStatus,
  readPylonKhalaCloseout,
  readPylonKhalaProof,
  readPylonKhalaStatus,
  resumePylonKhalaRequest,
} from "../src/khala-requester"
import { ensurePylonLocalState } from "../src/state"

const sse = (id: string, content: string) =>
  `data: ${JSON.stringify({
    choices: [{ delta: { content }, index: 0 }],
    id,
    object: "chat.completion.chunk",
  })}\n\ndata: [DONE]\n\n`

const servers: ReturnType<typeof Bun.serve>[] = []
const INDEX = join(import.meta.dir, "..", "src", "index.ts")
const CWD = join(import.meta.dir, "..")
type CloseoutProofResult = Parameters<typeof evaluatePylonKhalaCloseoutChecklist>[1]
type CloseoutTraceStatus = Parameters<typeof evaluatePylonKhalaCloseoutChecklist>[0]
type ProofPayload = Parameters<typeof evaluatePylonKhalaProofChecklist>[0]

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true)
})

async function withTempHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-khala-requester-test-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

async function runPylonCli(args: string[], env: Record<string, string>) {
  const proc = Bun.spawn(["bun", INDEX, ...args], {
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
      }, 10_000)
    }),
  ])
  if (timeout !== undefined) clearTimeout(timeout)
  const [stdout, stderr] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
  ])
  return { ...exit, stderr, stdout }
}

async function markAssignmentReady(home: string) {
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
    PYLON_HOME: home,
  })
  const state = await ensurePylonLocalState(summary)
  await writeFile(
    state.paths.runtimeState,
    `${JSON.stringify({
      lifecycle: "assignment-ready",
      displayName: "Khala Requester Test",
      resourceMode: "background_20",
      capabilityRefs: [],
      blockerRefs: [],
      updatedAt: "2026-06-27T00:00:00.000Z",
    })}\n`,
  )
}

function completeTraceStatus(overrides: Record<string, unknown> = {}) {
  return {
    schemaVersion: "openagents.pylon.codex_assignment_trace_status.v1",
    assignmentRef: "assignment-pylon-codex-1",
    closeoutPolicy: {
      paymentMode: "no-spend",
      payoutClaimAllowed: false,
      settlementState: "not_applicable",
      source: "worker_closeout_event",
    },
    pylonRef: "pylon-local-codex-1",
    owner: {
      agentUserRef: "agent:agent-user-1",
      openauthUserRef: "user-openauth-1",
    },
    lifecycle: {
      state: "closeout_submitted",
      createdAt: "2026-06-26T12:00:00.000Z",
      updatedAt: "2026-06-26T12:00:05.000Z",
      acceptedWorkRefs: ["accepted.pylon_codex.assignment_1"],
      artifactRefs: ["artifact.pylon_codex.assignment_1"],
      closeoutRefs: ["assignment.closeout.assignment_1"],
      proofRefs: ["proof.pylon_codex.assignment_1"],
      rejectionRefs: [],
    },
    events: {
      count: 6,
      progressCount: 3,
      latestEventKind: "worker_closeout",
      latestStatus: "closeout_submitted",
      latestObservedAt: "2026-06-26T12:00:05.000Z",
    },
    tokenUsage: {
      rowCount: 1,
      provider: "pylon-codex-own-capacity",
      model: "openagents/pylon-codex",
      usageTruth: "exact",
      demandKind: "own_capacity",
      demandSource: "khala_coding_delegation",
      inputTokens: 100,
      outputTokens: 40,
      reasoningTokens: 10,
      cacheReadTokens: 5,
      totalTokens: 140,
      status: "recorded",
    },
    traces: {
      count: 2,
      visibility: "owner_only",
      schemaVersion: "ATIF-v1.7",
      latestTraceUuid: "trace-final-1",
      finalTraceUuid: "trace-final-1",
      refs: ["trace-chunk-1", "trace-final-1"],
    },
    rawEventChunks: {
      count: 2,
      eventCount: 8,
      byteLength: 4096,
      latestChunkRef: "raw_chunk.pylon_codex.002",
      latestObservedAt: "2026-06-26T12:00:05.000Z",
      visibility: "owner_only",
    },
    rawEvents: {
      count: 2,
      eventCount: 8,
      byteLength: 4096,
      latestRawEventRef: "raw.pylon_codex.bbb",
      latestObservedAt: "2026-06-26T12:00:05.000Z",
      visibility: "owner_only",
      refs: ["raw.pylon_codex.aaa", "raw.pylon_codex.bbb"],
    },
    progress: {
      state: "closed_out",
      closeoutReady: true,
      hasLiveChunks: true,
      hasFinalTrace: true,
      hasTokenUsage: true,
    },
    generatedAt: "2026-06-26T12:00:06.000Z",
    ...overrides,
  }
}

function completeProof(overrides: Record<string, unknown> = {}) {
  return {
    schemaVersion: "openagents.pylon.codex_assignment_proof.v1",
    assignmentRef: "assignment-pylon-codex-1",
    closeoutPolicy: {
      paymentMode: "no-spend",
      payoutClaimAllowed: false,
      settlementState: "not_applicable",
      source: "worker_closeout_event",
    },
    pylonRef: "pylon-local-codex-1",
    owner: {
      agentUserRef: "agent:agent-user-1",
      openauthUserRef: "user-openauth-1",
    },
    tokenUsage: {
      rowCount: 1,
      provider: "pylon-codex-own-capacity",
      model: "openagents/pylon-codex",
      usageTruth: "exact",
      demandKind: "own_capacity",
      demandSource: "khala_coding_delegation",
      inputTokens: 100,
      outputTokens: 40,
      reasoningTokens: 10,
      cacheReadTokens: 5,
      totalTokens: 140,
    },
    traces: {
      count: 2,
      visibility: "owner_only",
      schemaVersion: "ATIF-v1.7",
      refs: ["trace-chunk-1", "trace-final-1"],
    },
    rawEvents: {
      count: 2,
      eventCount: 8,
      byteLength: 4096,
      visibility: "owner_only",
      refs: ["raw.pylon_codex.aaa", "raw.pylon_codex.bbb"],
    },
    generatedAt: "2026-06-26T12:00:06.000Z",
    ...overrides,
  }
}

function khalaAutoRunServer() {
  const assignmentRef = "assignment.public.khala_coding.cli_auto_run"
  const leaseRef = "lease.public.khala_coding.cli_auto_run"
  const requests: Array<{ body: Record<string, unknown>; method: string; path: string }> = []
  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      const text = await request.text()
      const body = text.trim() === "" ? {} : JSON.parse(text)
      requests.push({ body, method: request.method, path: url.pathname })

      if (url.pathname === "/v1/chat/completions") {
        return new Response(sse("chatcmpl_auto_run", "delegated"), {
          headers: {
            "openagents-coding-assignment-ref": assignmentRef,
            "openagents-durable-stream-url": "/v1/chat/completions/durable/chatcmpl_auto_run",
          },
        })
      }
      if (url.pathname.includes("/heartbeat")) {
        return Response.json({ heartbeatRef: `heartbeat.${body.pylonRef}.${body.sequence}` })
      }
      if (url.pathname.endsWith("/assignments")) {
        return Response.json({
          schema: "openagents.pylon.assignment_poll_response.v0.3",
          assignments: [
            {
              schema: "openagents.pylon.assignment_lease.v0.3",
              assignmentRef,
              leaseRef,
              goal: "Return a public-safe proof ref for this fake no-spend assignment.",
              paymentMode: "no-spend",
              capabilityRefs: [],
              expiresAt: "2099-01-01T00:00:00.000Z",
            },
          ],
        })
      }
      if (url.pathname.endsWith("/accept")) {
        return Response.json({ statusRef: `assignment.accepted.${assignmentRef}` })
      }
      if (url.pathname.endsWith("/progress")) {
        return Response.json({ progressRef: `assignment.progress.${leaseRef}.${body.sequence}` })
      }
      if (url.pathname.endsWith("/artifacts")) {
        return Response.json({ artifactRef: `assignment.artifacts.${leaseRef}` })
      }
      if (url.pathname.endsWith("/closeout")) {
        return Response.json({ closeoutRef: `assignment.closeout.${leaseRef}` })
      }
      return Response.json({ errorRef: "error.not_found" }, { status: 404 })
    },
  })
  servers.push(server)
  return { assignmentRef, baseUrl: `http://127.0.0.1:${server.port}`, requests }
}

describe("pylon khala requester body", () => {
  test("builds an OpenAI-compatible Khala request with a typed workflow marker", () => {
    const body = buildPylonKhalaChatRequestBody({
      prompt: "Fix the public failing test",
      targetPylonRef: "pylon.owner.codex",
      workflow: "codex_agent_task",
    })

    expect(body).toMatchObject({
      model: "openagents/khala",
      openagents: {
        coding: { targetPylonRef: "pylon.owner.codex" },
        workflowClass: "codex_agent_task",
      },
      stream: true,
      targetPylonRef: "pylon.owner.codex",
      workflowClass: "codex_agent_task",
    })
    expect((body.messages as Array<{ content: string; role: string }>)[0]).toEqual({
      content: "Fix the public failing test",
      role: "user",
    })
  })

  test("builds a workspace-backed Khala coding request", () => {
    const workspace = buildPylonKhalaGitCheckoutWorkspace({
      commit: "7ab7cb401803f6e04a6c93b7aa9102405de66419",
      repository: "OpenAgentsInc/openagents",
      verificationCommand: "bun run --cwd apps/openagents.com/workers/api test -- src/inference/coding-workflow-delegation.test.ts",
    })
    const body = buildPylonKhalaChatRequestBody({
      objectiveSummary: "Implement the public-safe issue slice and run the named verification command.",
      prompt: "Implement the public-safe issue slice and run the named verification command.",
      targetPylonRef: "pylon.owner.codex",
      workflow: "codex_agent_task",
      workspace,
    })

    expect(body).toMatchObject({
      openagents: {
        coding: {
          objectiveSummary: "Implement the public-safe issue slice and run the named verification command.",
          targetPylonRef: "pylon.owner.codex",
          workspace: {
            kind: "git_checkout",
            repository: {
              commitSha: "7ab7cb401803f6e04a6c93b7aa9102405de66419",
              fullName: "OpenAgentsInc/openagents",
              provider: "github",
              visibility: "public",
            },
            verificationCommand: {
              args: [
                "bun",
                "run",
                "--cwd",
                "apps/openagents.com/workers/api",
                "test",
                "--",
                "src/inference/coding-workflow-delegation.test.ts",
              ],
            },
          },
        },
      },
      targetPylonRef: "pylon.owner.codex",
      workflowClass: "codex_agent_task",
    })
  })

  test("accepts bounded hydralisk adapter verifier filenames", () => {
    const workspace = buildPylonKhalaGitCheckoutWorkspace({
      commit: "7ab7cb401803f6e04a6c93b7aa9102405de66419",
      repository: "OpenAgentsInc/openagents",
      verificationCommand: "bun --cwd apps/openagents.com/workers/api test src/inference/glm-pool-heartbeat.test.ts src/inference/hydralisk-adapter.test.ts src/inference/model-router.test.ts",
    })

    expect(workspace.verificationCommand.args).toEqual([
      "bun",
      "--cwd",
      "apps/openagents.com/workers/api",
      "test",
      "src/inference/glm-pool-heartbeat.test.ts",
      "src/inference/hydralisk-adapter.test.ts",
      "src/inference/model-router.test.ts",
    ])
    expect(workspace.verificationCommand.commandRef).toStartWith("command.public.pylon_khala.")
  })

  test("accepts public relative verifier paths with invoice-like filename letters", () => {
    const workspace = buildPylonKhalaGitCheckoutWorkspace({
      commit: "7ab7cb401803f6e04a6c93b7aa9102405de66419",
      repository: "OpenAgentsInc/openagents",
      verificationCommand: "bun test apps/pylon/tests/lnbc-routing.test.ts packages/nip90/src/lntb-normalize.test.ts",
    })

    expect(workspace.verificationCommand.args).toEqual([
      "bun",
      "test",
      "apps/pylon/tests/lnbc-routing.test.ts",
      "packages/nip90/src/lntb-normalize.test.ts",
    ])
  })

  test("rejects credential-shaped verifier values", () => {
    expect(() =>
      buildPylonKhalaGitCheckoutWorkspace({
        commit: "7ab7cb401803f6e04a6c93b7aa9102405de66419",
        repository: "OpenAgentsInc/openagents",
        verificationCommand: "bun test sk-1234567890abcdef1234567890abcdef",
      }),
    ).toThrow(/private, payment/)
  })

  test("still rejects invoice-shaped verifier values", () => {
    expect(() =>
      buildPylonKhalaGitCheckoutWorkspace({
        commit: "7ab7cb401803f6e04a6c93b7aa9102405de66419",
        repository: "OpenAgentsInc/openagents",
        verificationCommand: "bun test lnbc1p5exampleinvoicevalue000000",
      }),
    ).toThrow(/private, payment/)
  })

  test("requires an explicit workspace verification command", () => {
    expect(() =>
      buildPylonKhalaGitCheckoutWorkspace({
        commit: "7ab7cb401803f6e04a6c93b7aa9102405de66419",
        repository: "OpenAgentsInc/openagents",
      }),
    ).toThrow(/--verify/)
  })

  test("rejects unsafe prompt material before calling the gateway", () => {
    expect(() =>
      buildPylonKhalaChatRequestBody({
        prompt: "use bearer token secret",
      }),
    ).toThrow(/private, payment/)
  })
})

describe("pylon khala requester API", () => {
  test("CLI request auto-runs the returned no-spend assignment unless disabled", async () => {
    await withTempHome(async (home) => {
      await markAssignmentReady(home)
      const fake = khalaAutoRunServer()

      const proc = await runPylonCli(
        [
          "khala",
          "request",
          "--workflow",
          "codex_agent_task",
          "--fixture",
          "--prompt",
          "Run the public-safe fixture through my linked local Pylon.",
          "--base-url",
          fake.baseUrl,
          "--json",
        ],
        {
          OPENAGENTS_AGENT_TOKEN: "oa_agent_test",
          PYLON_HOME: home,
        },
      )

      expect(proc.timedOut).toBe(false)
      expect(proc.exitCode).toBe(0)
      const output = JSON.parse(proc.stdout)
      expect(output).toMatchObject({
        assignmentRef: fake.assignmentRef,
        autoRun: {
          assignmentRef: fake.assignmentRef,
          attempted: true,
          ok: true,
          schema: "openagents.pylon.khala_request_auto_run.v0.1",
        },
        assignmentRun: {
          ok: true,
          closeout: {
            assignmentRef: fake.assignmentRef,
            paymentMode: "no-spend",
            settlementState: "not_applicable",
          },
        },
      })
      const paths = fake.requests.map((request) => request.path)
      expect(paths).toContain("/v1/chat/completions")
      expect(paths.some((path) => path.endsWith("/assignments"))).toBe(true)
      expect(paths.some((path) => path.endsWith("/accept"))).toBe(true)
      expect(paths.some((path) => path.endsWith("/closeout"))).toBe(true)
      expect(proc.stderr)
        .toContain('"event":"assignment_run.completed"')
    })
  })

  test("CLI request can leave the lease un-run for diagnostics with --no-run", async () => {
    await withTempHome(async (home) => {
      await markAssignmentReady(home)
      const fake = khalaAutoRunServer()

      const proc = await runPylonCli(
        [
          "khala",
          "request",
          "--workflow",
          "codex_agent_task",
          "--fixture",
          "--prompt",
          "Create the lease but do not run it yet.",
          "--base-url",
          fake.baseUrl,
          "--no-run",
          "--json",
        ],
        {
          OPENAGENTS_AGENT_TOKEN: "oa_agent_test",
          PYLON_HOME: home,
        },
      )

      expect(proc.timedOut).toBe(false)
      expect(proc.exitCode).toBe(0)
      const output = JSON.parse(proc.stdout)
      expect(output).toMatchObject({
        assignmentRef: fake.assignmentRef,
        assignmentRun: null,
        autoRun: {
          attempted: false,
          reason: "disabled_by_no_run",
          schema: "openagents.pylon.khala_request_auto_run.v0.1",
        },
      })
      expect(fake.requests.map((request) => request.path)).toEqual([
        "/v1/chat/completions",
      ])
    })
  })

  test("request posts to /v1/chat/completions with bearer auth and projects the durable handle", async () => {
    const calls: Array<{ init: RequestInit; url: string }> = []
    const fetcher = async (url: URL | RequestInfo, init?: RequestInit) => {
      calls.push({ init: init ?? {}, url: String(url) })
      return new Response(sse("chatcmpl_123", "delegated"), {
        headers: {
          "openagents-coding-assignment-ref": "assignment.public.one",
          "openagents-durable-stream-url": "/v1/chat/completions/durable/chatcmpl_123",
        },
      })
    }

    const result = await issuePylonKhalaRequest(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.test",
        fetch: fetcher,
      },
      {
        prompt: "Run the fixture task",
        targetPylonRef: "pylon.owner.codex",
        workflow: "codex_agent_task",
      },
    )

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe("https://openagents.test/v1/chat/completions")
    expect(calls[0]?.init.method).toBe("POST")
    expect((calls[0]?.init.headers as Record<string, string>).Authorization).toBe("Bearer oa_agent_test")
    expect(JSON.parse(String(calls[0]?.init.body))).toMatchObject({
      model: "openagents/khala",
      openagents: {
        coding: { targetPylonRef: "pylon.owner.codex" },
        workflowClass: "codex_agent_task",
      },
      stream: true,
      targetPylonRef: "pylon.owner.codex",
      workflowClass: "codex_agent_task",
    })
    expect(result).toMatchObject({
      assignmentRef: "assignment.public.one",
      durableRequestId: "chatcmpl_123",
      durableStreamUrl: "/v1/chat/completions/durable/chatcmpl_123",
      model: "openagents/khala",
      ok: true,
      schema: "openagents.pylon.khala_request.v1",
      streamClosed: true,
      text: "delegated",
      workflow: "codex_agent_task",
    })
    expect(Number(result.nextOffset)).toBeGreaterThan(0)
  })

  test("request requires the user agent token", async () => {
    const originalToken = process.env.OPENAGENTS_AGENT_TOKEN
    delete process.env.OPENAGENTS_AGENT_TOKEN
    try {
      await expect(
        issuePylonKhalaRequest(
          {
            baseUrl: "https://openagents.test",
            fetch: async () => new Response("{}"),
          },
          { prompt: "Run the fixture task" },
        ),
      ).rejects.toThrow("OPENAGENTS_AGENT_TOKEN")
    } finally {
      if (originalToken === undefined) {
        delete process.env.OPENAGENTS_AGENT_TOKEN
      } else {
        process.env.OPENAGENTS_AGENT_TOKEN = originalToken
      }
    }
  })

  test("resume reads the durable suffix without a metering POST", async () => {
    const calls: Array<{ init: RequestInit; url: string }> = []
    const fetcher = async (url: URL | RequestInfo, init?: RequestInit) => {
      calls.push({ init: init ?? {}, url: String(url) })
      return new Response(sse("chatcmpl_123", "suffix"), {
        headers: {
          "stream-closed": "true",
          "stream-next-offset": "128",
          "stream-up-to-date": "true",
        },
      })
    }

    const result = await resumePylonKhalaRequest(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.test",
        fetch: fetcher,
      },
      {
        durableRequestId: "chatcmpl_123",
        offset: 64,
      },
    )

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe("https://openagents.test/v1/chat/completions/durable/chatcmpl_123?offset=64")
    expect(calls[0]?.init.method).toBe("GET")
    expect(result.nextOffset).toBe("128")
    expect(result.streamClosed).toBe(true)
    expect(result.streamUpToDate).toBe(true)
    expect(result.text).toBe("suffix")
  })

  test("status projects a durable stream state from the read headers", async () => {
    const result = await readPylonKhalaStatus(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.test",
        fetch: async () =>
          new Response("", {
            headers: {
              "stream-next-offset": "0",
              "stream-up-to-date": "true",
            },
          }),
      },
      "chatcmpl_123",
    )

    expect(result).toMatchObject({
      durableRequestId: "chatcmpl_123",
      nextOffset: "0",
      schema: "openagents.pylon.khala_status.v1",
      state: "up_to_date",
    })
  })

  test("assignment status reads owner-scoped trace status without raw payloads", async () => {
    const calls: Array<{ init: RequestInit; url: string }> = []
    const result = await readPylonKhalaAssignmentTraceStatus(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.test",
        fetch: async (url: URL | RequestInfo, init?: RequestInit) => {
          calls.push({ init: init ?? {}, url: String(url) })
          return new Response(
            JSON.stringify({
              schemaVersion: "openagents.pylon.codex_assignment_trace_status.v1",
              assignmentRef: "assignment-pylon-codex-1",
              pylonRef: "pylon-local-codex-1",
              owner: {
                agentUserRef: "agent:agent-user-1",
                openauthUserRef: "user-openauth-1",
              },
              lifecycle: {
                state: "running",
                createdAt: "2026-06-26T12:00:00.000Z",
                updatedAt: "2026-06-26T12:00:05.000Z",
                acceptedWorkRefs: [],
                artifactRefs: [],
                closeoutRefs: [],
                proofRefs: [],
                rejectionRefs: [],
              },
              events: {
                count: 2,
                progressCount: 1,
                latestEventKind: "assignment_progress",
                latestStatus: "runtime_active",
                latestObservedAt: "2026-06-26T12:00:02.000Z",
              },
              tokenUsage: {
                rowCount: 0,
                provider: "pylon-codex-own-capacity",
                model: "openagents/pylon-codex",
                usageTruth: "exact",
                demandKind: "own_capacity",
                demandSource: "khala_coding_delegation",
                inputTokens: 0,
                outputTokens: 0,
                reasoningTokens: 0,
                cacheReadTokens: 0,
                totalTokens: 0,
                status: "pending",
              },
              traces: {
                count: 1,
                visibility: "owner_only",
                schemaVersion: "ATIF-v1.7",
                latestTraceUuid: "trace-chunk-1",
                finalTraceUuid: null,
                refs: ["trace-chunk-1"],
              },
              rawEventChunks: {
                count: 2,
                eventCount: 5,
                byteLength: 120,
                latestChunkRef: "raw_chunk.pylon_codex.002",
                latestObservedAt: "2026-06-26T12:00:02.000Z",
                visibility: "owner_only",
              },
              rawEvents: {
                count: 0,
                eventCount: 0,
                byteLength: 0,
                latestRawEventRef: null,
                latestObservedAt: null,
                visibility: "owner_only",
                refs: [],
              },
              progress: {
                state: "streaming_chunks",
                closeoutReady: false,
                hasLiveChunks: true,
                hasFinalTrace: false,
                hasTokenUsage: false,
              },
              generatedAt: "2026-06-26T12:00:06.000Z",
            }),
            { headers: { "content-type": "application/json" } },
          )
        },
      },
      "assignment-pylon-codex-1",
    )

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe("https://openagents.test/api/pylon/codex/trace-status?assignmentRef=assignment-pylon-codex-1")
    expect(calls[0]?.init.method).toBe("GET")
    expect((calls[0]?.init.headers as Record<string, string>).Authorization).toBe("Bearer oa_agent_test")
    expect(result).toMatchObject({
      assignmentRef: "assignment-pylon-codex-1",
      ok: true,
      progress: {
        hasLiveChunks: true,
        state: "streaming_chunks",
      },
      rawEventChunks: {
        eventCount: 5,
        visibility: "owner_only",
      },
      schemaVersion: "openagents.pylon.codex_assignment_trace_status.v1",
      tokenUsage: {
        status: "pending",
      },
      traces: {
        visibility: "owner_only",
      },
    })
    expect(JSON.stringify(result)).not.toMatch(
      /rawEventsJson|trajectory_json|safe_metadata_json|r2_key|prompt|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
  })

  test("proof reads owner-scoped public-safe assignment totals", async () => {
    const calls: Array<{ init: RequestInit; url: string }> = []
    const result = await readPylonKhalaProof(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.test",
        fetch: async (url: URL | RequestInfo, init?: RequestInit) => {
          calls.push({ init: init ?? {}, url: String(url) })
          return new Response(
            JSON.stringify({
              schemaVersion: "openagents.pylon.codex_assignment_proof.v1",
              assignmentRef: "assignment-pylon-codex-1",
              pylonRef: "pylon-local-codex-1",
              owner: {
                agentUserRef: "agent:agent-user-1",
                openauthUserRef: "user-openauth-1",
              },
              tokenUsage: {
                rowCount: 2,
                provider: "pylon-codex-own-capacity",
                model: "openagents/pylon-codex",
                usageTruth: "exact",
                demandKind: "own_capacity",
                demandSource: "khala_coding_delegation",
                inputTokens: 100,
                outputTokens: 40,
                reasoningTokens: 10,
                cacheReadTokens: 5,
                totalTokens: 140,
              },
              traces: {
                count: 2,
                visibility: "owner_only",
                schemaVersion: "ATIF-v1.7",
                refs: ["trace-1", "trace-2"],
              },
              rawEvents: {
                count: 2,
                eventCount: 8,
                byteLength: 4096,
                visibility: "owner_only",
                refs: ["raw.pylon_codex.aaa", "raw.pylon_codex.bbb"],
              },
              generatedAt: "2026-06-26T12:00:00.000Z",
            }),
            { headers: { "content-type": "application/json" } },
          )
        },
      },
      "assignment-pylon-codex-1",
    )

    expect(calls).toHaveLength(1)
    expect(calls[0]?.url).toBe("https://openagents.test/api/pylon/codex/proof?assignmentRef=assignment-pylon-codex-1")
    expect(calls[0]?.init.method).toBe("GET")
    expect((calls[0]?.init.headers as Record<string, string>).Authorization).toBe("Bearer oa_agent_test")
    expect(result).toMatchObject({
      assignmentRef: "assignment-pylon-codex-1",
      ok: true,
      proofChecklist: {
        blockerRefs: [],
        ok: true,
        schema: "openagents.pylon.khala_proof_checklist.v0.1",
      },
      rawEvents: {
        eventCount: 8,
        visibility: "owner_only",
      },
      tokenUsage: {
        provider: "pylon-codex-own-capacity",
        totalTokens: 140,
        usageTruth: "exact",
      },
      traces: {
        count: 2,
        visibility: "owner_only",
      },
    })
    expect(JSON.stringify(result)).not.toMatch(
      /rawEventsJson|trajectory_json|safe_metadata_json|r2_key|prompt|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
  })

  test("proof checklist fails closed when owner trace or exact usage evidence is missing", () => {
    const checklist = evaluatePylonKhalaProofChecklist({
      assignmentRef: "assignment-pylon-codex-1",
      generatedAt: "not-a-date",
      owner: {
        agentUserRef: "agent:agent-user-1",
        openauthUserRef: "user-openauth-1",
      },
      pylonRef: "pylon-local-codex-1",
      rawEvents: {
        byteLength: 0,
        count: 0,
        eventCount: 0,
        refs: [],
        visibility: "owner_only",
      },
      schemaVersion: "openagents.pylon.codex_assignment_proof.v1",
      tokenUsage: {
        cacheReadTokens: 0,
        demandKind: "own_capacity",
        demandSource: "khala_coding_delegation",
        inputTokens: 0,
        model: "openagents/pylon-codex",
        outputTokens: 0,
        provider: "pylon-codex-own-capacity",
        reasoningTokens: 0,
        rowCount: 0,
        totalTokens: 0,
        usageTruth: "exact",
      },
      traces: {
        count: 0,
        refs: [],
        schemaVersion: "ATIF-v1.7",
        visibility: "owner_only",
      },
    })

    expect(checklist.ok).toBe(false)
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_proof.token_usage.rows_and_tokens_present",
    )
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_proof.traces.owner_only_present",
    )
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_proof.raw_events.owner_only_present",
    )
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_proof.generated_at.iso_timestamp",
    )
  })

  test("closeout checklist composes trace status and proof gates", () => {
    const proofPayload = completeProof()
    const proofChecklist = evaluatePylonKhalaProofChecklist(proofPayload as ProofPayload)
    const checklist = evaluatePylonKhalaCloseoutChecklist(
      completeTraceStatus() as CloseoutTraceStatus,
      { ...proofPayload, ok: true, proofChecklist } as CloseoutProofResult,
    )

    expect(checklist).toMatchObject({
      blockerRefs: [],
      caveatRefs: [
        "caveat.khala_closeout.public_token_counter_is_supporting_not_assignment_proof",
      ],
      ok: true,
      schema: "openagents.pylon.khala_closeout_checklist.v0.1",
    })
  })

  test("closeout checklist fails closed before final trace and recorded token rows", () => {
    const proofPayload = completeProof({
      closeoutPolicy: {
        paymentMode: "unknown",
        payoutClaimAllowed: null,
        settlementState: "unknown",
        source: "unavailable",
      },
      tokenUsage: {
        ...completeProof().tokenUsage,
        rowCount: 0,
        totalTokens: 0,
      },
    })
    const proofChecklist = evaluatePylonKhalaProofChecklist(proofPayload as ProofPayload)
    const checklist = evaluatePylonKhalaCloseoutChecklist(
      completeTraceStatus({
        closeoutPolicy: {
          paymentMode: "unknown",
          payoutClaimAllowed: null,
          settlementState: "unknown",
          source: "unavailable",
        },
        lifecycle: {
          ...completeTraceStatus().lifecycle,
          closeoutRefs: [],
        },
        progress: {
          state: "streaming_chunks",
          closeoutReady: false,
          hasLiveChunks: true,
          hasFinalTrace: false,
          hasTokenUsage: false,
        },
        rawEvents: {
          ...completeTraceStatus().rawEvents,
          count: 0,
          eventCount: 0,
          byteLength: 0,
          refs: [],
        },
        tokenUsage: {
          ...completeTraceStatus().tokenUsage,
          rowCount: 0,
          status: "pending",
          totalTokens: 0,
        },
        traces: {
          ...completeTraceStatus().traces,
          finalTraceUuid: null,
        },
      }) as CloseoutTraceStatus,
      { ...proofPayload, ok: true, proofChecklist } as CloseoutProofResult,
    )

    expect(checklist.ok).toBe(false)
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_closeout.lifecycle.closed_out",
    )
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_closeout.trace_status.final_owner_trace_present",
    )
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_closeout.trace_status.raw_events_owner_only_present",
    )
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_closeout.trace_status.token_usage_recorded",
    )
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_closeout.proof_checklist.ok",
    )
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_closeout.no_spend_payout_false",
    )
    expect(checklist.blockerRefs).toContain(
      "blocker.khala_proof.token_usage.rows_and_tokens_present",
    )
  })

  test("closeout checklist fails closed instead of throwing when remote closeout policy is absent", () => {
    const { closeoutPolicy: _proofCloseoutPolicy, ...proofPayload } =
      completeProof()
    const {
      closeoutPolicy: _statusCloseoutPolicy,
      ...traceStatusPayload
    } = completeTraceStatus()
    const proofChecklist = evaluatePylonKhalaProofChecklist(
      proofPayload as ProofPayload,
    )
    const checklist = evaluatePylonKhalaCloseoutChecklist(
      traceStatusPayload as CloseoutTraceStatus,
      { ...proofPayload, ok: true, proofChecklist } as CloseoutProofResult,
    )

    expect(checklist.ok).toBe(false)
    expect(checklist.blockerRefs).toEqual([
      "blocker.khala_closeout.no_spend_payout_false",
    ])
  })

  test("closeout reads owner-scoped trace status and proof as one JSON projection", async () => {
    const calls: Array<{ init: RequestInit; url: string }> = []
    const result = await readPylonKhalaCloseout(
      {
        agentToken: "oa_agent_test",
        baseUrl: "https://openagents.test",
        fetch: async (url: URL | RequestInfo, init?: RequestInit) => {
          calls.push({ init: init ?? {}, url: String(url) })
          const pathname = new URL(String(url)).pathname
          return new Response(
            JSON.stringify(
              pathname.endsWith("/trace-status")
                ? completeTraceStatus()
                : completeProof(),
            ),
            { headers: { "content-type": "application/json" } },
          )
        },
      },
      "assignment-pylon-codex-1",
    )

    expect(calls.map((call) => new URL(call.url).pathname).sort()).toEqual([
      "/api/pylon/codex/proof",
      "/api/pylon/codex/trace-status",
    ])
    expect(calls.every((call) => call.init.method === "GET")).toBe(true)
    expect(
      calls.every(
        (call) =>
          (call.init.headers as Record<string, string>).Authorization ===
          "Bearer oa_agent_test",
      ),
    ).toBe(true)
    expect(result).toMatchObject({
      assignmentRef: "assignment-pylon-codex-1",
      closeoutChecklist: {
        blockerRefs: [],
        ok: true,
      },
      ok: true,
      proof: {
        proofChecklist: {
          ok: true,
        },
      },
      schema: "openagents.pylon.khala_closeout.v0.1",
      status: {
        progress: {
          closeoutReady: true,
          state: "closed_out",
        },
      },
    })
    expect(JSON.stringify(result)).not.toMatch(
      /rawEventsJson|trajectory_json|safe_metadata_json|r2_key|prompt|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
  })

  test("parses durable request ids from relative and absolute resume URLs", () => {
    expect(durableRequestIdFromUrl("/v1/chat/completions/durable/chatcmpl_123")).toBe("chatcmpl_123")
    expect(durableRequestIdFromUrl("https://openagents.test/v1/chat/completions/durable/chatcmpl_456")).toBe("chatcmpl_456")
    expect(durableRequestIdFromUrl("/v1/chat/completions/not-durable/chatcmpl_123")).toBeNull()
  })

  test("local CLI issues a Khala request against a fixture gateway", async () => {
    const requests: Array<{ body: Record<string, unknown>; headers: Headers; path: string }> = []
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url)
        const body = JSON.parse(await request.text()) as Record<string, unknown>
        requests.push({ body, headers: request.headers, path: url.pathname })
        return new Response(sse("chatcmpl_cli", "cli delegated"), {
          headers: {
            "openagents-durable-stream-url": "/v1/chat/completions/durable/chatcmpl_cli",
          },
        })
      },
    })
    servers.push(server)

    const result = await runPylonCli(
      [
        "khala",
        "request",
        "--prompt",
        "Run the CLI fixture task",
        "--workflow",
        "codex_agent_task",
        "--pylon-ref",
        "pylon.owner.codex",
        "--commit",
        "7ab7cb401803f6e04a6c93b7aa9102405de66419",
        "--repo",
        "OpenAgentsInc/openagents",
        "--verify",
        "bun test",
        "--agent-token",
        "oa_agent_cli_test",
        "--json",
      ],
      {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_cli_test",
        PYLON_OPENAGENTS_BASE_URL: `http://127.0.0.1:${server.port}`,
      },
    )

    expect(result.timedOut).toBe(false)
    expect(result.exitCode).toBe(0)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.path).toBe("/v1/chat/completions")
    expect(requests[0]?.headers.get("authorization")).toBe("Bearer oa_agent_cli_test")
    expect(requests[0]?.body).toMatchObject({
      model: "openagents/khala",
      openagents: {
        coding: {
          objectiveSummary: "Run the CLI fixture task",
          targetPylonRef: "pylon.owner.codex",
          workspace: {
            kind: "git_checkout",
            repository: {
              commitSha: "7ab7cb401803f6e04a6c93b7aa9102405de66419",
              fullName: "OpenAgentsInc/openagents",
            },
          },
        },
        workflowClass: "codex_agent_task",
      },
      stream: true,
      targetPylonRef: "pylon.owner.codex",
      workflowClass: "codex_agent_task",
    })
    const body = JSON.parse(result.stdout) as Record<string, unknown>
    expect(body).toMatchObject({
      durableRequestId: "chatcmpl_cli",
      ok: true,
      schema: "openagents.pylon.khala_request.v1",
      text: "cli delegated",
    })
  }, 15_000)

  test("local CLI reads Khala assignment proof as JSON", async () => {
    const requests: Array<{ headers: Headers; path: string; search: string }> = []
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url)
        requests.push({ headers: request.headers, path: url.pathname, search: url.search })
        return new Response(
          JSON.stringify({
            schemaVersion: "openagents.pylon.codex_assignment_proof.v1",
            assignmentRef: url.searchParams.get("assignmentRef"),
            pylonRef: "pylon-local-codex-1",
            owner: {
              agentUserRef: "agent:agent-user-1",
              openauthUserRef: "user-openauth-1",
            },
            tokenUsage: {
              rowCount: 1,
              provider: "pylon-codex-own-capacity",
              model: "openagents/pylon-codex",
              usageTruth: "exact",
              demandKind: "own_capacity",
              demandSource: "khala_coding_delegation",
              inputTokens: 10,
              outputTokens: 5,
              reasoningTokens: 2,
              cacheReadTokens: 1,
              totalTokens: 15,
            },
            traces: {
              count: 1,
              visibility: "owner_only",
              schemaVersion: "ATIF-v1.7",
              refs: ["trace-1"],
            },
            rawEvents: {
              count: 1,
              eventCount: 3,
              byteLength: 1024,
              visibility: "owner_only",
              refs: ["raw.pylon_codex.aaa"],
            },
            generatedAt: "2026-06-26T12:00:00.000Z",
          }),
          { headers: { "content-type": "application/json" } },
        )
      },
    })
    servers.push(server)

    const result = await runPylonCli(
      ["khala", "proof", "assignment-pylon-codex-1", "--agent-token", "oa_agent_cli_test", "--json"],
      {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_cli_test",
        PYLON_OPENAGENTS_BASE_URL: `http://127.0.0.1:${server.port}`,
      },
    )

    expect(result.timedOut).toBe(false)
    expect(result.exitCode).toBe(0)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.path).toBe("/api/pylon/codex/proof")
    expect(requests[0]?.search).toBe("?assignmentRef=assignment-pylon-codex-1")
    expect(requests[0]?.headers.get("authorization")).toBe("Bearer oa_agent_cli_test")
    const body = JSON.parse(result.stdout) as Record<string, unknown>
    expect(body).toMatchObject({
      assignmentRef: "assignment-pylon-codex-1",
      ok: true,
      proofChecklist: {
        blockerRefs: [],
        ok: true,
      },
      tokenUsage: {
        totalTokens: 15,
        usageTruth: "exact",
      },
      traces: { visibility: "owner_only" },
      rawEvents: { eventCount: 3, visibility: "owner_only" },
    })
    expect(JSON.stringify(body)).not.toMatch(
      /rawEventsJson|trajectory_json|safe_metadata_json|r2_key|prompt|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
  }, 15_000)

  test("local CLI reads Khala closeout checklist as JSON", async () => {
    const requests: Array<{ headers: Headers; path: string; search: string }> = []
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url)
        requests.push({ headers: request.headers, path: url.pathname, search: url.search })
        return new Response(
          JSON.stringify(
            url.pathname.endsWith("/trace-status")
              ? completeTraceStatus()
              : completeProof(),
          ),
          { headers: { "content-type": "application/json" } },
        )
      },
    })
    servers.push(server)

    const result = await runPylonCli(
      ["khala", "closeout", "assignment-pylon-codex-1", "--agent-token", "oa_agent_cli_test", "--json"],
      {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_cli_test",
        PYLON_OPENAGENTS_BASE_URL: `http://127.0.0.1:${server.port}`,
      },
    )

    expect(result.timedOut).toBe(false)
    expect(result.exitCode).toBe(0)
    expect(requests.map((request) => request.path).sort()).toEqual([
      "/api/pylon/codex/proof",
      "/api/pylon/codex/trace-status",
    ])
    expect(requests.every((request) => request.search === "?assignmentRef=assignment-pylon-codex-1")).toBe(true)
    expect(requests.every((request) => request.headers.get("authorization") === "Bearer oa_agent_cli_test")).toBe(true)
    const body = JSON.parse(result.stdout) as Record<string, unknown>
    expect(body).toMatchObject({
      assignmentRef: "assignment-pylon-codex-1",
      closeoutChecklist: {
        blockerRefs: [],
        ok: true,
        schema: "openagents.pylon.khala_closeout_checklist.v0.1",
      },
      ok: true,
      proof: {
        proofChecklist: { ok: true },
      },
      schema: "openagents.pylon.khala_closeout.v0.1",
      status: {
        progress: {
          closeoutReady: true,
          state: "closed_out",
        },
      },
    })
    expect(JSON.stringify(body)).not.toMatch(
      /rawEventsJson|trajectory_json|safe_metadata_json|r2_key|prompt|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
  }, 15_000)

  test("local CLI reads Khala assignment trace status as JSON", async () => {
    const requests: Array<{ headers: Headers; path: string; search: string }> = []
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url)
        requests.push({ headers: request.headers, path: url.pathname, search: url.search })
        return new Response(
          JSON.stringify({
            schemaVersion: "openagents.pylon.codex_assignment_trace_status.v1",
            assignmentRef: url.searchParams.get("assignmentRef"),
            pylonRef: "pylon-local-codex-1",
            owner: {
              agentUserRef: "agent:agent-user-1",
              openauthUserRef: "user-openauth-1",
            },
            lifecycle: {
              state: "running",
              createdAt: "2026-06-26T12:00:00.000Z",
              updatedAt: "2026-06-26T12:00:05.000Z",
              acceptedWorkRefs: [],
              artifactRefs: [],
              closeoutRefs: [],
              proofRefs: [],
              rejectionRefs: [],
            },
            events: {
              count: 2,
              progressCount: 1,
              latestEventKind: "assignment_progress",
              latestStatus: "runtime_active",
              latestObservedAt: "2026-06-26T12:00:02.000Z",
            },
            tokenUsage: {
              rowCount: 0,
              provider: "pylon-codex-own-capacity",
              model: "openagents/pylon-codex",
              usageTruth: "exact",
              demandKind: "own_capacity",
              demandSource: "khala_coding_delegation",
              inputTokens: 0,
              outputTokens: 0,
              reasoningTokens: 0,
              cacheReadTokens: 0,
              totalTokens: 0,
              status: "pending",
            },
            traces: {
              count: 1,
              visibility: "owner_only",
              schemaVersion: "ATIF-v1.7",
              latestTraceUuid: "trace-chunk-1",
              finalTraceUuid: null,
              refs: ["trace-chunk-1"],
            },
            rawEventChunks: {
              count: 2,
              eventCount: 5,
              byteLength: 120,
              latestChunkRef: "raw_chunk.pylon_codex.002",
              latestObservedAt: "2026-06-26T12:00:02.000Z",
              visibility: "owner_only",
            },
            rawEvents: {
              count: 0,
              eventCount: 0,
              byteLength: 0,
              latestRawEventRef: null,
              latestObservedAt: null,
              visibility: "owner_only",
              refs: [],
            },
            progress: {
              state: "streaming_chunks",
              closeoutReady: false,
              hasLiveChunks: true,
              hasFinalTrace: false,
              hasTokenUsage: false,
            },
            generatedAt: "2026-06-26T12:00:06.000Z",
          }),
          { headers: { "content-type": "application/json" } },
        )
      },
    })
    servers.push(server)

    const result = await runPylonCli(
      ["khala", "status", "--assignment-ref", "assignment-pylon-codex-1", "--agent-token", "oa_agent_cli_test", "--json"],
      {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_cli_test",
        PYLON_OPENAGENTS_BASE_URL: `http://127.0.0.1:${server.port}`,
      },
    )

    expect(result.timedOut).toBe(false)
    expect(result.exitCode).toBe(0)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.path).toBe("/api/pylon/codex/trace-status")
    expect(requests[0]?.search).toBe("?assignmentRef=assignment-pylon-codex-1")
    expect(requests[0]?.headers.get("authorization")).toBe("Bearer oa_agent_cli_test")
    const body = JSON.parse(result.stdout) as Record<string, unknown>
    expect(body).toMatchObject({
      assignmentRef: "assignment-pylon-codex-1",
      ok: true,
      progress: {
        hasLiveChunks: true,
        state: "streaming_chunks",
      },
      rawEventChunks: {
        visibility: "owner_only",
      },
    })
    expect(JSON.stringify(body)).not.toMatch(
      /rawEventsJson|trajectory_json|safe_metadata_json|r2_key|prompt|shell|\/Users|secret|access[_-]?token|bearer/i,
    )
  }, 15_000)

  test("local CLI refuses codex_agent_task without fixture intent or complete workspace pins before gateway calls", async () => {
    const requests: string[] = []
    const server = Bun.serve({
      port: 0,
      fetch(request) {
        requests.push(request.url)
        return new Response("unexpected")
      },
    })
    servers.push(server)

    const result = await runPylonCli(
      [
        "khala",
        "request",
        "--prompt",
        "Implement public issue #6349",
        "--workflow",
        "codex_agent_task",
        "--pylon-ref",
        "pylon.owner.codex",
        "--repo",
        "OpenAgentsInc/openagents",
        "--json",
      ],
      {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_cli_test",
        PYLON_OPENAGENTS_BASE_URL: `http://127.0.0.1:${server.port}`,
      },
    )

    expect(result.timedOut).toBe(false)
    expect(result.exitCode).not.toBe(0)
    expect(requests).toHaveLength(0)
    const output = `${result.stdout}\n${result.stderr}`
    expect(output).toContain("requires explicit fixture intent")
    expect(output).toContain("--commit")
    expect(output).toContain("--verify")
  }, 15_000)

  test("local CLI preserves an explicit codex fixture smoke request without workspace pins", async () => {
    const requests: Array<{ body: Record<string, unknown>; path: string }> = []
    const server = Bun.serve({
      port: 0,
      async fetch(request) {
        const url = new URL(request.url)
        const body = JSON.parse(await request.text()) as Record<string, unknown>
        requests.push({ body, path: url.pathname })
        return new Response(sse("chatcmpl_fixture", "fixture delegated"), {
          headers: {
            "openagents-durable-stream-url": "/v1/chat/completions/durable/chatcmpl_fixture",
          },
        })
      },
    })
    servers.push(server)

    const result = await runPylonCli(
      [
        "khala",
        "request",
        "--prompt",
        "Run the CLI fixture task",
        "--workflow",
        "codex_agent_task",
        "--pylon-ref",
        "pylon.owner.codex",
        "--fixture",
        "--json",
      ],
      {
        OPENAGENTS_AGENT_TOKEN: "oa_agent_cli_test",
        PYLON_OPENAGENTS_BASE_URL: `http://127.0.0.1:${server.port}`,
      },
    )

    expect(result.timedOut).toBe(false)
    expect(result.exitCode).toBe(0)
    expect(requests).toHaveLength(1)
    expect(requests[0]?.path).toBe("/v1/chat/completions")
    expect(requests[0]?.body).toMatchObject({
      openagents: {
        coding: { targetPylonRef: "pylon.owner.codex" },
        workflowClass: "codex_agent_task",
      },
      workflowClass: "codex_agent_task",
    })
    expect(JSON.stringify(requests[0]?.body)).not.toContain("workspace")
    const body = JSON.parse(result.stdout) as Record<string, unknown>
    expect(body).toMatchObject({
      durableRequestId: "chatcmpl_fixture",
      ok: true,
      text: "fixture delegated",
    })
  }, 15_000)
})
