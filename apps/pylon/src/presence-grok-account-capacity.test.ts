import { describe, expect, test } from "vite-plus/test"
import { mkdtemp, rm, writeFile } from "node:fs/promises"
import { homedir, tmpdir } from "node:os"
import { join } from "node:path"

import {
  GROK_CODING_CAPABILITY_REF,
  grokAccountCapacityRefs,
  grokPerAccountConcurrency,
  localGrokAccountCapacities,
  localGrokAccountReadiness,
  sendHeartbeat,
} from "./presence.js"
import { hashPylonAccountRef } from "./account-registry.js"
import { createBootstrapSummary, parseBootstrapArgs } from "./bootstrap.js"

describe("#8640 per-account Grok heartbeat capacity", () => {
  test("probes only the exact Pylon-owned named home with an isolated environment", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-grok-capacity-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const readyHome = join(home, "accounts", "grok", "grok-ready")
      const externalHome = join(home, "external-grok")
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [
            { provider: "grok", ref: "grok-ready", home: readyHome },
            { provider: "grok", ref: "grok-external", home: externalHome },
            { provider: "grok", ref: "grok-default", home: join(homedir(), ".grok") },
          ],
        },
      }))

      const probedHomes: string[] = []
      const readiness = await localGrokAccountReadiness(
        summary,
        {
          GROK_HOME: "",
          PYLON_HOME: home,
          XAI_API_KEY: "must-not-reach-probe",
        },
        {
          readinessProbe: async ({ env }) => {
            probedHomes.push(env.GROK_HOME ?? "")
            expect(env.XAI_API_KEY).toBeUndefined()
            return { plane: "cli_session", ready: true }
          },
        },
      )

      expect(probedHomes).toEqual([readyHome])
      expect(readiness).toEqual([
        {
          accountRefHash: hashPylonAccountRef("grok", "grok-ready"),
          ready: true,
        },
        {
          accountRefHash: hashPylonAccountRef("grok", "grok-external"),
          ready: false,
          reason: "custody_invalid",
        },
      ])
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("advertises bounded pooled and per-account slots only after readiness proof", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-grok-heartbeat-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const accountRef = "grok-fc5"
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [{
            provider: "grok",
            ref: accountRef,
            home: join(home, "accounts", "grok", accountRef),
          }],
        },
      }))
      const accountRefHash = hashPylonAccountRef("grok", accountRef)
      const accountKey = accountRefHash.split(".").at(-1)!
      const accounts = await localGrokAccountCapacities(
        summary,
        {
          OPENAGENTS_PYLON_GROK_ACCOUNT_CONCURRENCY: "2",
          PYLON_HOME: home,
        },
        {
          busyByAccount: { [accountRefHash]: 1 },
          readinessProbe: async () => ({ plane: "cli_session", ready: true }),
        },
      )

      expect(accounts).toEqual([{
        accountKey,
        accountRefHash,
        available: 1,
        busy: 1,
        queued: 0,
        ready: 2,
      }])
      expect(grokAccountCapacityRefs(accounts)).toEqual({
        capacityRefs: [
          "capacity.coding.grok.ready=2",
          "capacity.coding.grok.available=1",
          `capacity.coding.grok.account.${accountKey}.ready=2`,
          `capacity.coding.grok.account.${accountKey}.available=1`,
        ],
        loadRefs: [
          "load.coding.grok.busy=1",
          "load.coding.grok.queued=0",
          `load.coding.grok.account.${accountKey}.busy=1`,
          `load.coding.grok.account.${accountKey}.queued=0`,
        ],
      })
      expect(GROK_CODING_CAPABILITY_REF).toBe("capability.public.coding.grok")
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("does not publish a slot for failed readiness and bounds concurrency", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-grok-unready-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const accountRef = "grok-unready"
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [{
            provider: "grok",
            ref: accountRef,
            home: join(home, "accounts", "grok", accountRef),
          }],
        },
      }))
      const accounts = await localGrokAccountCapacities(
        summary,
        { PYLON_HOME: home },
        { readinessProbe: async () => ({ plane: "cli_session", ready: false }) },
      )
      expect(accounts).toEqual([])
      expect(grokAccountCapacityRefs(accounts)).toEqual({ capacityRefs: [], loadRefs: [] })
      expect(grokPerAccountConcurrency({ OPENAGENTS_PYLON_GROK_ACCOUNT_CONCURRENCY: "999" })).toBe(64)
      expect(grokPerAccountConcurrency({ OPENAGENTS_PYLON_GROK_ACCOUNT_CONCURRENCY: "invalid" })).toBe(1)
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("heartbeat carries proved Grok capacity and capability without private custody data", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-grok-heartbeat-wire-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
        PYLON_HOME: home,
      })
      const accountRef = "grok-wire"
      const accountHome = join(home, "accounts", "grok", accountRef)
      await writeFile(summary.paths.config, JSON.stringify({
        dev: {
          accounts: [{ provider: "grok", ref: accountRef, home: accountHome }],
        },
      }))
      const bodies: Array<Record<string, unknown>> = []
      const fetchImpl = (async (_input, init) => {
        bodies.push(JSON.parse(String(init?.body ?? "{}")) as Record<string, unknown>)
        return Response.json({ ok: true })
      }) as typeof fetch
      await sendHeartbeat(summary, {
        baseUrl: "https://openagents.example",
        env: {
          CODEX_HOME: join(home, "empty-codex"),
          OPENAGENTS_PYLON_GROK_ACCOUNT_CONCURRENCY: "1",
          PYLON_ACCOUNT_HOME_ROOT: join(home, "empty-siblings"),
          PYLON_HOME: home,
        },
        fetch: fetchImpl,
        grokReadinessProbe: async () => ({ plane: "cli_session", ready: true }),
        now: () => new Date("2026-07-10T07:00:00.000Z"),
      })

      const body = bodies.at(-1)
      expect(body).toBeDefined()
      if (body === undefined) throw new Error("heartbeat request body missing")
      const capacityRefs = body.capacityRefs as string[]
      const loadRefs = body.loadRefs as string[]
      const capabilityRefs = body.capabilityRefs as string[]
      expect(capacityRefs).toContain("capacity.coding.grok.ready=1")
      expect(capacityRefs).toContain("capacity.coding.grok.available=1")
      expect(loadRefs).toContain("load.coding.grok.busy=0")
      expect(loadRefs).toContain("load.coding.grok.queued=0")
      expect(capabilityRefs).toContain(GROK_CODING_CAPABILITY_REF)
      const serialized = JSON.stringify(body)
      expect(serialized).not.toContain(accountRef)
      expect(serialized).not.toContain(accountHome)
      expect(serialized).not.toContain("XAI_API_KEY")
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })
})
