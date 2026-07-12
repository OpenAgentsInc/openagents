/**
 * User-configured MCP server persistence host (I2, EP250 wave-2 — main only).
 *
 * Reads/writes the user's MCP server list to a private JSON file under the
 * app userData root, and hands the ENABLED-plus-disabled full config list to
 * the fable-local runtime through a main-side getter (`servers()`).
 *
 * Security posture:
 * - The file is written mode 0600 (owner read/write only) via temp+rename,
 *   then re-chmod'd defensively.
 * - `env` / `headers` / `args` VALUES are user-provided and may be sensitive:
 *   they are persisted and returned by `servers()` (main-only) but NEVER
 *   logged and NEVER included in the renderer projection (`list()` / mutation
 *   results carry counts only, via `toMcpConfigServerView`).
 * - Every stored row is re-validated against the FROZEN
 *   `FableLocalMcpServerConfigSchema` on read; invalid rows are DROPPED and
 *   counted (never a throw), so a hand-edited or corrupt file cannot crash the
 *   app or smuggle an out-of-contract server into the runtime.
 */
import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"

import { decode } from "./chat-contract.ts"
import {
  FABLE_LOCAL_MCP_NAME_PATTERN,
  FABLE_LOCAL_MCP_SERVER_LIMIT,
  FableLocalMcpServerConfigSchema,
  type FableLocalMcpServerConfig,
} from "./fable-local-contract.ts"
import {
  mcpReservedServerName,
  toMcpConfigServerView,
  type McpConfigListResult,
  type McpConfigMutationResult,
} from "./mcp-config-contract.ts"

const FILE_VERSION = 1
const OWNER_ONLY = 0o600

const decodeOne = (value: unknown): FableLocalMcpServerConfig | null =>
  decode(FableLocalMcpServerConfigSchema, value) as FableLocalMcpServerConfig | null

/**
 * Add-time validation for a SINGLE entry, regardless of `enabled` (the frozen
 * `normalizeFableLocalMcpServers` only validates enabled entries at turn time;
 * this rejects bad rows before they are ever persisted). Mirrors that
 * normalizer's rules: name charset, the reserved `codex` name, duplicates
 * against the current list, and the transport-specific required field.
 */
export const validateMcpServerConfig = (
  config: FableLocalMcpServerConfig,
  existingNames: ReadonlyArray<string>,
): { readonly ok: true; readonly config: FableLocalMcpServerConfig } | {
  readonly ok: false
  readonly reason: string
} => {
  const name = config.name.trim()
  if (!FABLE_LOCAL_MCP_NAME_PATTERN.test(name)) {
    return { ok: false, reason: "invalid server name (allowed: letters, digits, _ or -, 1-64 chars)" }
  }
  if (name === mcpReservedServerName) {
    return { ok: false, reason: "reserved server name (internal delegate server)" }
  }
  if (existingNames.includes(name)) {
    return { ok: false, reason: "duplicate server name" }
  }
  if (config.transport === "stdio") {
    if ((config.command ?? "").trim() === "") {
      return { ok: false, reason: "stdio transport requires a command" }
    }
  } else {
    if (!/^https?:\/\//i.test((config.url ?? "").trim())) {
      return { ok: false, reason: "http transport requires an http(s) url" }
    }
  }
  return { ok: true, config: { ...config, name } }
}

export type McpConfigStore = Readonly<{
  /** MAIN-ONLY full config list (incl. secret values) — the runtime getter. */
  servers: () => ReadonlyArray<FableLocalMcpServerConfig>
  /** Public-safe projection for the renderer (no secret values). */
  list: () => McpConfigListResult
  add: (config: FableLocalMcpServerConfig) => McpConfigMutationResult
  remove: (name: string) => McpConfigMutationResult
  toggle: (name: string, enabled: boolean) => McpConfigMutationResult
}>

/**
 * Open (or lazily create) the MCP config store at `filePath`. Reads are cheap
 * and always fresh so config edits take effect on the next runtime turn
 * without a restart.
 */
export const openMcpConfigStore = (filePath: string): McpConfigStore => {
  /** Parse + per-entry validate. Returns valid rows and the dropped count. */
  const read = (): { servers: FableLocalMcpServerConfig[]; dropped: number } => {
    let raw: unknown
    try {
      raw = JSON.parse(readFileSync(filePath, "utf8"))
    } catch {
      // Missing or unparseable file → empty list (never a throw).
      return { servers: [], dropped: 0 }
    }
    const rows =
      typeof raw === "object" && raw !== null && Array.isArray((raw as { servers?: unknown }).servers)
        ? ((raw as { servers: unknown[] }).servers)
        : []
    const servers: FableLocalMcpServerConfig[] = []
    let dropped = 0
    const seen = new Set<string>()
    for (const row of rows) {
      const decoded = decodeOne(row)
      // Drop invalid, over-cap, and duplicate-name rows; never crash.
      if (decoded === null || servers.length >= FABLE_LOCAL_MCP_SERVER_LIMIT) {
        dropped += 1
        continue
      }
      const name = decoded.name.trim()
      if (seen.has(name)) {
        dropped += 1
        continue
      }
      seen.add(name)
      servers.push(decoded)
    }
    return { servers, dropped }
  }

  const write = (servers: ReadonlyArray<FableLocalMcpServerConfig>): void => {
    const bounded = servers.slice(0, FABLE_LOCAL_MCP_SERVER_LIMIT)
    mkdirSync(path.dirname(filePath), { recursive: true })
    const temporary = `${filePath}.tmp`
    // Never log the payload — env/headers/args values may be sensitive.
    writeFileSync(temporary, JSON.stringify({ version: FILE_VERSION, servers: bounded }), {
      encoding: "utf8",
      mode: OWNER_ONLY,
    })
    renameSync(temporary, filePath)
    try {
      chmodSync(filePath, OWNER_ONLY)
    } catch {
      // Best-effort: rename preserved the temp file's 0600 mode already.
    }
  }

  const okResult = (
    servers: ReadonlyArray<FableLocalMcpServerConfig>,
    dropped: number,
  ): McpConfigMutationResult => ({
    state: "ok",
    dropped,
    servers: servers.map(toMcpConfigServerView),
  })

  return {
    servers: () => read().servers,
    list: () => {
      const { servers, dropped } = read()
      return { state: "ok", dropped, servers: servers.map(toMcpConfigServerView) }
    },
    add: (config) => {
      const { servers, dropped } = read()
      if (servers.length >= FABLE_LOCAL_MCP_SERVER_LIMIT) {
        return { state: "rejected", reason: `at most ${FABLE_LOCAL_MCP_SERVER_LIMIT} servers` }
      }
      const validated = validateMcpServerConfig(
        config,
        servers.map((server) => server.name),
      )
      if (!validated.ok) return { state: "rejected", reason: validated.reason }
      const next = [...servers, validated.config]
      write(next)
      return okResult(next, dropped)
    },
    remove: (name) => {
      const { servers, dropped } = read()
      const target = name.trim()
      const next = servers.filter((server) => server.name !== target)
      if (next.length === servers.length) {
        return { state: "rejected", reason: "unknown server name" }
      }
      write(next)
      return okResult(next, dropped)
    },
    toggle: (name, enabled) => {
      const { servers, dropped } = read()
      const target = name.trim()
      if (!servers.some((server) => server.name === target)) {
        return { state: "rejected", reason: "unknown server name" }
      }
      const next = servers.map((server) =>
        server.name === target ? { ...server, enabled } : server,
      )
      write(next)
      return okResult(next, dropped)
    },
  }
}
