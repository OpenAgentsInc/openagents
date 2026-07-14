/**
 * MCP config persistence host tests (I2, EP250 wave-2).
 *
 * Behavior contract: openagents_desktop.settings.mcp_servers.v1 — the
 * programmatic (persistence) oracle. Round-trip persist/read, per-entry drop of
 * schema-invalid stored rows, 0600 file mode, schema-bound and
 * reserved/duplicate rejection on add, and the public-safe projection carrying
 * no secret values.
 */
import { describe, expect, test } from "vite-plus/test"
import { mkdirSync, mkdtempSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { openMcpConfigStore, validateMcpServerConfig } from "./mcp-config-host.ts"
import type { FableLocalMcpServerConfig } from "./fable-local-contract.ts"

const tempStore = () => {
  const dir = mkdtempSync(path.join(tmpdir(), "mcp-config-host-"))
  return { dir, file: path.join(dir, "mcp", "servers.json") }
}

const stdio = (over: Partial<FableLocalMcpServerConfig> = {}): FableLocalMcpServerConfig => ({
  name: "docs",
  transport: "stdio",
  enabled: true,
  command: "docs-mcp",
  ...over,
})

describe("openMcpConfigStore round-trip persistence", () => {
  test("add then read returns the same config; servers() carries full secret values (main-only)", () => {
    const { file } = tempStore()
    const store = openMcpConfigStore(file)
    expect(store.list()).toEqual({ state: "ok", dropped: 0, servers: [] })

    const result = store.add(stdio({ args: ["--root", "/x"], env: { TOKEN: "secret-value" } }))
    expect(result.state).toBe("ok")

    // servers() (the runtime getter) keeps the full config including env values.
    const servers = store.servers()
    expect(servers).toHaveLength(1)
    expect(servers[0]).toMatchObject({ name: "docs", command: "docs-mcp", env: { TOKEN: "secret-value" } })

    // A fresh store over the same file reconstructs identical state.
    const reopened = openMcpConfigStore(file)
    expect(reopened.servers()[0]).toMatchObject({ env: { TOKEN: "secret-value" } })
  })

  test("the renderer projection (list) carries NO secret values — counts only", () => {
    const { file } = tempStore()
    const store = openMcpConfigStore(file)
    store.add(stdio({ args: ["--a", "--b"], env: { TOKEN: "secret-value" } }))
    const listed = store.list()
    if (listed.state !== "ok") throw new Error("expected ok")
    const view = listed.servers[0]!
    expect(view).toEqual({
      name: "docs",
      transport: "stdio",
      enabled: true,
      command: "docs-mcp",
      argsCount: 2,
      envCount: 1,
      headersCount: 0,
    })
    // The serialized projection must not contain the secret value anywhere.
    expect(JSON.stringify(listed)).not.toContain("secret-value")
  })

  test("http server round-trips with headers redacted to a count in the projection", () => {
    const { file } = tempStore()
    const store = openMcpConfigStore(file)
    store.add({
      name: "remote",
      transport: "http",
      enabled: true,
      url: "https://example.test/mcp",
      headers: { Authorization: "Bearer secret-token" },
    })
    const listed = store.list()
    if (listed.state !== "ok") throw new Error("expected ok")
    expect(listed.servers[0]).toEqual({
      name: "remote",
      transport: "http",
      enabled: true,
      url: "https://example.test/mcp",
      argsCount: 0,
      envCount: 0,
      headersCount: 1,
    })
    expect(JSON.stringify(listed)).not.toContain("secret-token")
  })
})

describe("file security", () => {
  test("the persisted file is written mode 0600 (owner read/write only)", () => {
    const { file } = tempStore()
    const store = openMcpConfigStore(file)
    store.add(stdio())
    const mode = statSync(file).mode & 0o777
    expect(mode).toBe(0o600)
  })
})

describe("invalid / corrupt file handling (never crash)", () => {
  test("a corrupt / unparseable file reads as empty, not a throw", () => {
    const { file } = tempStore()
    // Seed a garbage file.
    const store = openMcpConfigStore(file)
    store.add(stdio())
    writeFileSync(file, "{ this is not json", "utf8")
    const reopened = openMcpConfigStore(file)
    expect(reopened.list()).toEqual({ state: "ok", dropped: 0, servers: [] })
  })

  test("schema-invalid stored rows are DROPPED and counted; valid rows survive", () => {
    const { file } = tempStore()
    mkdirSync(path.dirname(file), { recursive: true })
    writeFileSync(
      file,
      JSON.stringify({
        version: 1,
        servers: [
          stdio({ name: "good" }),
          { name: "bad", transport: "sse", enabled: true, url: "https://x" }, // invalid transport
          { name: "no-bool", transport: "stdio", enabled: "yes", command: "x" }, // invalid enabled
          stdio({ name: "good" }), // duplicate name
        ],
      }),
      "utf8",
    )
    const store = openMcpConfigStore(file)
    const listed = store.list()
    if (listed.state !== "ok") throw new Error("expected ok")
    expect(listed.servers.map((s) => s.name)).toEqual(["good"])
    expect(listed.dropped).toBe(3)
    // The runtime getter also only sees the one valid row.
    expect(store.servers()).toHaveLength(1)
  })
})

describe("add validation (schema bounds + reserved/duplicate/transport)", () => {
  test("rejects a bad name, the reserved codex name, and duplicates", () => {
    const { file } = tempStore()
    const store = openMcpConfigStore(file)
    expect(store.add(stdio({ name: "bad name!" })).state).toBe("rejected")
    expect(store.add(stdio({ name: "codex" })).state).toBe("rejected")
    expect(store.add(stdio({ name: "dup" })).state).toBe("ok")
    expect(store.add(stdio({ name: "dup" })).state).toBe("rejected")
  })

  test("rejects a stdio server with no command and an http server with a non-http url", () => {
    const { file } = tempStore()
    const store = openMcpConfigStore(file)
    expect(store.add(stdio({ command: "  " })).state).toBe("rejected")
    expect(
      store.add({ name: "remote", transport: "http", enabled: true, url: "ftp://x" }).state,
    ).toBe("rejected")
  })

  test("enforces the 16-server list cap", () => {
    const { file } = tempStore()
    const store = openMcpConfigStore(file)
    for (let index = 0; index < 16; index += 1) {
      expect(store.add(stdio({ name: `s${index}` })).state).toBe("ok")
    }
    const overflow = store.add(stdio({ name: "s16" }))
    expect(overflow.state).toBe("rejected")
  })

  test("validateMcpServerConfig trims the stored name and echoes it back", () => {
    const result = validateMcpServerConfig(stdio({ name: "  trimmed  " }), [])
    expect(result.ok).toBe(true)
    if (result.ok) expect(result.config.name).toBe("trimmed")
  })
})

describe("remove / toggle", () => {
  test("remove drops a known server and rejects an unknown one", () => {
    const { file } = tempStore()
    const store = openMcpConfigStore(file)
    store.add(stdio({ name: "keep" }))
    store.add(stdio({ name: "drop" }))
    expect(store.remove("missing").state).toBe("rejected")
    const after = store.remove("drop")
    if (after.state !== "ok") throw new Error("expected ok")
    expect(after.servers.map((s) => s.name)).toEqual(["keep"])
    expect(store.servers().map((s) => s.name)).toEqual(["keep"])
  })

  test("toggle flips enabled and persists; unknown name is rejected", () => {
    const { file } = tempStore()
    const store = openMcpConfigStore(file)
    store.add(stdio({ name: "t", enabled: true }))
    expect(store.toggle("missing", false).state).toBe("rejected")
    const off = store.toggle("t", false)
    if (off.state !== "ok") throw new Error("expected ok")
    expect(off.servers[0]!.enabled).toBe(false)
    // Persisted: a fresh store sees the disabled state.
    expect(openMcpConfigStore(file).servers()[0]!.enabled).toBe(false)
  })
})

// Coverage linkage marker for openagents_desktop.settings.mcp_servers.v1.
