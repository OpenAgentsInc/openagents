import { createHash, randomBytes, timingSafeEqual } from "node:crypto"
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http"

import {
  openKhalaLocalStore,
  writeKhalaLocalStore,
  type KhalaDeviceGrant,
  type KhalaKnownEnvironment,
  type KhalaLocalStore,
  type KhalaLocalStoreOpenResult,
} from "./local-store.js"

// Zero-install local runtime front door (issue #8784, T3-teardown follow-on):
// `khala up` initializes/migrates the device-local store, starts a loopback
// HTTP runtime, mints a short-lived single-use pairing token, and prints a
// pairing URL with the token in the URL FRAGMENT so it never reaches server
// request logs. Pairing is the default gate: every control route requires a
// paired device grant, so there is no unauthenticated control surface.

export const KHALA_LOCAL_HOST = "127.0.0.1"
export const DEFAULT_PAIRING_TOKEN_TTL_MS = 5 * 60 * 1000

interface PairingTokenRecord {
  readonly tokenHash: Buffer
  readonly expiresAtMs: number
  used: boolean
}

export interface KhalaLocalServerOptions {
  readonly env: Record<string, string | undefined>
  readonly port?: number | undefined
  readonly pairingTokenTtlMs?: number | undefined
  readonly now?: (() => number) | undefined
  // Request log sink. Receives method + pathname ONLY — never query strings,
  // bodies, or tokens (fragments never reach the server by construction).
  readonly log?: ((line: string) => void) | undefined
}

export interface KhalaPairResult {
  readonly ok: true
  readonly environmentRef: string
  readonly grantId: string
  readonly grantSecret: string
}

export interface KhalaLocalServer {
  readonly host: string
  readonly port: number
  readonly baseUrl: string
  readonly storePath: string
  readonly storeOutcome: KhalaLocalStoreOpenResult["outcome"]
  readonly environmentRef: string
  /** Mints a fresh short-lived single-use pairing token and returns the fragment-style pairing URL. */
  mintPairingUrl(): { readonly token: string; readonly url: string; readonly expiresAtMs: number }
  close(): Promise<void>
}

export async function startKhalaLocalServer(options: KhalaLocalServerOptions): Promise<KhalaLocalServer> {
  const now = options.now ?? (() => Date.now())
  const ttlMs = options.pairingTokenTtlMs ?? DEFAULT_PAIRING_TOKEN_TTL_MS
  const log = options.log ?? (() => {})

  const opened = await openKhalaLocalStore(options.env)
  let store: KhalaLocalStore = opened.store
  const environmentRef = store.deviceId

  const pairingTokens: Array<PairingTokenRecord> = []
  // Grant secrets verified by hash; secrets themselves are never persisted.
  const persistStore = async (next: KhalaLocalStore): Promise<void> => {
    store = next
    await writeKhalaLocalStore(opened.path, next)
  }

  const handler = async (request: IncomingMessage, response: ServerResponse): Promise<void> => {
    const url = new URL(request.url ?? "/", `http://${KHALA_LOCAL_HOST}`)
    // Log method + pathname only. Never the query string, never a body.
    log(`${request.method ?? "GET"} ${url.pathname}`)

    // Fragment discipline is enforced, not hoped: a pairing token in a query
    // parameter is rejected on EVERY route before any handling, and is never
    // looked up or consumed. Tokens are only accepted from a POST body, where
    // the browser page copies them out of the URL fragment.
    if (url.searchParams.has("token")) {
      sendJson(response, 400, {
        ok: false,
        reason: "token_in_query_rejected",
        message: "Pairing tokens are only accepted in the URL fragment (#token=...), never as a query parameter.",
      })
      return
    }

    if (request.method === "GET" && url.pathname === "/") {
      response.writeHead(302, { location: "/pair" })
      response.end()
      return
    }

    if (request.method === "GET" && url.pathname === "/pair") {
      response.writeHead(200, { "content-type": "text/html; charset=utf-8" })
      response.end(pairPageHtml())
      return
    }

    if (request.method === "POST" && url.pathname === "/api/pair") {
      const body = await readJsonBody(request)
      const token = typeof body?.token === "string" ? body.token.trim() : ""
      const verdict = consumePairingToken(pairingTokens, token, now())
      if (verdict !== "ok") {
        sendJson(response, 401, { ok: false, reason: verdict })
        return
      }
      const grantSecret = `khala_grant_${randomBytes(32).toString("base64url")}`
      const grant: KhalaDeviceGrant = {
        grantId: `grant-${randomBytes(8).toString("hex")}`,
        secretHash: sha256Hex(grantSecret),
        scope: "local-control",
        kind: "opaque-device-grant",
        createdAt: new Date(now()).toISOString(),
      }
      const knownEnvironment: KhalaKnownEnvironment = {
        environmentRef,
        url: baseUrl(),
        pairedAt: grant.createdAt,
        grantId: grant.grantId,
      }
      await persistStore({
        ...store,
        grants: [...store.grants, grant],
        knownEnvironments: [
          ...store.knownEnvironments.filter(entry => entry.environmentRef !== environmentRef),
          knownEnvironment,
        ],
      })
      const result: KhalaPairResult = { ok: true, environmentRef, grantId: grant.grantId, grantSecret }
      sendJson(response, 200, result)
      return
    }

    // Everything below is the pairing-gated control surface. Default-deny:
    // no paired grant, no control.
    if (url.pathname.startsWith("/api/")) {
      const grant = authorizeGrant(request, store)
      if (grant === undefined) {
        sendJson(response, 401, { ok: false, reason: "pairing_required" })
        return
      }
      if (request.method === "GET" && url.pathname === "/api/status") {
        sendJson(response, 200, {
          ok: true,
          environmentRef,
          grantId: grant.grantId,
          knownEnvironments: store.knownEnvironments.length,
          schemaVersion: store.schemaVersion,
        })
        return
      }
      sendJson(response, 404, { ok: false, reason: "not_found" })
      return
    }

    sendJson(response, 404, { ok: false, reason: "not_found" })
  }

  const server: Server = createServer((request, response) => {
    void handler(request, response).catch(() => {
      if (!response.headersSent) sendJson(response, 500, { ok: false, reason: "internal_error" })
      else response.end()
    })
  })

  await new Promise<void>((resolve, reject) => {
    server.once("error", reject)
    // Loopback only, always. The pairing gate is not a substitute for binding
    // discipline: this runtime is never reachable off-machine.
    server.listen({ host: KHALA_LOCAL_HOST, port: options.port ?? 0 }, () => resolve())
  })

  const address = server.address()
  if (address === null || typeof address === "string") {
    server.close()
    throw new Error("Local Khala runtime failed to bind a loopback TCP port.")
  }
  const port = address.port
  function baseUrl(): string {
    return `http://${KHALA_LOCAL_HOST}:${port}`
  }

  return {
    host: address.address,
    port,
    baseUrl: baseUrl(),
    storePath: opened.path,
    storeOutcome: opened.outcome,
    environmentRef,
    mintPairingUrl: () => {
      const token = randomBytes(24).toString("base64url")
      const expiresAtMs = now() + ttlMs
      pairingTokens.push({ tokenHash: sha256(token), expiresAtMs, used: false })
      return { token, url: `${baseUrl()}/pair#token=${token}`, expiresAtMs }
    },
    close: () =>
      new Promise<void>((resolve, reject) => {
        server.close(error => (error ? reject(error) : resolve()))
      }),
  }
}

function consumePairingToken(
  records: Array<PairingTokenRecord>,
  token: string,
  nowMs: number,
): "ok" | "invalid_token" | "expired_token" | "used_token" {
  if (token.length === 0) return "invalid_token"
  const hash = sha256(token)
  const record = records.find(candidate => timingSafeEqual(candidate.tokenHash, hash))
  if (record === undefined) return "invalid_token"
  if (record.used) return "used_token"
  if (nowMs >= record.expiresAtMs) return "expired_token"
  record.used = true
  return "ok"
}

function authorizeGrant(request: IncomingMessage, store: KhalaLocalStore): KhalaDeviceGrant | undefined {
  const header = request.headers.authorization
  if (typeof header !== "string" || !header.startsWith("Bearer ")) return undefined
  const secret = header.slice("Bearer ".length).trim()
  if (secret.length === 0) return undefined
  const presented = Buffer.from(sha256Hex(secret), "utf8")
  return store.grants.find(grant => {
    const stored = Buffer.from(grant.secretHash, "utf8")
    return stored.length === presented.length && timingSafeEqual(stored, presented)
  })
}

async function readJsonBody(request: IncomingMessage): Promise<Record<string, unknown> | undefined> {
  const chunks: Array<Buffer> = []
  let size = 0
  for await (const chunk of request) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)
    size += buffer.length
    if (size > 64 * 1024) return undefined
    chunks.push(buffer)
  }
  try {
    const parsed: unknown = JSON.parse(Buffer.concat(chunks).toString("utf8"))
    return typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : undefined
  } catch {
    return undefined
  }
}

function sendJson(response: ServerResponse, status: number, payload: unknown): void {
  const body = `${JSON.stringify(payload)}\n`
  response.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "cache-control": "no-store",
  })
  response.end(body)
}

function sha256(text: string): Buffer {
  return createHash("sha256").update(text, "utf8").digest()
}

function sha256Hex(text: string): string {
  return createHash("sha256").update(text, "utf8").digest("hex")
}

// The pairing page reads the token from the URL FRAGMENT client-side (the
// fragment is never sent to the server), exchanges it via POST body, scrubs
// the fragment from the address bar, and keeps the returned device grant in
// localStorage for this loopback origin.
function pairPageHtml(): string {
  return `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Pair Khala</title>
<style>
  body { background: #0a0e1a; color: #cdd6f4; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; display: grid; place-items: center; min-height: 100vh; margin: 0; }
  main { max-width: 32rem; padding: 2rem; text-align: center; }
  h1 { color: #4f9cf9; font-size: 1.25rem; }
  code { color: #89b4fa; }
</style>
</head>
<body>
<main>
<h1>Khala local runtime</h1>
<p id="status">Pairing…</p>
</main>
<script>
(async () => {
  const status = document.getElementById("status")
  const token = new URLSearchParams(location.hash.replace(/^#/, "")).get("token")
  history.replaceState(null, "", "/pair")
  if (!token) {
    status.textContent = "No pairing token. Re-run "khala up" in your terminal and open the printed pairing URL."
    return
  }
  try {
    const response = await fetch("/api/pair", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ token }),
    })
    const payload = await response.json()
    if (payload.ok) {
      localStorage.setItem("khala.local.grant", JSON.stringify({ grantId: payload.grantId, grantSecret: payload.grantSecret, environmentRef: payload.environmentRef }))
      status.textContent = "Paired. This browser now holds a device-local grant for " + payload.environmentRef + "."
    } else {
      status.textContent = "Pairing failed (" + payload.reason + "). Re-run "khala up" for a fresh pairing URL."
    }
  } catch {
    status.textContent = "Pairing failed: the local runtime is not reachable."
  }
})()
</script>
</body>
</html>
`
}

export interface KhalaLocalUpOptions {
  readonly env: Record<string, string | undefined>
  readonly port?: number | undefined
  readonly open: boolean
  readonly smoke: boolean
  readonly stdout: (line: string) => void
  readonly openUrl?: ((url: string) => void) | undefined
  readonly pairingTokenTtlMs?: number | undefined
}

// `khala up` entry: init/migrate store → listen on loopback → mint pairing
// token → print fragment pairing URL (→ optionally open browser). With
// --smoke it then proves the whole pairing loop against itself over real
// loopback HTTP and exits (the bounded fresh-machine proof for CI).
export async function runKhalaLocalUp(options: KhalaLocalUpOptions): Promise<number> {
  const server = await startKhalaLocalServer({
    env: options.env,
    port: options.port,
    pairingTokenTtlMs: options.pairingTokenTtlMs,
  })
  const minted = server.mintPairingUrl()

  options.stdout("Khala local runtime ready. No account required.")
  options.stdout(`Store: ${server.storePath} (schema v1, ${server.storeOutcome})`)
  options.stdout(`Listening on ${server.baseUrl} (loopback only)`)
  options.stdout("Authentication required. Open Khala using the pairing URL:")
  options.stdout(`  ${minted.url}`)
  options.stdout("The pairing token is single-use and short-lived; the #fragment never reaches server logs.")

  if (options.open) {
    ;(options.openUrl ?? (() => {}))(minted.url)
  }

  if (options.smoke) {
    const code = await runPairingSmoke(server, minted.token, options.stdout)
    await server.close()
    return code
  }

  options.stdout("Press Ctrl+C to stop.")
  await new Promise<void>(resolve => {
    const stop = () => {
      process.off("SIGINT", stop)
      process.off("SIGTERM", stop)
      resolve()
    }
    process.on("SIGINT", stop)
    process.on("SIGTERM", stop)
  })
  await server.close()
  options.stdout("Khala local runtime stopped.")
  return 0
}

async function runPairingSmoke(
  server: KhalaLocalServer,
  token: string,
  stdout: (line: string) => void,
): Promise<number> {
  const expect = (condition: boolean, label: string): boolean => {
    stdout(`smoke: ${condition ? "ok" : "FAIL"} — ${label}`)
    return condition
  }
  let pass = true

  const page = await fetch(`${server.baseUrl}/pair`)
  pass = expect(page.status === 200, "GET /pair serves the pairing page") && pass

  const queryAttempt = await fetch(`${server.baseUrl}/pair?token=${token}`)
  pass = expect(queryAttempt.status === 400, "token in a query param is REJECTED (fragment-only discipline)") && pass

  const ungated = await fetch(`${server.baseUrl}/api/status`)
  pass = expect(ungated.status === 401, "control surface is pairing-gated by default (401 unauthenticated)") && pass

  const pair = await fetch(`${server.baseUrl}/api/pair`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ token }),
  })
  const paired = (await pair.json()) as { ok?: boolean; grantSecret?: string; environmentRef?: string }
  pass = expect(pair.status === 200 && paired.ok === true, "pairing consumes the fragment token for a device-local grant") && pass

  const replay = await fetch(`${server.baseUrl}/api/pair`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ token }),
  })
  pass = expect(replay.status === 401, "pairing token is single-use (replay rejected)") && pass

  const status = await fetch(`${server.baseUrl}/api/status`, {
    headers: { authorization: `Bearer ${paired.grantSecret ?? ""}` },
  })
  const statusBody = (await status.json()) as { ok?: boolean }
  pass = expect(status.status === 200 && statusBody.ok === true, "paired grant reaches the control surface (usable session)") && pass

  stdout(pass ? "smoke: PASS — init -> migrate/load -> listen -> mint -> pair -> gated control" : "smoke: FAIL")
  return pass ? 0 : 1
}
