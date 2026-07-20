import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { ChildProcess } from "node:child_process"
import { PassThrough } from "node:stream"

import { describe, expect, test } from "vite-plus/test"

import {
  IdeRunCommandSchema,
  type IdeRunEvent,
  type IdeRunSnapshot,
} from "./run-contract.ts"
import {
  buildIdeRunEnvironment,
  openIdeRunHost,
  type IdeRunSpawn,
} from "./run-host.ts"
import { defaultSafeTerminalEnvironment } from "../terminal-host.ts"
import type { IdePortableMutationAuthority } from "./portable-mutation-authority.ts"
import type { IdePortableMutationPermit } from "./portable-mutation-authority.ts"

const owner = { _tag: "Human" as const, actorRef: "owner.desktop" }
const testMutationAuthority: IdePortableMutationAuthority = {
  authorize: grantRef => ({
    _tag: "Permitted",
    permit: Object.freeze({
      _tag: "LocalOnly",
      key: `local:${grantRef}`,
      grantRef,
      sessionRef: `session.${grantRef}`,
      workContextRef: `work-context.${grantRef}`,
      attachmentRef: null,
      generation: null,
      targetRef: null,
    }),
  }),
  reauthorize: () => true,
}

const portablePermit = (generation: number): IdePortableMutationPermit => Object.freeze({
  _tag: "Portable",
  key: `portable:attachment.${generation}:${generation}`,
  grantRef: "workspace.grant.portable-run",
  sessionRef: "session.portable-run",
  workContextRef: "work-context.portable-run",
  attachmentRef: `attachment.${generation}`,
  generation,
  targetRef: "target.local",
})

type FakeRunChild = ChildProcess & Readonly<{
  fakeStdout: PassThrough
  fakeStderr: PassThrough
}>

const makeFakeRunChild = (): FakeRunChild => {
  const child = new ChildProcess()
  const fakeStdout = new PassThrough()
  const fakeStderr = new PassThrough()
  child.stdout = fakeStdout
  child.stderr = fakeStderr
  return Object.assign(child, {
    fakeStdout,
    fakeStderr,
  })
}

const waitFor = async (predicate: () => boolean, timeoutMs = 5_000): Promise<void> => {
  const deadline = Date.now() + timeoutMs
  while (!predicate()) {
    if (Date.now() > deadline) throw new Error("Timed out while waiting for the run host fixture.")
    await new Promise(resolve => setTimeout(resolve, 5))
  }
}

const writeSingleTask = (root: string): void => {
  mkdirSync(path.join(root, ".openagents"), { recursive: true })
  writeFileSync(path.join(root, ".openagents", "tasks.json"), JSON.stringify({
    version: 1,
    tasks: [{
      id: "portable",
      label: "Portable task",
      group: "test",
      executable: process.execPath,
      argv: ["-e", "process.stdout.write('task output')"],
      dependsOn: [],
      background: false,
      readinessPattern: null,
      timeoutMs: 5_000,
      maxRetries: 0,
      artifactPaths: [],
    }],
  }), "utf8")
}

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

  test("keeps discovery readable but refuses a task before spawn without portable authority", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-ide13-run-refused-"))
    writeSingleTask(root)
    let spawnCount = 0
    const host = await openIdeRunHost({
      workspace: () => ({ root, grantRef: "workspace.grant.portable-run" }),
      mutationAuthority: {
        authorize: () => ({ _tag: "Refused", reason: "sync_unavailable" }),
        reauthorize: () => false,
      },
      spawnProcess: ((..._args) => {
        spawnCount += 1
        return makeFakeRunChild()
      }) satisfies IdeRunSpawn,
      emit: () => undefined,
    })
    try {
      const discovered = await host.command(IdeRunCommandSchema.cases.Discover.make({}))
      expect(discovered?._tag).toBe("Succeeded")
      if (discovered?._tag !== "Succeeded") return
      const definition = discovered.snapshot.taskDefinitions.find(candidate => candidate.label === "Portable task")
      if (definition === undefined) throw new Error("expected portable task")
      const started = await host.command(IdeRunCommandSchema.cases.StartTask.make({
        definitionRef: definition.definitionRef,
        actor: owner,
      }))
      expect(started).toMatchObject({ _tag: "Refused", reason: "stale_generation" })
      expect(spawnCount).toBe(0)
      expect((await host.snapshot())?.taskDefinitions).toHaveLength(1)
    } finally {
      await host.dispose()
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("kills a stale task once after async launch and drops late output and settlement", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-ide13-task-stale-"))
    writeSingleTask(root)
    const children: FakeRunChild[] = []
    const signals: NodeJS.Signals[] = []
    const events: IdeRunEvent[] = []
    const permit = portablePermit(7)
    let activeKey = permit.key
    const observedPermitImmutability: boolean[] = []
    const host = await openIdeRunHost({
      workspace: () => ({ root, grantRef: permit.grantRef }),
      mutationAuthority: {
        authorize: () => ({ _tag: "Permitted", permit }),
        reauthorize: candidate => {
          observedPermitImmutability.push(Object.isFrozen(candidate))
          return candidate.key === activeKey
        },
      },
      spawnProcess: ((..._args) => {
        const child = makeFakeRunChild()
        children.push(child)
        return child
      }) satisfies IdeRunSpawn,
      signalProcess: (_child, signal) => { signals.push(signal) },
      emit: event => { events.push(event) },
    })
    try {
      const discovered = await host.command(IdeRunCommandSchema.cases.Discover.make({}))
      if (discovered?._tag !== "Succeeded") throw new Error("expected discovery")
      const definition = discovered.snapshot.taskDefinitions.find(candidate => candidate.label === "Portable task")
      if (definition === undefined) throw new Error("expected task")
      expect((await host.command(IdeRunCommandSchema.cases.StartTask.make({
        definitionRef: definition.definitionRef,
        actor: owner,
      })))?._tag).toBe("Succeeded")
      await waitFor(() => children.length === 1)
      activeKey = portablePermit(8).key
      children[0]!.emit("spawn")
      children[0]!.fakeStdout.emit("data", Buffer.from("late task output"))
      children[0]!.emit("exit", 0)
      await waitFor(() => signals.filter(signal => signal === "SIGTERM").length === 1)
      expect(signals.filter(signal => signal === "SIGTERM")).toHaveLength(1)
      expect(observedPermitImmutability.length).toBeGreaterThan(0)
      expect(observedPermitImmutability.every(Boolean)).toBe(true)
      expect(events.some(event => event._tag === "Snapshot" && event.snapshot.outputChannels.some(channel =>
        channel.chunks.some(chunk => chunk.text.includes("late task output"))))).toBe(false)
      expect(events.some(event => event._tag === "Snapshot" && event.snapshot.taskRuns.some(run =>
        run.outcome._tag === "Succeeded"))).toBe(false)
    } finally {
      await host.dispose()
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("kills a task once when portable authority changes during spawn", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-ide13-task-spawn-race-"))
    writeSingleTask(root)
    const children: FakeRunChild[] = []
    const signals: NodeJS.Signals[] = []
    const permit = portablePermit(9)
    let activeKey = permit.key
    const host = await openIdeRunHost({
      workspace: () => ({ root, grantRef: permit.grantRef }),
      mutationAuthority: {
        authorize: () => ({ _tag: "Permitted", permit }),
        reauthorize: candidate => candidate.key === activeKey,
      },
      spawnProcess: ((..._args) => {
        const child = makeFakeRunChild()
        children.push(child)
        activeKey = portablePermit(10).key
        return child
      }) satisfies IdeRunSpawn,
      signalProcess: (_child, signal) => { signals.push(signal) },
      emit: () => undefined,
    })
    try {
      const discovered = await host.command(IdeRunCommandSchema.cases.Discover.make({}))
      if (discovered?._tag !== "Succeeded") throw new Error("expected discovery")
      const definition = discovered.snapshot.taskDefinitions.find(candidate => candidate.label === "Portable task")
      if (definition === undefined) throw new Error("expected task")
      expect((await host.command(IdeRunCommandSchema.cases.StartTask.make({
        definitionRef: definition.definitionRef,
        actor: owner,
      })))?._tag).toBe("Succeeded")
      await waitFor(() => children.length === 1)
      expect(signals.filter(signal => signal === "SIGTERM")).toHaveLength(1)
      children[0]!.emit("spawn")
      children[0]!.fakeStdout.emit("data", Buffer.from("late spawn-race output"))
      children[0]!.emit("exit", 0)
      expect(signals.filter(signal => signal === "SIGTERM")).toHaveLength(1)
    } finally {
      await host.dispose()
      rmSync(root, { recursive: true, force: true })
    }
  })

  test("kills a stale test process and drops its late assertion result", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-ide13-test-stale-"))
    writeFileSync(path.join(root, "fixture.test.ts"), "export {}\n", "utf8")
    const children: FakeRunChild[] = []
    const signals: NodeJS.Signals[] = []
    const events: IdeRunEvent[] = []
    const permit = portablePermit(11)
    let activeKey = permit.key
    const host = await openIdeRunHost({
      workspace: () => ({ root, grantRef: permit.grantRef }),
      mutationAuthority: {
        authorize: () => ({ _tag: "Permitted", permit }),
        reauthorize: candidate => candidate.key === activeKey,
      },
      spawnProcess: ((..._args) => {
        const child = makeFakeRunChild()
        children.push(child)
        return child
      }) satisfies IdeRunSpawn,
      signalProcess: (_child, signal) => { signals.push(signal) },
      emit: event => { events.push(event) },
    })
    try {
      const discovered = await host.command(IdeRunCommandSchema.cases.Discover.make({}))
      if (discovered?._tag !== "Succeeded") throw new Error("expected discovery")
      const controller = discovered.snapshot.testControllers[0]
      if (controller === undefined) throw new Error("expected test controller")
      const item = controller.items.find(candidate => candidate.kind === "file")
      if (item === undefined) throw new Error("expected test item")
      expect((await host.command(IdeRunCommandSchema.cases.RunTests.make({
        controllerRef: controller.controllerRef,
        itemRefs: [item.itemRef],
        profile: "run",
        retryOf: null,
        actor: owner,
      })))?._tag).toBe("Succeeded")
      await waitFor(() => children.length === 1)
      activeKey = portablePermit(12).key
      children[0]!.fakeStdout.emit("data", Buffer.from("Tests 1 passed"))
      children[0]!.emit("exit", 0)
      await waitFor(() => signals.filter(signal => signal === "SIGTERM").length === 1)
      expect(signals.filter(signal => signal === "SIGTERM")).toHaveLength(1)
      expect(events.some(event => event._tag === "Snapshot" && event.snapshot.testRuns.some(run =>
        run.outcome._tag === "Succeeded"))).toBe(false)
    } finally {
      await host.dispose()
      rmSync(root, { recursive: true, force: true })
    }
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
      mutationAuthority: testMutationAuthority,
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
