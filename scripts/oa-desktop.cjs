#!/usr/bin/env node
/**
 * Firm reason: 6 (OpenAgents Cloud-to-Desktop & Desktop-to-Desktop CLI helper `oa-desktop` for NIP-HOST + NIP-REACH + NIP-TERM, #14152).
 *
 * Works seamlessly in TWO environments:
 *   1. Cloud Run Sandbox (`OA_DESKTOP_AUTH_FILE` set):
 *      Talks strictly to the per-session loopback proxy inside `bridge-server.ts`
 *      using the session-scoped `0600` credential file (never sees `OA_SANDBOX_BRIDGE_TOKEN`).
 *   2. Direct Mac/Linux Desktop or CLI (`OA_DESKTOP_TOKEN` or `~/.openagents/desktop-bridge/bridge.env`):
 *      Talks directly to `https://openagents.com` (or `OA_DESKTOP_ORIGIN`) using the user's
 *      signed Bearer token (`GET /api/desktop/hosts` and `POST /api/internal/desktop/exec`).
 *
 * Usage:
 *   oa-desktop hosts [--json]
 *   oa-desktop exec [--host LABEL_OR_ID] [--grant ID] [--epoch N] [--cwd PATH] [--timeout MS] [--json] -- <command>
 */
const fs = require("node:fs")
const os = require("node:os")
const path = require("node:path")

const parseEnvFile = (filePath) => {
  const out = {}
  try {
    const raw = fs.readFileSync(filePath, "utf8")
    for (const line of raw.split("\n")) {
      const t = line.trim()
      if (!t || t.startsWith("#")) continue
      const eq = t.indexOf("=")
      if (eq <= 0) continue
      const k = t.slice(0, eq).trim()
      let v = t.slice(eq + 1).trim()
      if ((v.startsWith('"') && v.endsWith('"')) || (v.startsWith("'") && v.endsWith("'"))) {
        v = v.slice(1, -1)
      }
      out[k] = v
    }
  } catch {
    // ignore missing/unreadable file
  }
  return out
}

const readTransport = () => {
  const authPath = (process.env.OA_DESKTOP_AUTH_FILE || "").trim()
  if (authPath) {
    const raw = JSON.parse(fs.readFileSync(authPath, "utf8"))
    if (!raw || typeof raw.baseUrl !== "string" || typeof raw.secret !== "string") {
      throw new Error("Invalid OA_DESKTOP_AUTH_FILE payload.")
    }
    return {
      mode: "sandbox",
      baseUrl: raw.baseUrl.replace(/\/+$/, ""),
      headers: { "x-oa-session-secret": raw.secret },
      hostsPath: "/local/desktop/hosts",
      execPath: "/local/desktop/exec"
    }
  }

  const envFile = path.join(os.homedir(), ".openagents", "desktop-bridge", "bridge.env")
  const fileVars = parseEnvFile(envFile)
  const token = (process.env.OA_DESKTOP_TOKEN || fileVars.OA_DESKTOP_TOKEN || "").trim()
  const origin = (
    process.env.OA_DESKTOP_ORIGIN ||
    fileVars.OA_DESKTOP_ORIGIN ||
    "https://openagents.com"
  )
    .trim()
    .replace(/\/+$/, "")

  if (!token) {
    throw new Error(
      "No OpenAgents desktop credentials found. Set OA_DESKTOP_TOKEN or create ~/.openagents/desktop-bridge/bridge.env (or run inside an OpenAgents Cloud Sandbox)."
    )
  }

  return {
    mode: "direct",
    baseUrl: origin,
    headers: { authorization: `Bearer ${token}` },
    hostsPath: "/api/desktop/hosts",
    execPath: "/api/internal/desktop/exec"
  }
}

const requestJson = async (transport, method, pathname, body) => {
  const res = await fetch(`${transport.baseUrl}${pathname}`, {
    method,
    headers: {
      accept: "application/json",
      ...transport.headers,
      ...(body !== undefined ? { "content-type": "application/json" } : {})
    },
    ...(body !== undefined ? { body: JSON.stringify(body) } : {})
  })
  const text = await res.text()
  let parsed = null
  try {
    parsed = JSON.parse(text)
  } catch {
    parsed = { error: text || `HTTP ${res.status}` }
  }
  return { status: res.status, ok: res.ok, body: parsed }
}

const usage = () => {
  process.stderr.write(
    "Usage:\n" +
      "  oa-desktop hosts [--json]\n" +
      "  oa-desktop exec [--host LABEL_OR_ID] [--grant ID] [--epoch N] [--cwd PATH] [--timeout MS] [--json] -- <command>\n"
  )
}

const main = async () => {
  const argv = process.argv.slice(2)
  const sub = argv[0]
  if (!sub || sub === "-h" || sub === "--help") {
    usage()
    process.exit(0)
  }

  const transport = readTransport()

  if (sub === "hosts") {
    const asJson = argv.includes("--json")
    const { ok, status, body } = await requestJson(transport, "GET", transport.hostsPath)
    if (!ok) {
      process.stderr.write(`oa-desktop hosts failed (${status}): ${body.error || JSON.stringify(body)}\n`)
      process.exit(1)
    }
    if (asJson) {
      process.stdout.write(`${JSON.stringify(body, null, 2)}\n`)
      return
    }
    const hosts = Array.isArray(body.hosts) ? body.hosts : []
    if (hosts.length === 0) {
      process.stdout.write("No connected desktop hosts found for this account.\n")
      return
    }
    for (const h of hosts) {
      const state = h.online ? "ONLINE" : "offline"
      const roots = Array.isArray(h.workspaceRoots) ? h.workspaceRoots.join(", ") : ""
      const grants = Array.isArray(h.activeGrants) ? h.activeGrants : []
      const grantSummary =
        grants.length > 0
          ? grants.map((g) => `${g.id} [${(g.rights || []).join(",")}]`).join("; ")
          : "no active grant"
      process.stdout.write(
        `${state}  host=${h.id}  label="${h.label || h.hostname}"  platform=${h.platform}/${h.arch}  epoch=${h.epoch}\n` +
          `  workspaces: ${roots}\n` +
          `  grants:     ${grantSummary}\n`
      )
    }
    return
  }

  if (sub === "exec") {
    let hostId = undefined
    let grantId = undefined
    let epoch = undefined
    let cwd = undefined
    let timeoutMs = undefined
    let asJson = false
    const cmdParts = []
    let afterDashDash = false

    for (let i = 1; i < argv.length; i++) {
      const tok = argv[i]
      if (afterDashDash) {
        cmdParts.push(tok)
        continue
      }
      if (tok === "--") {
        afterDashDash = true
        continue
      }
      if (tok === "--json") {
        asJson = true
        continue
      }
      if (tok === "--host" && i + 1 < argv.length) {
        hostId = argv[++i]
        continue
      }
      if (tok === "--grant" && i + 1 < argv.length) {
        grantId = argv[++i]
        continue
      }
      if (tok === "--epoch" && i + 1 < argv.length) {
        epoch = Number(argv[++i])
        continue
      }
      if (tok === "--cwd" && i + 1 < argv.length) {
        cwd = argv[++i]
        continue
      }
      if (tok === "--timeout" && i + 1 < argv.length) {
        timeoutMs = Number(argv[++i])
        continue
      }
      cmdParts.push(tok)
    }

    const command = cmdParts.join(" ").trim()
    if (!command) {
      process.stderr.write("oa-desktop exec requires a command.\n")
      usage()
      process.exit(2)
    }

    const payload = {
      command,
      ...(hostId ? { hostId } : {}),
      ...(grantId ? { grantId } : {}),
      ...(Number.isFinite(epoch) ? { epoch } : {}),
      ...(cwd ? { cwd } : {}),
      ...(Number.isFinite(timeoutMs) ? { timeoutMs } : {})
    }

    const { ok, status, body } = await requestJson(transport, "POST", transport.execPath, payload)
    if (asJson) {
      process.stdout.write(`${JSON.stringify(body, null, 2)}\n`)
      process.exit(ok && body.exitCode === 0 ? 0 : (typeof body.exitCode === "number" ? body.exitCode : 1))
    }
    if (!ok) {
      const code = body.code ? ` [${body.code}]` : ""
      process.stderr.write(`oa-desktop exec refused (${status})${code}: ${body.error || JSON.stringify(body)}\n`)
      process.exit(1)
    }
    if (body.stdout) process.stdout.write(body.stdout)
    if (body.stderr) process.stderr.write(body.stderr)
    if (body.truncated) {
      process.stderr.write("\n[oa-desktop: output truncated at 256 KiB]\n")
    }
    process.exit(typeof body.exitCode === "number" ? body.exitCode : (body.status === "completed" ? 0 : 1))
  }

  process.stderr.write(`Unknown subcommand: ${sub}\n`)
  usage()
  process.exit(2)
}

main().catch((err) => {
  process.stderr.write(`oa-desktop error: ${err instanceof Error ? err.message : String(err)}\n`)
  process.exit(1)
})
