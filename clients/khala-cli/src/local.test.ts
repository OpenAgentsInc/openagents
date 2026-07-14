import { describe, expect, test } from "vite-plus/test"
import { spawn } from "node:child_process"
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import {
  KHALA_LOCAL_STORE_SCHEMA_VERSION,
  khalaLocalStorePath,
  openKhalaLocalStore,
  type KhalaLocalStore,
} from "./local-store.js"
import { KHALA_LOCAL_HOST, startKhalaLocalServer, type KhalaLocalServer } from "./local.js"

function tempEnv(prefix: string): Record<string, string | undefined> {
  const dir = mkdtempSync(join(tmpdir(), prefix))
  return {
    HOME: dir,
    XDG_CONFIG_HOME: join(dir, ".config"),
  }
}

async function withServer(
  options: Partial<Parameters<typeof startKhalaLocalServer>[0]>,
  run: (server: KhalaLocalServer) => Promise<void>,
): Promise<void> {
  const server = await startKhalaLocalServer({
    env: options.env ?? tempEnv("khala-local-test-"),
    ...options,
  })
  try {
    await run(server)
  } finally {
    await server.close()
  }
}

async function pair(server: KhalaLocalServer, token: string): Promise<Response> {
  return fetch(`${server.baseUrl}/api/pair`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ token }),
  })
}

describe("khala local store", () => {
  test("initializes a fresh device-local store", async () => {
    const env = tempEnv("khala-store-init-")
    const opened = await openKhalaLocalStore(env)
    expect(opened.outcome).toBe("initialized")
    expect(opened.store.schemaVersion).toBe(KHALA_LOCAL_STORE_SCHEMA_VERSION)
    expect(opened.store.deviceId).toStartWith("khala-local-")
    expect(opened.store.knownEnvironments).toEqual([])
    expect(opened.store.grants).toEqual([])

    const reopened = await openKhalaLocalStore(env)
    expect(reopened.outcome).toBe("loaded")
    expect(reopened.store.deviceId).toBe(opened.store.deviceId)
  })

  test("migrates a legacy schemaVersion-less store forward, preserving the deviceId", async () => {
    const env = tempEnv("khala-store-migrate-")
    const path = khalaLocalStorePath(env)
    const { mkdirSync } = await import("node:fs")
    mkdirSync(dirname(path), { recursive: true })
    writeFileSync(path, JSON.stringify({ deviceId: "khala-local-legacy00" }))

    const opened = await openKhalaLocalStore(env)
    expect(opened.outcome).toBe("migrated")
    expect(opened.store.schemaVersion).toBe(KHALA_LOCAL_STORE_SCHEMA_VERSION)
    expect(opened.store.deviceId).toBe("khala-local-legacy00")
    expect(opened.store.knownEnvironments).toEqual([])
    expect(typeof opened.store.migratedAt).toBe("string")
  })

  test("refuses a store written by a newer CLI instead of clobbering it", async () => {
    const env = tempEnv("khala-store-newer-")
    const path = khalaLocalStorePath(env)
    const { mkdirSync } = await import("node:fs")
    mkdirSync(dirname(path), { recursive: true })
    writeFileSync(path, JSON.stringify({ schemaVersion: KHALA_LOCAL_STORE_SCHEMA_VERSION + 1, deviceId: "x" }))
    await expect(openKhalaLocalStore(env)).rejects.toThrow(/newer than this CLI/)
  })

  test("refuses corrupt JSON instead of silently rebuilding", async () => {
    const env = tempEnv("khala-store-corrupt-")
    const path = khalaLocalStorePath(env)
    const { mkdirSync } = await import("node:fs")
    mkdirSync(dirname(path), { recursive: true })
    writeFileSync(path, "{not json")
    await expect(openKhalaLocalStore(env)).rejects.toThrow(/not valid JSON/)
  })
})

describe("khala up pairing server", () => {
  test("binds 127.0.0.1 only", async () => {
    await withServer({}, async server => {
      expect(server.host).toBe(KHALA_LOCAL_HOST)
      expect(server.baseUrl).toStartWith("http://127.0.0.1:")
    })
  })

  test("pairing URL carries the token in the URL fragment, never a query param", async () => {
    await withServer({}, async server => {
      const minted = server.mintPairingUrl()
      expect(minted.url).toBe(`http://127.0.0.1:${server.port}/pair#token=${minted.token}`)
      expect(minted.url).not.toContain("?")
    })
  })

  test("pairing token is single-use: replay is rejected after a successful pair", async () => {
    await withServer({}, async server => {
      const minted = server.mintPairingUrl()
      const first = await pair(server, minted.token)
      expect(first.status).toBe(200)
      const payload = (await first.json()) as { ok: boolean; grantSecret: string }
      expect(payload.ok).toBe(true)
      expect(payload.grantSecret).toStartWith("khala_grant_")

      const replay = await pair(server, minted.token)
      expect(replay.status).toBe(401)
      expect(((await replay.json()) as { reason: string }).reason).toBe("used_token")
    })
  })

  test("pairing token expires: a token past its TTL is rejected", async () => {
    let nowMs = 1_000_000
    await withServer({ pairingTokenTtlMs: 120_000, now: () => nowMs }, async server => {
      const minted = server.mintPairingUrl()
      expect(minted.expiresAtMs).toBe(1_120_000)
      nowMs = 1_120_001
      const late = await pair(server, minted.token)
      expect(late.status).toBe(401)
      expect(((await late.json()) as { reason: string }).reason).toBe("expired_token")
    })
  })

  test("a token in a query param is REJECTED on every route and is not consumed", async () => {
    await withServer({}, async server => {
      const minted = server.mintPairingUrl()

      const pageAttempt = await fetch(`${server.baseUrl}/pair?token=${minted.token}`)
      expect(pageAttempt.status).toBe(400)
      expect(((await pageAttempt.json()) as { reason: string }).reason).toBe("token_in_query_rejected")

      const postAttempt = await fetch(`${server.baseUrl}/api/pair?token=${minted.token}`, { method: "POST" })
      expect(postAttempt.status).toBe(400)

      const statusAttempt = await fetch(`${server.baseUrl}/api/status?token=${minted.token}`)
      expect(statusAttempt.status).toBe(400)

      // The rejected attempts must not have consumed the token: the
      // fragment-disciplined body exchange still works afterwards.
      const legit = await pair(server, minted.token)
      expect(legit.status).toBe(200)
    })
  })

  test("request logs never contain the pairing token (method + pathname only)", async () => {
    const logs: Array<string> = []
    await withServer({ log: line => logs.push(line) }, async server => {
      const minted = server.mintPairingUrl()
      await fetch(`${server.baseUrl}/pair`)
      await fetch(`${server.baseUrl}/pair?token=${minted.token}`)
      await pair(server, minted.token)
      expect(logs.length).toBeGreaterThanOrEqual(3)
      for (const line of logs) {
        expect(line).not.toContain(minted.token)
        expect(line).not.toContain("token=")
      }
      expect(logs).toContain("GET /pair")
      expect(logs).toContain("POST /api/pair")
    })
  })

  test("control surface is pairing-gated by default; a paired grant unlocks it", async () => {
    const env = tempEnv("khala-local-gate-")
    await withServer({ env }, async server => {
      const anonymous = await fetch(`${server.baseUrl}/api/status`)
      expect(anonymous.status).toBe(401)
      expect(((await anonymous.json()) as { reason: string }).reason).toBe("pairing_required")

      const forged = await fetch(`${server.baseUrl}/api/status`, {
        headers: { authorization: "Bearer khala_grant_not-a-real-grant" },
      })
      expect(forged.status).toBe(401)

      const minted = server.mintPairingUrl()
      const paired = (await (await pair(server, minted.token)).json()) as { grantSecret: string; grantId: string }
      const authorized = await fetch(`${server.baseUrl}/api/status`, {
        headers: { authorization: `Bearer ${paired.grantSecret}` },
      })
      expect(authorized.status).toBe(200)
      const body = (await authorized.json()) as { ok: boolean; environmentRef: string; grantId: string }
      expect(body.ok).toBe(true)
      expect(body.environmentRef).toBe(server.environmentRef)
      expect(body.grantId).toBe(paired.grantId)
    })
  })

  test("pairing persists a KnownEnvironment entry and a hashed grant, never the secret", async () => {
    const env = tempEnv("khala-local-known-env-")
    await withServer({ env }, async server => {
      const minted = server.mintPairingUrl()
      const paired = (await (await pair(server, minted.token)).json()) as { grantSecret: string; grantId: string }

      const raw = readFileSync(khalaLocalStorePath(env), "utf8")
      expect(raw).not.toContain(paired.grantSecret)
      expect(raw).not.toContain(minted.token)

      const store = JSON.parse(raw) as KhalaLocalStore
      expect(store.knownEnvironments).toHaveLength(1)
      expect(store.knownEnvironments[0]?.environmentRef).toBe(server.environmentRef)
      expect(store.knownEnvironments[0]?.url).toBe(server.baseUrl)
      expect(store.knownEnvironments[0]?.grantId).toBe(paired.grantId)
      expect(store.grants).toHaveLength(1)
      expect(store.grants[0]?.kind).toBe("opaque-device-grant")
      expect(store.grants[0]?.scope).toBe("local-control")
      expect(store.grants[0]?.secretHash).toMatch(/^[0-9a-f]{64}$/)
    })
  })
})

describe("khala up fresh-machine smoke", () => {
  // Bounded fresh-machine proof for #8784: a clean temp HOME/XDG config (no
  // prior Khala state) runs ONE command — `khala up --smoke` — which must
  // init the store, listen on loopback, mint a fragment pairing URL, and
  // complete a real HTTP pairing loop into a gated, usable session. This does
  // not exercise registry download (npx fetch) — that path is covered by the
  // publishing runbook's install smokes.
  test("one command on a clean home reaches a usable paired local session", async () => {
    const env = tempEnv("khala-up-fresh-")
    const packageDir = dirname(dirname(fileURLToPath(import.meta.url)))
    const transcript = await new Promise<{ code: number | null; stdout: string; stderr: string }>((resolve, reject) => {
      const child = spawn(process.execPath, [join(packageDir, "src", "index.ts"), "up", "--smoke"], {
        cwd: packageDir,
        env: {
          PATH: process.env.PATH,
          HOME: env.HOME,
          XDG_CONFIG_HOME: env.XDG_CONFIG_HOME,
        },
        stdio: ["ignore", "pipe", "pipe"],
      })
      let stdout = ""
      let stderr = ""
      child.stdout.on("data", chunk => (stdout += String(chunk)))
      child.stderr.on("data", chunk => (stderr += String(chunk)))
      child.on("error", reject)
      child.on("close", code => resolve({ code, stdout, stderr }))
    })

    // Surface the receipt transcript in test output.
    console.log(`--- khala up --smoke transcript (clean HOME=${env.HOME}) ---`)
    console.log(transcript.stdout.trimEnd())
    console.log("--- end transcript ---")

    expect(transcript.code).toBe(0)
    expect(transcript.stdout).toContain("Khala local runtime ready. No account required.")
    expect(transcript.stdout).toContain("(schema v1, initialized)")
    expect(transcript.stdout).toContain("Listening on http://127.0.0.1:")
    expect(transcript.stdout).toMatch(/http:\/\/127\.0\.0\.1:\d+\/pair#token=[A-Za-z0-9_-]+/)
    expect(transcript.stdout).toContain("smoke: ok — token in a query param is REJECTED (fragment-only discipline)")
    expect(transcript.stdout).toContain("smoke: ok — pairing token is single-use (replay rejected)")
    expect(transcript.stdout).toContain("smoke: ok — control surface is pairing-gated by default (401 unauthenticated)")
    expect(transcript.stdout).toContain("smoke: ok — paired grant reaches the control surface (usable session)")
    expect(transcript.stdout).toContain("smoke: PASS — init -> migrate/load -> listen -> mint -> pair -> gated control")
  }, 30_000)
})
