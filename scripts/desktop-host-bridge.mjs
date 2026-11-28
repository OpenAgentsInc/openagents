#!/usr/bin/env node
/**
 * Firm reason: 6 (Desktop-to-Cloud outbound host bridge daemon for NIP-HOST + NIP-REACH + NIP-TERM, #14152).
 *
 * Connects this computer outbound over HTTPS to `openagents.com` so Cloud Sandbox
 * agents (`oa-worker` via `oa-desktop`) can run scoped commands inside admitted
 * NIP-WS workspace roots with audit logging and instant revocation.
 *
 * Usage:
 *   node scripts/desktop-host-bridge.mjs \
 *     --origin https://openagents.com \
 *     --token "$OA_DESKTOP_TOKEN" \
 *     --workspace "$HOME/work/openagents" \
 *     --workspace "$HOME/work/openagents.com"
 *
 * Environment fallbacks:
 *   OA_DESKTOP_ORIGIN     (default https://openagents.com)
 *   OA_DESKTOP_TOKEN      (signed Bearer token from POST /api/desktop/token)
 *   OA_DESKTOP_WORKSPACES (comma-separated absolute workspace roots)
 *   OA_DESKTOP_HOST_ID    (optional stable host ID; defaults to ~/.openagents/desktop-bridge/host-id)
 *   OA_DESKTOP_LABEL      (optional display label; defaults to os.hostname())
 */
import { spawn } from "node:child_process"
import { createHash, randomBytes } from "node:crypto"
import fs from "node:fs"
import os from "node:os"
import path from "node:path"

const MAX_OUTPUT_BYTES = 262_144
const DEFAULT_POLL_WAIT_MS = 15_000
const DEFAULT_GRANT_TTL_SEC = 7 * 24 * 60 * 60

const parseArgs = (argv) => {
  const opts = {
    origin: (process.env.OA_DESKTOP_ORIGIN || "https://openagents.com").trim().replace(/\/+$/, ""),
    token: (process.env.OA_DESKTOP_TOKEN || "").trim(),
    hostId: (process.env.OA_DESKTOP_HOST_ID || "").trim(),
    label: (process.env.OA_DESKTOP_LABEL || os.hostname()).trim(),
    workspaces: [],
    ttlSeconds: DEFAULT_GRANT_TTL_SEC,
    pollWaitMs: DEFAULT_POLL_WAIT_MS,
    stateFile: "",
    once: false
  }
  if (process.env.OA_DESKTOP_WORKSPACES) {
    for (const part of process.env.OA_DESKTOP_WORKSPACES.split(",")) {
      if (part.trim()) opts.workspaces.push(part.trim())
    }
  }
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if ((a === "--origin" || a === "-o") && i + 1 < argv.length) {
      opts.origin = argv[++i].trim().replace(/\/+$/, "")
    } else if ((a === "--token" || a === "-t") && i + 1 < argv.length) {
      opts.token = argv[++i].trim()
    } else if ((a === "--workspace" || a === "-w") && i + 1 < argv.length) {
      for (const part of argv[++i].split(",")) {
        if (part.trim()) opts.workspaces.push(part.trim())
      }
    } else if (a === "--host-id" && i + 1 < argv.length) {
      opts.hostId = argv[++i].trim()
    } else if (a === "--label" && i + 1 < argv.length) {
      opts.label = argv[++i].trim()
    } else if (a === "--ttl" && i + 1 < argv.length) {
      opts.ttlSeconds = Number(argv[++i]) || DEFAULT_GRANT_TTL_SEC
    } else if (a === "--poll-ms" && i + 1 < argv.length) {
      opts.pollWaitMs = Math.max(500, Math.min(20_000, Number(argv[++i]) || DEFAULT_POLL_WAIT_MS))
    } else if (a === "--state-file" && i + 1 < argv.length) {
      opts.stateFile = argv[++i].trim()
    } else if (a === "--once") {
      opts.once = true
    } else if (a === "-h" || a === "--help") {
      console.log(
        "Usage: node scripts/desktop-host-bridge.mjs --origin URL --token TOKEN --workspace DIR [--workspace DIR2] [--label NAME] [--state-file PATH]"
      )
      process.exit(0)
    }
  }
  if (opts.workspaces.length === 0) {
    opts.workspaces.push(process.cwd())
  }
  return opts
}

const resolveHostId = (explicit) => {
  if (explicit) return explicit
  const home = os.homedir()
  const coderKeyPath = path.join(home, ".openagents", "coder", "host", "key")
  try {
    const keyHex = fs.readFileSync(coderKeyPath, "utf8").trim()
    if (/^[0-9a-f]{64}$/i.test(keyHex)) {
      // Derive a stable public host identifier without ever transmitting the secret key.
      return createHash("sha256").update(`openagents.desktop.host.v1:${keyHex}`).digest("hex")
    }
  } catch {
    // fall through to dedicated desktop-bridge host-id
  }
  const bridgeDir = path.join(home, ".openagents", "desktop-bridge")
  const idPath = path.join(bridgeDir, "host-id")
  try {
    const existing = fs.readFileSync(idPath, "utf8").trim()
    if (/^[a-zA-Z0-9._-]{6,128}$/.test(existing)) return existing
  } catch {
    // create fresh stable host ID
  }
  fs.mkdirSync(bridgeDir, { recursive: true, mode: 0o700 })
  const fresh = randomBytes(32).toString("hex")
  fs.writeFileSync(idPath, `${fresh}\n`, { mode: 0o600 })
  return fresh
}

const resolveRealWorkspaces = (inputs) => {
  const seen = new Set()
  const out = []
  for (const raw of inputs) {
    const abs = path.resolve(raw)
    const real = fs.realpathSync(abs)
    const stat = fs.statSync(real)
    if (!stat.isDirectory()) {
      throw new Error(`Workspace root is not a directory: ${real}`)
    }
    if (!seen.has(real)) {
      seen.add(real)
      out.push(real)
    }
  }
  return out
}

const isRealPathAllowed = (candidate, realRoots) => {
  try {
    const realTarget = fs.realpathSync(path.resolve(candidate))
    for (const root of realRoots) {
      if (realTarget === root || realTarget.startsWith(`${root}${path.sep}`)) {
        return { ok: true, realTarget }
      }
    }
    return { ok: false, realTarget }
  } catch {
    return { ok: false, realTarget: "" }
  }
}

const apiJson = async (opts, method, pathname, body) => {
  const headers = {
    accept: "application/json",
    ...(opts.token ? { authorization: `Bearer ${opts.token}` } : {}),
    ...(body !== undefined ? { "content-type": "application/json" } : {})
  }
  const res = await fetch(`${opts.origin}${pathname}`, {
    method,
    headers,
    ...(body !== undefined ? { body: JSON.stringify(body) } : {})
  })
  const text = await res.text()
  let parsed = null
  try {
    parsed = JSON.parse(text)
  } catch {
    parsed = { error: text || `HTTP ${res.status}` }
  }
  return { ok: res.ok, status: res.status, body: parsed }
}

const runBoundedCommand = (command, cwd, timeoutMs) =>
  new Promise((resolve) => {
    const startedAt = Date.now()
    const isWin = process.platform === "win32"
    const shell = isWin ? (process.env.COMSPEC || "cmd.exe") : "/bin/sh"
    const args = isWin ? ["/d", "/s", "/c", command] : ["-lc", command]

    let stdoutBytes = 0
    let stderrBytes = 0
    const stdoutChunks = []
    const stderrChunks = []
    let truncated = false
    let timedOut = false

    const child = spawn(shell, args, {
      cwd,
      detached: !isWin,
      stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...process.env,
        SSH_CONNECTION: "1"
      }
    })

    const killTree = (sig) => {
      try {
        if (!isWin && child.pid) process.kill(-child.pid, sig)
        else child.kill(sig)
      } catch {
        // already exited
      }
    }

    const timer = setTimeout(() => {
      timedOut = true
      killTree("SIGTERM")
      setTimeout(() => killTree("SIGKILL"), 2000).unref()
    }, timeoutMs)

    child.stdout.on("data", (chunk) => {
      const buf = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)
      if (stdoutBytes + buf.length <= MAX_OUTPUT_BYTES) {
        stdoutChunks.push(buf)
        stdoutBytes += buf.length
      } else {
        const remain = MAX_OUTPUT_BYTES - stdoutBytes
        if (remain > 0) {
          stdoutChunks.push(buf.subarray(0, remain))
          stdoutBytes += remain
        }
        truncated = true
      }
    })

    child.stderr.on("data", (chunk) => {
      const buf = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk)
      if (stderrBytes + buf.length <= MAX_OUTPUT_BYTES) {
        stderrChunks.push(buf)
        stderrBytes += buf.length
      } else {
        const remain = MAX_OUTPUT_BYTES - stderrBytes
        if (remain > 0) {
          stderrChunks.push(buf.subarray(0, remain))
          stderrBytes += remain
        }
        truncated = true
      }
    })

    child.on("error", (err) => {
      clearTimeout(timer)
      resolve({
        status: "failed",
        exitCode: null,
        signal: null,
        stdout: Buffer.concat(stdoutChunks).toString("utf8"),
        stderr: `${Buffer.concat(stderrChunks).toString("utf8")}\n${err.message}`.trim(),
        truncated,
        durationMs: Date.now() - startedAt,
        error: err.message
      })
    })

    child.on("close", (code, signal) => {
      clearTimeout(timer)
      const stdout = Buffer.concat(stdoutChunks).toString("utf8")
      const stderr = Buffer.concat(stderrChunks).toString("utf8")
      if (timedOut) {
        resolve({
          status: "failed",
          exitCode: code,
          signal: signal || "SIGTERM",
          stdout,
          stderr: `${stderr}\n[desktop-host-bridge: command timed out after ${timeoutMs}ms]`.trim(),
          truncated,
          durationMs: Date.now() - startedAt,
          error: `Command timed out after ${timeoutMs}ms`
        })
        return
      }
      resolve({
        status: "completed",
        exitCode: code,
        signal: signal || null,
        stdout,
        stderr,
        truncated,
        durationMs: Date.now() - startedAt
      })
    })
  })

const main = async () => {
  const opts = parseArgs(process.argv.slice(2))
  const hostId = resolveHostId(opts.hostId)
  const realRoots = resolveRealWorkspaces(opts.workspaces)
  const generation = Math.floor(Date.now() / 1000)

  const reg = await apiJson(opts, "POST", "/api/desktop/hosts/register", {
    hostId,
    label: opts.label,
    hostname: os.hostname(),
    platform: process.platform,
    arch: process.arch,
    generation,
    workspaceRoots: realRoots,
    capabilities: ["terminal"]
  })
  if (!reg.ok || !reg.body?.host) {
    console.error(`[desktop-bridge] registration failed (${reg.status}):`, reg.body)
    process.exit(1)
  }

  const grantRes = await apiJson(opts, "POST", `/api/desktop/hosts/${encodeURIComponent(hostId)}/grants`, {
    rights: ["terminal"],
    workspaceRoots: realRoots,
    ttlSeconds: opts.ttlSeconds
  })
  if (!grantRes.ok || !grantRes.body?.grant) {
    console.error(`[desktop-bridge] grant issuance failed (${grantRes.status}):`, grantRes.body)
    process.exit(1)
  }

  const activeGrant = grantRes.body.grant
  let currentEpoch = activeGrant.epoch
  let stopping = false

  if (opts.stateFile) {
    fs.writeFileSync(
      opts.stateFile,
      JSON.stringify(
        {
          hostId,
          grantId: activeGrant.id,
          epoch: currentEpoch,
          origin: opts.origin,
          workspaceRoots: realRoots,
          startedAtMs: Date.now()
        },
        null,
        2
      ),
      { mode: 0o600 }
    )
  }

  console.log(
    `[desktop-bridge] connected origin=${opts.origin} host=${hostId.slice(0, 16)}… grant=${activeGrant.id} epoch=${currentEpoch}`
  )
  console.log(`[desktop-bridge] admitted NIP-WS roots: ${realRoots.join(", ")}`)

  const shutdown = async (sig) => {
    if (stopping) return
    stopping = true
    console.log(`\n[desktop-bridge] ${sig} received — revoking NIP-HOST grant and bumping epoch…`)
    try {
      await apiJson(opts, "POST", `/api/desktop/hosts/${encodeURIComponent(hostId)}/revoke`, {
        reason: `desktop-bridge shutdown (${sig})`
      })
    } catch {
      // best-effort revoke on exit
    }
    process.exit(0)
  }
  process.on("SIGINT", () => void shutdown("SIGINT"))
  process.on("SIGTERM", () => void shutdown("SIGTERM"))

  let backoffMs = 1000
  while (!stopping) {
    try {
      const poll = await apiJson(
        opts,
        "GET",
        `/api/desktop/hosts/${encodeURIComponent(hostId)}/poll?waitMs=${opts.pollWaitMs}`
      )
      if (!poll.ok) {
        console.error(`[desktop-bridge] poll failed (${poll.status}):`, poll.body)
        await new Promise((r) => setTimeout(r, backoffMs))
        backoffMs = Math.min(15_000, backoffMs * 2)
        continue
      }
      backoffMs = 1000

      if (typeof poll.body?.epoch === "number" && poll.body.epoch > currentEpoch) {
        console.log(
          `[desktop-bridge] host epoch bumped remotely (${currentEpoch} -> ${poll.body.epoch}); local grant ${activeGrant.id} revoked.`
        )
        currentEpoch = poll.body.epoch
      }

      const req = poll.body?.request
      if (!req) {
        if (opts.once) break
        continue
      }

      // Independent desktop-side NIP-HOST + NIP-WS verification before executing anything
      if (req.grantId !== activeGrant.id || req.epoch !== activeGrant.epoch || currentEpoch !== activeGrant.epoch) {
        await apiJson(
          opts,
          "POST",
          `/api/desktop/hosts/${encodeURIComponent(hostId)}/results/${encodeURIComponent(req.requestId)}`,
          {
            status: "refused",
            refusalCode: currentEpoch !== activeGrant.epoch ? "revoked" : "stale_epoch",
            error: "Desktop host refused request: grant or epoch mismatch."
          }
        )
        continue
      }

      const pathCheck = isRealPathAllowed(req.cwd, realRoots)
      if (!pathCheck.ok) {
        console.warn(`[desktop-bridge] REFUSED outside_workspace req=${req.requestId} cwd=${req.cwd}`)
        await apiJson(
          opts,
          "POST",
          `/api/desktop/hosts/${encodeURIComponent(hostId)}/results/${encodeURIComponent(req.requestId)}`,
          {
            status: "refused",
            refusalCode: "outside_workspace",
            error: `Desktop host refused cwd outside admitted NIP-WS workspace roots: ${req.cwd}`
          }
        )
        continue
      }

      console.log(`[desktop-bridge] EXEC req=${req.requestId} cwd=${pathCheck.realTarget} cmd=${req.command.slice(0, 120)}`)
      const result = await runBoundedCommand(req.command, pathCheck.realTarget, req.timeoutMs || 60_000)
      console.log(
        `[desktop-bridge] DONE req=${req.requestId} status=${result.status} exit=${String(result.exitCode)} duration=${result.durationMs}ms`
      )
      await apiJson(
        opts,
        "POST",
        `/api/desktop/hosts/${encodeURIComponent(hostId)}/results/${encodeURIComponent(req.requestId)}`,
        result
      )
      if (opts.once) break
    } catch (err) {
      if (stopping) break
      console.error(`[desktop-bridge] poll loop error:`, err instanceof Error ? err.message : String(err))
      await new Promise((r) => setTimeout(r, backoffMs))
      backoffMs = Math.min(15_000, backoffMs * 2)
    }
  }
}

main().catch((err) => {
  console.error("[desktop-bridge] fatal:", err)
  process.exit(1)
})
