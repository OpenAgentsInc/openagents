import { describe, expect, test } from "bun:test"
import { mkdtempSync } from "node:fs"
import { mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { createControlSessionActions } from "../src/node/control-sessions"
import {
  makeCloudControlSessionExecutor,
  type CloudSessionGrantBinding,
} from "../src/openagents-cloud-provider"
import {
  resolveCloudControlConfig,
  type CloudWorkroomEvent,
} from "../src/cloud-control-client"

// #4997: faked cloud control plane over `fetch`. It records the calls Pylon
// makes (grant resolve, placement, event polls) and serves a scripted run that
// progresses queued -> started -> log -> artifact -> receipt -> completed.
function makeFakeCloudControlPlane(options?: {
  failGrant?: boolean
  scriptedEvents?: CloudWorkroomEvent[]
}) {
  const calls: { url: string; method: string; body?: unknown }[] = []
  const externalRunId = "shc-codex:oa-gce-ephemeral-test:test_run"

  const queue: CloudWorkroomEvent[][] = options?.scriptedEvents
    ? [options.scriptedEvents]
    : [
        [{ kind: "log", summary: "cloning repository" }],
        [
          {
            kind: "artifact",
            summary: "produced diff artifact",
            artifactRefs: ["artifact.cloud.diff.0"],
          },
        ],
        [
          {
            kind: "receipt",
            summary: "recorded usage receipt",
            receiptRefs: ["receipt.openagents.resource_usage_receipt.v1.0"],
          },
          { kind: "completed", summary: "run completed" },
        ],
      ]

  const fakeFetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input.toString()
    const method = init?.method ?? "GET"
    const body = init?.body ? JSON.parse(String(init.body)) : undefined
    calls.push({ url, method, body })

    // Grant resolver endpoint (#4999 neutral contract).
    if (url.includes("/grants/resolve")) {
      if (options?.failGrant) {
        return new Response("nope", { status: 403 })
      }
      const grant = {
        grantRef: body.grantRef,
        provider: "chatgpt_codex",
        providerAccountRef: body.providerAccountRef,
        providerSecretRef: "secret://test-secret",
        runnerSessionId: body.runnerSessionId,
        expiresAt: new Date(Date.now() + 3_600_000).toISOString(),
        status: "issued",
        materialization: {
          kind: "probe_chatgpt_auth",
          provider: "chatgpt_codex",
          providerSecretRef: "secret://test-secret",
          target: { kind: "env", name: "PROBE_CHATGPT_AUTH_CONTENT" },
          homeIsolation: "per_run",
          scrubAfterCloseout: true,
        },
      }
      return Response.json(grant)
    }

    // Placement endpoint.
    if (url.endsWith("/v1/placement")) {
      return Response.json({
        binding: {
          contractVersion: "openagents.codex_placement_assignment.v1",
          runId: body.run_id,
          externalRunId,
          lane: "cloud-gcp",
          providerLane: "gcp",
          runnerId: "oa-gce-ephemeral-test",
          capacityClassId: "gce.ephemeral.standard.v1",
          sandboxMode: "danger_full_access",
          reason: "policy_default_gce",
          costDriven: false,
          caps: { sessionTtlMs: 28_800_000 },
        },
        externalRunId,
        status: "queued",
        events: [
          { kind: "queued", summary: "queued on cloud control daemon" },
          { kind: "started", summary: "started codex run" },
        ],
      })
    }

    // Events feed.
    if (url.includes("/events")) {
      const next = queue.shift() ?? []
      const terminal = next.some((e) =>
        ["completed", "failed", "timeout", "cancelled"].includes(e.kind),
      )
      return Response.json({
        status: terminal ? "completed" : "running",
        events: next,
        cursor: 0,
      })
    }

    return new Response("not found", { status: 404 })
  }) as unknown as typeof fetch

  return { fakeFetch, calls, externalRunId }
}

async function withFixture<T>(
  fn: (fixture: {
    proofDir: string
    summary: ReturnType<typeof createBootstrapSummary>
    worktree: string
  }) => Promise<T>,
) {
  const root = mkdtempSync(join(tmpdir(), "pylon-cloud-backend-"))
  try {
    const pylonHome = join(root, "pylon-home")
    const worktree = join(root, "worktree")
    const proofDir = join(root, "proofs")
    await mkdir(pylonHome, { recursive: true })
    await mkdir(worktree, { recursive: true })
    await writeFile(join(pylonHome, "config.json"), `${JSON.stringify({ dev: { accounts: [] } })}\n`)
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    return await fn({ proofDir, summary, worktree })
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

const GRANT_BINDING: CloudSessionGrantBinding = {
  authGrantRef: "codex-auth-grant_test",
  providerAccountRef: "provider-account_test",
  ownerRef: "owner://sha256/test",
}

describe("OpenAgents Cloud execution backend (#4997)", () => {
  test("resolveCloudControlConfig gates on neutral env (off by default)", () => {
    expect(resolveCloudControlConfig({}).configured).toBe(false)
    expect(
      resolveCloudControlConfig({ OA_CLOUD_CONTROL_URL: "https://cloud.example" }).configured,
    ).toBe(false)
    const ok = resolveCloudControlConfig({
      OA_CLOUD_CONTROL_URL: "https://cloud.example/",
      OA_CLOUD_CONTROL_TOKEN: "tok",
    })
    expect(ok.configured).toBe(true)
    if (ok.configured) {
      // Trailing slash trimmed.
      expect(ok.config.baseUrl).toBe("https://cloud.example")
      expect(ok.config.bearerToken).toBe("tok")
    }
  })

  test("spawn cloud-gcp resolves grant, places run, maps events, surfaces terminal + artifact + receipt", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const { fakeFetch, calls, externalRunId } = makeFakeCloudControlPlane()
      const env = {
        OA_CLOUD_CONTROL_URL: "https://cloud.example",
        OA_CLOUD_CONTROL_TOKEN: "control-token",
        OA_CODEX_GRANT_RESOLVE_URL: "https://grants.example",
        OA_CODEX_GRANT_RESOLVE_TOKEN: "grant-token",
      }
      const resolved = resolveCloudControlConfig(env)
      expect(resolved.configured).toBe(true)
      if (!resolved.configured) throw new Error("unreachable")

      const cloudExecutor = makeCloudControlSessionExecutor({
        config: resolved.config,
        env,
        fetchImpl: fakeFetch,
        pollIntervalMs: 1,
        grantBindingForSession: () => GRANT_BINDING,
      })

      const actions = createControlSessionActions({
        env,
        cloudExecutor,
        proofsDir: proofDir,
        summary,
      })

      const spawned = await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        worktreePath: worktree,
        objective: "make the cloud loop run",
        verify: ["bun", "--version"],
        lane: "cloud-gcp",
      })

      // Wait for terminal state.
      let row: Awaited<ReturnType<typeof actions.list>>[number] | undefined
      for (let attempt = 0; attempt < 200; attempt += 1) {
        const list = await actions.list()
        row = list.find((s) => s.sessionRef === spawned.sessionRef)
        if (row && (row.state === "completed" || row.state === "failed")) break
        await Bun.sleep(5)
      }

      expect(row?.state).toBe("completed")
      expect(row?.lane).toBe("cloud-gcp")

      // Runner binding provenance is real ("running on Google GCE").
      expect(row?.cloudRunner).toEqual({
        lane: "cloud-gcp",
        providerLane: "gcp",
        runnerId: "oa-gce-ephemeral-test",
        externalRunId,
      })
      // resource_usage_receipt ref surfaced.
      expect(row?.resourceUsageReceiptRef).toBe(
        "receipt.openagents.resource_usage_receipt.v1.0",
      )

      // The loop actually called grant resolve -> placement -> events.
      const urls = calls.map((c) => c.url)
      expect(urls.some((u) => u.includes("/grants/resolve"))).toBe(true)
      expect(urls.some((u) => u.endsWith("/v1/placement"))).toBe(true)
      expect(urls.some((u) => u.includes("/events"))).toBe(true)

      // Placement body carried the lane + refs.
      const placement = calls.find((c) => c.url.endsWith("/v1/placement"))
      expect((placement?.body as Record<string, unknown>).lane).toBe("cloud-gcp")
      expect((placement?.body as Record<string, unknown>).wallet_authority).toBe(false)
      expect((placement?.body as Record<string, unknown>).auth_grant_ref).toBe(
        "codex-auth-grant_test",
      )

      // Events were mapped into the Pylon session stream (lane-transparent).
      const detail = await actions.events(spawned.sessionRef)
      const texts = detail.recentEvents
        .map((e) => e.messageText ?? "")
        .filter((t) => t.length > 0)
      expect(texts.some((t) => t.includes("placed on gcp:oa-gce-ephemeral-test"))).toBe(true)
      expect(texts.some((t) => t.includes("cloning repository"))).toBe(true)
      expect(texts.some((t) => t.includes("artifact"))).toBe(true)

      // A retained proof artifact was written and is lane-transparent.
      const artifact = await actions.artifact(spawned.sessionRef)
      expect(artifact.kind).toBe("proof")
    })
  })

  test("cloud run failure ends in failed terminal state without raw error text", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      const { fakeFetch } = makeFakeCloudControlPlane({
        scriptedEvents: [{ kind: "failed", summary: "cloud run failed" }],
      })
      const env = {
        OA_CLOUD_CONTROL_URL: "https://cloud.example",
        OA_CLOUD_CONTROL_TOKEN: "control-token",
      }
      const resolved = resolveCloudControlConfig(env)
      if (!resolved.configured) throw new Error("unreachable")
      const cloudExecutor = makeCloudControlSessionExecutor({
        config: resolved.config,
        env,
        fetchImpl: fakeFetch,
        pollIntervalMs: 1,
        // No grant binding -> grant resolution skipped; placement still attempted.
        grantBindingForSession: () => ({}),
      })
      const actions = createControlSessionActions({
        env,
        cloudExecutor,
        proofsDir: proofDir,
        summary,
      })
      const spawned = await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        worktreePath: worktree,
        objective: "fail in the cloud",
        verify: ["bun", "--version"],
        lane: "cloud-gcp",
      })

      let row: Awaited<ReturnType<typeof actions.list>>[number] | undefined
      for (let attempt = 0; attempt < 200; attempt += 1) {
        const list = await actions.list()
        row = list.find((s) => s.sessionRef === spawned.sessionRef)
        if (row && (row.state === "completed" || row.state === "failed")) break
        await Bun.sleep(5)
      }
      expect(row?.state).toBe("failed")
      expect(row?.cloudRunner?.lane).toBe("cloud-gcp")
    })
  })

  test("cloud lane falls back to local executor when no cloud executor is configured", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      let localExecutorCalled = false
      const actions = createControlSessionActions({
        env: {}, // no cloud config
        executor: async () => {
          localExecutorCalled = true
          throw new Error("local executor reached")
        },
        // No cloudExecutor and no factory -> cloud lanes degrade to local.
        proofsDir: proofDir,
        summary,
      })
      await actions.spawn({
        type: "session.spawn",
        adapter: "codex",
        worktreePath: worktree,
        objective: "cloud lane with no cloud config",
        verify: ["bun", "--version"],
        lane: "cloud-gcp",
      })
      for (let attempt = 0; attempt < 50 && !localExecutorCalled; attempt += 1) {
        await Bun.sleep(5)
      }
      expect(localExecutorCalled).toBe(true)
    })
  })
})
