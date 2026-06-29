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
  makeCloudControlClient,
  OPENAGENTS_CODEX_WORKROOM_EVENT_KINDS,
  resolveCloudControlConfig,
  type CloudWorkroomEvent,
  type CloudWorkroomEventKind,
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
  test("cloud HTTP client round-trips every codex_workroom_event.v1 kind", async () => {
    const rawEvents = OPENAGENTS_CODEX_WORKROOM_EVENT_KINDS.map((kind) => {
      const base = {
        summary: `summary for ${kind}`,
        artifactRefs: kind === "artifact" ? [`artifact.${kind}`] : undefined,
        receiptRefs:
          kind === "receipt" || kind === "cloud.gce.resource_usage_receipt"
            ? [`receipt.${kind}`]
            : undefined,
      }
      switch (kind) {
        case "cloud.gce.provisioned":
          return { ...base, kind: "started", type: kind }
        case "cloud.gce.cleanup":
          return { ...base, kind: "cleanup", type: kind }
        case "cloud.gce.degraded":
          return { ...base, kind: "log", type: kind }
        case "cloud.gce.resource_usage_receipt":
          return { ...base, kind: "receipt", type: kind }
        default:
          return { ...base, kind }
      }
    })
    const fakeFetch = (async () =>
      Response.json({
        status: "running",
        events: rawEvents,
        cursor: rawEvents.length,
      })) as unknown as typeof fetch

    const client = makeCloudControlClient(
      { baseUrl: "https://cloud.example", bearerToken: "test-token" },
      fakeFetch,
    )
    const page = await client.fetchEvents("external-run", 0)

    expect(page.events.map((event) => event.kind)).toEqual(
      [...OPENAGENTS_CODEX_WORKROOM_EVENT_KINDS],
    )
    expect(page.events.every((event) => event.summary === `summary for ${event.kind}`)).toBe(true)
    expect(page.events.find((event) => event.kind === "artifact")?.artifactRefs).toEqual([
      "artifact.artifact",
    ])
    expect(page.events.find((event) => event.kind === "receipt")?.receiptRefs).toEqual([
      "receipt.receipt",
    ])
    expect(
      page.events.find((event) => event.kind === "cloud.gce.resource_usage_receipt")
        ?.receiptRefs,
    ).toEqual(["receipt.cloud.gce.resource_usage_receipt"])
  })

  test("cloud HTTP client rejects unknown workroom event kinds instead of inventing phases", async () => {
    const fakeFetch = (async () =>
      Response.json({
        status: "running",
        events: [
          { kind: "log", summary: "valid event" },
          { kind: "cloud.gce.unknown", summary: "invalid event" },
          { kind: "made.up.kind", summary: "invalid event" },
        ],
        cursor: 3,
      })) as unknown as typeof fetch

    const client = makeCloudControlClient(
      { baseUrl: "https://cloud.example", bearerToken: "test-token" },
      fakeFetch,
    )
    const page = await client.fetchEvents("external-run", 0)

    expect(page.events.map((event) => event.kind)).toEqual(["log"])
  })

  test("cloud.gce.resource alias normalizes to the canonical workroom kind", async () => {
    const fakeFetch = (async () =>
      Response.json({
        status: "running",
        events: [
          {
            kind: "receipt",
            type: "cloud.gce.resource",
            summary: "resource alias",
            receiptRefs: ["receipt.alias"],
          },
        ],
        cursor: 1,
      })) as unknown as typeof fetch

    const client = makeCloudControlClient(
      { baseUrl: "https://cloud.example", bearerToken: "test-token" },
      fakeFetch,
    )
    const page = await client.fetchEvents("external-run", 0)

    expect(page.events).toEqual([
      {
        kind: "cloud.gce.resource_usage_receipt" satisfies CloudWorkroomEventKind,
        summary: "resource alias",
        receiptRefs: ["receipt.alias"],
      },
    ])
  })

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

  test("cloud-gcp maps the four cloud.gce.* lease-lifecycle kinds: receipt round-trips, VM provenance surfaced", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      // A scripted cloud stream that carries the four GCE lease-lifecycle
      // discriminators on the `type` field exactly the way the cloud control
      // plane emits them (cloud commit fbd62cf, JobEvent.type = cloud.gce.*).
      // The broad workroom `kind` reuses the existing kinds, so without the
      // #5005 mapping these would round-trip as plain started/receipt/log and
      // the cleanup event would be dropped entirely.
      const scriptedEvents: CloudWorkroomEvent[] = [
        // cloud.gce.provisioned -> VM lease acquired (provision receipt ref).
        {
          kind: "started",
          type: "cloud.gce.provisioned",
          summary: "GCE capacity lease ready on the fake provisioner.",
          receiptRefs: ["sha256:provision-receipt-0"],
        },
        // cloud.gce.degraded -> a failed acquire / fallback (must stay visible).
        {
          kind: "log",
          type: "cloud.gce.degraded",
          summary: "GCE provisioning unavailable; run continues on local control host.",
        },
        // cloud.gce.resource_usage_receipt -> the refs-only
        // resource_usage_receipt.v1 digest (MOST IMPORTANT: must round-trip).
        {
          kind: "receipt",
          type: "cloud.gce.resource_usage_receipt",
          summary: "openagents.resource_usage_receipt.v1 emitted for GCE session.",
          receiptRefs: ["sha256:resource-usage-receipt-1"],
        },
        // cloud.gce.cleanup -> VM released (cleanup receipt ref). Non-terminal.
        {
          kind: "cleanup",
          type: "cloud.gce.cleanup",
          summary: "GCE capacity lease released; cleanup receipt minted.",
          receiptRefs: ["sha256:cleanup-receipt-2"],
        },
        // The run itself terminates AFTER the GCE lifecycle events.
        { kind: "completed", summary: "run completed" },
      ] as CloudWorkroomEvent[]

      const { fakeFetch } = makeFakeCloudControlPlane({ scriptedEvents })
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
        objective: "run on a cloud GCE VM",
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

      // The GCE lifecycle events are NON-terminal: the run still completes.
      expect(row?.state).toBe("completed")
      expect(row?.lane).toBe("cloud-gcp")

      // MOST IMPORTANT: the resource_usage_receipt.v1 ref round-trips to the
      // session exactly like SHC/local. Provision/cleanup receipts are
      // lifecycle provenance only and must not mask the usage receipt.
      expect(row?.resourceUsageReceiptRef).toBe("sha256:resource-usage-receipt-1")

      // VM lifecycle provenance lines appear in the lane-transparent stream.
      const detail = await actions.events(spawned.sessionRef)
      const texts = detail.recentEvents
        .map((e) => e.messageText ?? "")
        .filter((t) => t.length > 0)
      expect(texts.some((t) => t.includes("GCE capacity lease ready"))).toBe(true)
      // degraded is visible (failed acquire / fallback signal).
      expect(texts.some((t) => t.includes("GCE provisioning unavailable"))).toBe(true)
      expect(texts.some((t) => t.includes("resource_usage_receipt.v1 emitted"))).toBe(true)
      // cleanup (VM released) is surfaced rather than silently dropped.
      expect(texts.some((t) => t.includes("GCE capacity lease released"))).toBe(true)
    })
  })

  test("cloud.gce.resource alias surfaces the resource_usage_receipt ref", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      // The issue refers to the resource event as `cloud.gce.resource`; the
      // cloud emits it as `cloud.gce.resource_usage_receipt`. Pylon accepts the
      // issue's alias and normalizes it so the receipt ref still round-trips.
      const scriptedEvents = [
        {
          kind: "receipt",
          type: "cloud.gce.resource",
          summary: "resource usage receipt (alias spelling)",
          receiptRefs: ["sha256:aliased-resource-receipt"],
        },
        { kind: "completed", summary: "run completed" },
      ] as unknown as CloudWorkroomEvent[]

      const { fakeFetch } = makeFakeCloudControlPlane({ scriptedEvents })
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
        objective: "resource alias",
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
      expect(row?.state).toBe("completed")
      expect(row?.resourceUsageReceiptRef).toBe("sha256:aliased-resource-receipt")
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
