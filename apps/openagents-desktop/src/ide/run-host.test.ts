import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { describe, expect, test } from "vite-plus/test"

import {
  IdeRunCommandSchema,
  type IdeRunEvent,
  type IdeRunSnapshot,
} from "./run-contract.ts"
import {
  buildIdeRunEnvironment,
  openIdeRunHost,
} from "./run-host.ts"
import { defaultSafeTerminalEnvironment } from "../terminal-host.ts"

const owner = { _tag: "Human" as const, actorRef: "owner.desktop" }

const waitForSnapshot = (
  events: ReadonlyArray<IdeRunEvent>,
  subscribe: (listener: (event: IdeRunEvent) => void) => void,
  predicate: (snapshot: IdeRunSnapshot) => boolean,
): Promise<IdeRunSnapshot> => {
  const existing = events.findLast((event) => event._tag === "Snapshot" && predicate(event.snapshot))
  if (existing?._tag === "Snapshot") return Promise.resolve(existing.snapshot)
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("Timed out while waiting for an IDE run snapshot.")), 10_000)
    subscribe((event) => {
      if (event._tag !== "Snapshot" || !predicate(event.snapshot)) return
      clearTimeout(timer)
      resolve(event.snapshot)
    })
  })
}

describe("IDE-10 run host", () => {
  test("admits only the named safe environment and never exposes values", () => {
    const result = buildIdeRunEnvironment({
      HOME: "/owner/home",
      PATH: "/usr/bin",
      SHELL: "/bin/zsh",
      GH_TOKEN: "github_pat_never_render_this",
      RANDOM_UNRELATED: "not-admitted",
    })
    expect(result.values).toEqual({
      HOME: "/owner/home",
      PATH: "/usr/bin",
      SHELL: "/bin/zsh",
      TERM: "xterm-256color",
      COLORTERM: "truecolor",
      OPENAGENTS_DESKTOP_TERMINAL: "1",
    })
    expect(result.manifest.inheritedAllHostVariables).toBe(false)
    expect(result.manifest.valuesExposedToRenderer).toBe(false)
    expect(JSON.stringify(result.manifest)).not.toContain("/owner/home")
    expect(JSON.stringify(result.manifest)).not.toContain("github_pat")
    expect(defaultSafeTerminalEnvironment({
      PATH: "/usr/bin",
      GH_TOKEN: "github_pat_never_pass",
      RANDOM_UNRELATED: "not-admitted",
    })).toEqual({
      PATH: "/usr/bin",
      TERM: "xterm-256color",
      COLORTERM: "truecolor",
      OPENAGENTS_DESKTOP_TERMINAL: "1",
    })
  })

  test("runs declared dependency tasks, records artifacts, redacts output, and exports a mode-0600 tail", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-ide10-host-"))
    const exportsRoot = path.join(root, ".exports")
    mkdirSync(path.join(root, ".openagents"), { recursive: true })
    writeFileSync(path.join(root, ".openagents", "tasks.json"), JSON.stringify({
      version: 1,
      tasks: [
        {
          id: "prepare",
          label: "Prepare",
          group: "build",
          executable: process.execPath,
          argv: ["-e", "require('node:fs').writeFileSync('prepared.txt','ready'); process.stdout.write('prepare complete')"],
          dependsOn: [],
          background: false,
          readinessPattern: null,
          timeoutMs: 5_000,
          maxRetries: 0,
          artifactPaths: ["prepared.txt"],
        },
        {
          id: "verify",
          label: "Verify",
          group: "test",
          executable: process.execPath,
          argv: ["-e", "if(require('node:fs').readFileSync('prepared.txt','utf8')!=='ready')process.exit(2); require('node:fs').writeFileSync('verified.txt','ok'); process.stdout.write('sk-secretvalue123456 src/file.ts:3:4')"],
          dependsOn: ["prepare"],
          background: false,
          readinessPattern: null,
          timeoutMs: 5_000,
          maxRetries: 0,
          artifactPaths: ["verified.txt"],
        },
      ],
    }), "utf8")
    writeFileSync(path.join(root, "fixture.test.ts"), "export {}\n", "utf8")

    const events: IdeRunEvent[] = []
    const listeners = new Set<(event: IdeRunEvent) => void>()
    const host = await openIdeRunHost({
      workspace: () => ({ root, grantRef: "workspace.grant.ide10" }),
      environment: () => ({ PATH: process.env.PATH, GH_TOKEN: "github_pat_never_pass" }),
      exportRoot: exportsRoot,
      emit: (event) => {
        events.push(event)
        for (const listener of listeners) listener(event)
      },
    })
    try {
      const discovered = await host.command(IdeRunCommandSchema.cases.Discover.make({}))
      expect(discovered?._tag, discovered?._tag === "Refused" ? discovered.message : "").toBe("Succeeded")
      if (discovered?._tag !== "Succeeded") return
      const verify = discovered.snapshot.taskDefinitions.find((definition) => definition.label === "Verify")
      expect(verify?.dependencies).toHaveLength(1)
      expect(discovered.snapshot.testControllers[0]?.items.some((item) => item.label === "fixture.test.ts")).toBe(true)
      if (verify === undefined) return

      const completed = waitForSnapshot(events, (listener) => listeners.add(listener), (snapshot) =>
        snapshot.taskRuns.some((run) => run.definitionRef === verify.definitionRef && run.outcome._tag === "Succeeded"))
      const started = await host.command(IdeRunCommandSchema.cases.StartTask.make({ definitionRef: verify.definitionRef, actor: owner }))
      expect(started?._tag).toBe("Succeeded")
      const snapshot = await completed
      const dependency = snapshot.taskDefinitions.find((definition) => definition.label === "Prepare")
      expect(snapshot.taskRuns.some((run) => run.definitionRef === dependency?.definitionRef && run.outcome._tag === "Succeeded")).toBe(true)
      const run = snapshot.taskRuns.findLast((candidate) => candidate.definitionRef === verify.definitionRef)
      expect(run?.artifacts.map((artifact) => artifact.pathRef)).toEqual(["verified.txt"])
      expect(run?.problems[0]).toMatchObject({ pathRef: "src/file.ts", line: 3, column: 4 })
      const output = snapshot.outputChannels.find((channel) => channel.channelRef === run?.outputChannelRef)
      expect(output?.chunks.map((chunk) => chunk.text).join("")).toContain("«redacted»")
      expect(output?.redactionCount).toBeGreaterThan(0)
      if (output === undefined) return

      const exported = await host.command(IdeRunCommandSchema.cases.ExportOutput.make({ channelRef: output.channelRef, actor: owner }))
      expect(exported?._tag).toBe("Succeeded")
      const exportedFiles = readdirSync(exportsRoot)
      expect(exportedFiles).toHaveLength(1)
      const exportStats = statSync(path.join(exportsRoot, exportedFiles[0] ?? "missing"))
      expect(exportStats.mode & 0o777).toBe(0o600)
      expect(readFileSync(path.join(root, "verified.txt"), "utf8")).toBe("ok")
      expect(exported?._tag === "Succeeded" && exported.snapshot.receipts.some((receipt) => receipt.operation === "output_export")).toBe(true)
    } finally {
      await host.dispose()
      rmSync(root, { recursive: true, force: true })
    }
  })
})
