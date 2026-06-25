import { afterEach, describe, expect, test } from "bun:test"
import { join } from "node:path"

import {
  buildPylonKhalaGitCheckoutWorkspace,
  buildPylonKhalaChatRequestBody,
  durableRequestIdFromUrl,
  issuePylonKhalaRequest,
  readPylonKhalaStatus,
  resumePylonKhalaRequest,
} from "../src/khala-requester"

const sse = (id: string, content: string) =>
  `data: ${JSON.stringify({
    choices: [{ delta: { content }, index: 0 }],
    id,
    object: "chat.completion.chunk",
  })}\n\ndata: [DONE]\n\n`

const servers: ReturnType<typeof Bun.serve>[] = []
const INDEX = join(import.meta.dir, "..", "src", "index.ts")
const CWD = join(import.meta.dir, "..")

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true)
})

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

  test("rejects unsafe prompt material before calling the gateway", () => {
    expect(() =>
      buildPylonKhalaChatRequestBody({
        prompt: "use bearer token secret",
      }),
    ).toThrow(/private, payment/)
  })
})

describe("pylon khala requester API", () => {
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
})
