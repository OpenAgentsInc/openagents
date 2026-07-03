import { describe, expect, test } from "bun:test"
import { existsSync } from "node:fs"
import { readFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { fileURLToPath } from "node:url"

import {
  inspectCodexReferenceRoot,
  KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF,
} from "../src/bun/codex-parity-contract"
import {
  KHALA_CODE_DESKTOP_SLASH_COMMANDS,
  evaluateKhalaCodeDesktopSlashCommandAvailability,
  findKhalaCodeDesktopSlashCommand,
  khalaCodeDesktopSlashCommandDispatchCoverage,
  khalaCodeDesktopSlashCommands,
  parseKhalaCodeDesktopSlashCommand,
} from "../src/shared/codex-slash-commands"

type ParsedCodexSlashCommand = {
  readonly aliases: readonly string[]
  readonly command: string
  readonly enumName: string
}

type CodexSlashCommandSourceStatus =
  | {
      readonly ok: true
      readonly path: string
      readonly status: "ready"
    }
  | {
      readonly blockerRef: typeof KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF
      readonly ok: false
      readonly reason: string
      readonly status: "blocked"
    }

const kebabCase = (value: string): string =>
  value.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase()

const codexSlashCommandSourceBlocked = (reason: string): CodexSlashCommandSourceStatus => ({
  blockerRef: KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF,
  ok: false,
  reason,
  status: "blocked",
})

const inspectCodexSlashCommandSourcePath = (): CodexSlashCommandSourceStatus => {
  const explicit = process.env.KHALA_CODE_CODEX_SLASH_COMMAND_SOURCE?.trim()
  if (explicit !== undefined && explicit.length > 0) {
    if (existsSync(explicit)) {
      return {
        ok: true,
        path: explicit,
        status: "ready",
      }
    }
    return codexSlashCommandSourceBlocked(
      `KHALA_CODE_CODEX_SLASH_COMMAND_SOURCE does not exist: ${explicit}`,
    )
  }

  const reference = inspectCodexReferenceRoot(dirname(fileURLToPath(import.meta.url)))
  if (!reference.ok) return reference

  const candidate = join(reference.root, "codex-rs/tui/src/slash_command.rs")
  if (existsSync(candidate)) {
    return {
      ok: true,
      path: candidate,
      status: "ready",
    }
  }
  return codexSlashCommandSourceBlocked(
    `Codex reference checkout is missing codex-rs/tui/src/slash_command.rs: ${candidate}`,
  )
}

const expectCodexSlashCommandSourcePathOrBlocker = (): string | null => {
  const status = inspectCodexSlashCommandSourcePath()
  if (status.ok) return status.path

  expect(status).toMatchObject({
    blockerRef: KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF,
    ok: false,
    status: "blocked",
  })
  expect(status.reason.length).toBeGreaterThan(0)
  return null
}

const parseCodexSlashCommands = (source: string): readonly ParsedCodexSlashCommand[] => {
  const enumBody = source.match(/pub enum SlashCommand \{([\s\S]*?)\n\}/)?.[1]
  if (enumBody === undefined) throw new Error("Could not parse SlashCommand enum body")

  const commands: ParsedCodexSlashCommand[] = []
  let pendingAttrs: string[] = []
  for (const line of enumBody.split("\n")) {
    const trimmed = line.trim()
    if (trimmed.startsWith("#[strum(")) {
      pendingAttrs = [...pendingAttrs, trimmed]
      continue
    }
    const variant = trimmed.match(/^([A-Za-z0-9_]+),/)
    if (variant === null) continue
    const enumName = variant[1]!
    const attr = pendingAttrs.join(" ")
    const toStrings = [...attr.matchAll(/to_string\s*=\s*"([^"]+)"/g)]
      .map(match => match[1]!)
    const serializes = [...attr.matchAll(/serialize\s*=\s*"([^"]+)"/g)]
      .map(match => match[1]!)
    const command = toStrings[0] ?? serializes[0] ?? kebabCase(enumName)
    commands.push({
      aliases: serializes.filter(alias => alias !== command),
      command,
      enumName,
    })
    pendingAttrs = []
  }
  return commands
}

describe("Khala Code Codex slash command registry", () => {
  test("tracks the upstream Codex SlashCommand enum and aliases", async () => {
    const sourcePath = expectCodexSlashCommandSourcePathOrBlocker()
    if (sourcePath === null) return

    const source = await readFile(sourcePath, "utf8")
    const upstream = parseCodexSlashCommands(source)
    const khala = KHALA_CODE_DESKTOP_SLASH_COMMANDS.map(command => ({
      aliases: command.aliases,
      command: command.command,
      enumName: command.enumName,
    }))

    expect([...khala]).toEqual([...upstream])
  })

  test("preserves aliases and inline-argument support", () => {
    expect(parseKhalaCodeDesktopSlashCommand("/clean")).toMatchObject({
      command: { command: "stop", enumName: "Stop" },
      rawCommand: "clean",
    })
    expect(parseKhalaCodeDesktopSlashCommand("/pet floof")).toMatchObject({
      args: "floof",
      command: { command: "pets", enumName: "Pets" },
      rawCommand: "pet",
    })

    const inlineCommands = KHALA_CODE_DESKTOP_SLASH_COMMANDS
      .filter(command => command.supportsInlineArgs)
      .map(command => command.command)
    expect(inlineCommands).toEqual([
      "ide",
      "keymap",
      "sandbox-add-read-dir",
      "review",
      "rename",
      "resume",
      "architect",
      "plan",
      "goal",
      "side",
      "btw",
      "raw",
      "usage",
      "pets",
      "mcp",
    ])
  })

  test("maps background terminal slash commands to experimental app-server methods", () => {
    expect(findKhalaCodeDesktopSlashCommand("/ps")?.dispatch).toMatchObject({
      kind: "app_server",
      method: "thread/backgroundTerminals/list",
      experimental: true,
      requiresThread: true,
    })
    expect(findKhalaCodeDesktopSlashCommand("/stop")?.dispatch).toMatchObject({
      kind: "app_server",
      method: "thread/backgroundTerminals/clean",
      experimental: true,
      requiresThread: true,
    })
    expect(findKhalaCodeDesktopSlashCommand("/clean")?.dispatch).toMatchObject({
      kind: "app_server",
      method: "thread/backgroundTerminals/clean",
      experimental: true,
      requiresThread: true,
    })
  })

  test("matches Codex platform/debug visibility gates", () => {
    const linuxCommands = khalaCodeDesktopSlashCommands({ platform: "linux" })
      .map(command => command.command)
    expect(linuxCommands).not.toContain("app")
    expect(linuxCommands).not.toContain("sandbox-add-read-dir")
    expect(linuxCommands).not.toContain("rollout")
    expect(linuxCommands).not.toContain("test-approval")
    expect(linuxCommands).toContain("copy")

    const windowsCommands = khalaCodeDesktopSlashCommands({ platform: "win32" })
      .map(command => command.command)
    expect(windowsCommands).toContain("app")
    expect(windowsCommands).toContain("sandbox-add-read-dir")

    const debugCommands = khalaCodeDesktopSlashCommands({
      debug: true,
      platform: "darwin",
    }).map(command => command.command)
    expect(debugCommands).toContain("rollout")
    expect(debugCommands).toContain("test-approval")

    const androidCommands = khalaCodeDesktopSlashCommands({ platform: "android" })
      .map(command => command.command)
    expect(androidCommands).not.toContain("copy")
  })

  test("preserves active-turn and side-conversation availability", () => {
    const plan = findKhalaCodeDesktopSlashCommand("/plan")!
    const raw = findKhalaCodeDesktopSlashCommand("/raw")!
    const model = findKhalaCodeDesktopSlashCommand("/model")!

    expect(evaluateKhalaCodeDesktopSlashCommandAvailability(plan, {
      activeTurn: true,
    })).toMatchObject({
      available: false,
      reason: "/plan is not available while Codex is working.",
    })
    expect(evaluateKhalaCodeDesktopSlashCommandAvailability(raw, {
      sideConversation: true,
    })).toEqual({ available: true })
    expect(evaluateKhalaCodeDesktopSlashCommandAvailability(model, {
      sideConversation: true,
    })).toMatchObject({
      available: false,
      reason: "/model is only available from the main thread.",
    })
  })

  test("documents dispatch coverage for every Codex command", () => {
    const groups = new Set(KHALA_CODE_DESKTOP_SLASH_COMMANDS.map(command => command.group))
    expect([...groups].sort()).toEqual([
      "background",
      "diagnostics",
      "ecosystem",
      "exit",
      "session",
      "settings",
      "turn_task",
      "workspace",
    ])

    for (const entry of khalaCodeDesktopSlashCommandDispatchCoverage()) {
      if (entry.dispatchKind === "app_server") {
        expect(entry.method).toBeTruthy()
      }
      if (entry.dispatchKind === "gap") {
        expect(entry.dependency).toContain("Codex")
      }
    }
  })

  test("maps IDE, diff, and mention parity commands to Codex app-server methods", () => {
    expect(findKhalaCodeDesktopSlashCommand("/ide")?.dispatch).toMatchObject({
      kind: "app_server",
      method: "config/read",
    })
    expect(findKhalaCodeDesktopSlashCommand("/diff")?.dispatch).toMatchObject({
      kind: "app_server",
      method: "gitDiffToRemote",
    })
    expect(findKhalaCodeDesktopSlashCommand("/mention")?.dispatch).toMatchObject({
      kind: "app_server",
      method: "fuzzyFileSearch",
    })
  })

  test("maps preference and appearance commands to Codex config methods", () => {
    const coverage = new Map(
      khalaCodeDesktopSlashCommandDispatchCoverage()
        .map(entry => [entry.command, entry]),
    )

    for (const command of ["keymap", "vim", "statusline", "theme", "pets", "personality"]) {
      expect(coverage.get(command)).toMatchObject({
        command,
        dispatchKind: "app_server",
        method: "config/read",
      })
    }
    expect(findKhalaCodeDesktopSlashCommand("/pet")?.command).toBe("pets")
  })

  test("maps BTW steering while keeping side-agent plan gaps typed", () => {
    const btw = findKhalaCodeDesktopSlashCommand("/btw")!
    expect(btw.dispatch).toMatchObject({
      kind: "app_server",
      method: "turn/steer",
      requiresArgs: true,
    })

    for (const raw of ["/approve", "/plan", "/agent", "/subagents", "/side"]) {
      const command = findKhalaCodeDesktopSlashCommand(raw)!
      expect(command.dispatch).toMatchObject({
        kind: "gap",
        unavailable: {
          kind: "upstream_app_server_gap",
          gapId: "codex.app_server.gap.side_agent_plan_controls",
        },
      })
    }
  })
})
