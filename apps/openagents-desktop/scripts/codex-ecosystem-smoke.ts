import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createCodexAppServerSupervisor } from "../src/codex-app-server-supervisor.ts"

const binary = process.env.CODEX_BIN
if (!binary) throw new Error("CODEX_BIN must name the exact installed Codex executable")

const object = (value: unknown): Record<string, unknown> | null => typeof value === "object" && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : null
const array = (value: unknown): unknown[] => Array.isArray(value) ? value : []
const argsFor = (schema: unknown): Record<string, unknown> => {
  const row = object(schema); const properties = object(row?.properties) ?? {}; const required = array(row?.required).filter(value => typeof value === "string") as string[]
  return Object.fromEntries(required.map(name => {
    const property = object(properties[name]); const type = property?.type
    return [name, type === "number" || type === "integer" ? 1 : type === "boolean" ? false : type === "array" ? [] : type === "object" ? {} : "Codex app-server"]
  }))
}

const root = mkdtempSync(join(tmpdir(), "oa-codex-ecosystem-"))
const supervisor = createCodexAppServerSupervisor({ strictGeneratedDecoding: true })
try {
  const lease = await supervisor.acquire({ binary, env: process.env, cwd: root, accountRef: "codex-current", hostTarget: "cap08-smoke" })
  const before = object(await lease.request("skills/list", { cwds: [root], forceReload: true }))
  const skillRoot = join(root, "skills")
  const skill = join(skillRoot, "cap08-smoke")
  mkdirSync(skill, { recursive: true })
  writeFileSync(join(skill, "SKILL.md"), "---\nname: cap08-smoke\ndescription: CAP-08 live reconciliation smoke\n---\nReturn CAP08.\n", { mode: 0o600 })
  await lease.request("skills/extraRoots/set", { extraRoots: [skillRoot] })
  const after = object(await lease.request("skills/list", { cwds: [root], forceReload: true }))
  const names = array(after?.data).flatMap(group => array(object(group)?.skills).flatMap(value => typeof object(value)?.name === "string" ? [object(value)!.name] : []))
  if (!names.includes("cap08-smoke")) throw new Error(`live skill reconciliation failed (before groups: ${array(before?.data).length})`)

  const status = object(await lease.request("mcpServerStatus/list", { detail: "full" }))
  const candidates = array(status?.data).flatMap(serverValue => {
    const server = object(serverValue); const serverName = typeof server?.name === "string" ? server.name : null; const tools = object(server?.tools) ?? {}
    return serverName === null ? [] : Object.entries(tools).flatMap(([tool, value]) => /(?:search|list|read|get|status|docs)/iu.test(tool) ? [{ server: serverName, tool, schema: object(value)?.inputSchema }] : [])
  })
  const selected = candidates.find(value => /openai.*docs/iu.test(value.server)) ?? candidates[0]
  if (selected === undefined) throw new Error("no read-only MCP tool is configured for the live smoke")
  const thread = object(object(await lease.request("thread/start", { cwd: root, ephemeral: true, approvalPolicy: "never", sandbox: "read-only" }))?.thread)
  const threadId = typeof thread?.id === "string" ? thread.id : null
  if (threadId === null) throw new Error("ephemeral MCP smoke thread omitted identity")
  const result = object(await lease.request("mcpServer/tool/call", { server: selected.server, tool: selected.tool, threadId, arguments: argsFor(selected.schema) }))
  if (result === null) throw new Error("MCP tool smoke returned no typed result")
  lease.release()
  console.log(`Verified live skill invalidation and a declared read-only MCP tool through the exact packaged app-server (${names.length} reconciled skills).`)
} finally {
  supervisor.close()
  rmSync(root, { recursive: true, force: true })
}
