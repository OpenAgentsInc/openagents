import { describe, expect, test } from "bun:test"
import type { HarnessV1NetworkPolicy } from "@ai-sdk/harness"
import type {
  Experimental_SandboxProcess,
  Experimental_SandboxSession,
} from "@ai-sdk/provider-utils"
import {
  buildOpenAgentsSandboxSnapshotIdentity,
  createOpenAgentsAiSdkSandboxProvider,
  type OpenAgentsSandboxCreateSessionInput,
  type OpenAgentsSandboxSessionDescriptor,
  type OpenAgentsSandboxV1Client,
} from "./index.js"

const accountHomes = {
  claudeConfigDir: "/workspace/.agents/claude-code",
  codexHome: "/workspace/.agents/codex",
  home: "/workspace/.agents/home",
}

const snapshotInputs = {
  agentsSetupRef: ".agents/setup@sha256:setup",
  baseImageRef: "openagents/sandbox:2026-07-04",
  lockfileRefs: ["bun.lock@sha256:bun", "package.json@sha256:pkg"],
  repoRef: "github.com/OpenAgentsInc/openagents@main",
  sandboxProfileRef: "openagents.public-untrusted.v1",
  toolchainRef: "bun@1.3.11,node@24",
}

describe("createOpenAgentsAiSdkSandboxProvider", () => {
  test("maps lifecycle, onFirstCreate, explicit homes, and snapshot identity", async () => {
    const client = new FixtureOpenAgentsSandboxClient()
    const provider = createOpenAgentsAiSdkSandboxProvider({
      accountHomes,
      client,
      defaultWorkingDirectory: "/workspace",
      initialNetworkPolicy: { mode: "deny-all" },
      lane: "public_untrusted",
      ports: [3901],
      snapshotInputs,
    })

    const session = await provider.createSession({
      identity: "ai-sdk-bootstrap-recipe.abc123",
      onFirstCreate: async (restricted: Experimental_SandboxSession) => {
        await restricted.writeTextFile({
          content: "setup complete",
          path: "setup/result.txt",
        })
      },
      sessionId: "thread-1",
    })

    expect(client.creates).toHaveLength(1)
    const create = client.creates[0]
    expect(create.accountHomes).toEqual(accountHomes)
    expect(create.snapshotIdentity.inputs).toMatchObject({
      agentsSetupRef: snapshotInputs.agentsSetupRef,
      baseImageRef: snapshotInputs.baseImageRef,
      bridgeBootstrapRecipeRef: "ai-sdk-bootstrap-recipe.abc123",
      repoRef: snapshotInputs.repoRef,
      sandboxProfileRef: snapshotInputs.sandboxProfileRef,
      schemaVersion: "openagents.sandbox.snapshot_identity.v1",
      toolchainRef: snapshotInputs.toolchainRef,
    })
    expect(create.snapshotIdentity.identity).toStartWith("sha256:")
    expect(await session.readTextFile({ path: "setup/result.txt" })).toBe(
      "setup complete",
    )

    const resumed = await provider.resumeSession?.({ sessionId: "thread-1" })
    expect(resumed?.id).toBe("thread-1")
    await session.stop()
    expect(client.stopped).toEqual(["thread-1"])
    await session.destroy?.()
    expect(client.destroyed).toEqual(["thread-1"])
  })

  test("covers file I/O, bridge port ingress, and restricted view", async () => {
    const client = new FixtureOpenAgentsSandboxClient()
    const provider = createOpenAgentsAiSdkSandboxProvider({
      accountHomes,
      client,
      initialNetworkPolicy: { mode: "allow-all" },
      ports: [4101],
      snapshotInputs,
    })
    const session = await provider.createSession({ sessionId: "thread-ports" })

    await session.writeTextFile({
      content: "console.log('hello')",
      path: "src/index.ts",
    })
    expect(await session.readTextFile({ path: "/workspace/src/index.ts" })).toBe(
      "console.log('hello')",
    )
    await expect(session.readTextFile({ path: "/tmp/outside.txt" })).rejects.toThrow(
      "escapes OpenAgents sandbox",
    )
    await expect(session.readTextFile({ path: "../outside.txt" })).rejects.toThrow(
      "escapes OpenAgents sandbox",
    )

    await session.setPorts?.([4102])
    expect(await session.getPortUrl({ port: 4102, protocol: "ws" })).toBe(
      "wss://sandbox.openagents.test/thread-ports/ports/4102",
    )
    const restricted = session.restricted()
    expect("stop" in restricted).toBe(false)
    expect("destroy" in restricted).toBe(false)
    expect("setNetworkPolicy" in restricted).toBe(false)
    expect("getPortUrl" in restricted).toBe(false)
  })

  test("enforces public-lane network policy below the AI SDK adapter", async () => {
    const client = new FixtureOpenAgentsSandboxClient()
    const provider = createOpenAgentsAiSdkSandboxProvider({
      accountHomes,
      client,
      initialNetworkPolicy: { mode: "deny-all" },
      lane: "public_untrusted",
      snapshotInputs,
    })
    const session = await provider.createSession({ sessionId: "network" })

    expect((await session.run({
      command: "openagents-network-check allowed.example",
    })).exitCode).toBe(42)
    await session.setNetworkPolicy?.({
      allowedHosts: ["allowed.example"],
      mode: "custom",
    })
    expect((await session.run({
      command: "openagents-network-check allowed.example",
    })).exitCode).toBe(0)
    expect((await session.run({
      command: "openagents-network-check blocked.example",
    })).exitCode).toBe(42)
    await expect(session.setNetworkPolicy?.({ mode: "allow-all" })).rejects.toThrow(
      "Public/untrusted",
    )

    expect(() =>
      createOpenAgentsAiSdkSandboxProvider({
        accountHomes,
        client,
        initialNetworkPolicy: { mode: "allow-all" },
        lane: "public_untrusted",
        snapshotInputs,
      }),
    ).toThrow("Public/untrusted")
  })

  test("passes explicit agent homes into process environments", async () => {
    const client = new FixtureOpenAgentsSandboxClient()
    const provider = createOpenAgentsAiSdkSandboxProvider({
      accountHomes,
      client,
      snapshotInputs,
    })
    const session = await provider.createSession({ sessionId: "env" })
    const result = await session.run({
      command: "print-openagents-env",
      env: {
        HOME: "/ambient/should-not-win",
      },
    })
    expect(result.stdout).toBe(
      [
        "HOME=/workspace/.agents/home",
        "CODEX_HOME=/workspace/.agents/codex",
        "CLAUDE_CONFIG_DIR=/workspace/.agents/claude-code",
      ].join("\n"),
    )
  })

  test("snapshot identity is stable across lockfile order and sensitive to setup", () => {
    const first = buildOpenAgentsSandboxSnapshotIdentity({
      bridgeBootstrapRecipeRef: "bridge@sha256:1",
      inputs: snapshotInputs,
    })
    const second = buildOpenAgentsSandboxSnapshotIdentity({
      bridgeBootstrapRecipeRef: "bridge@sha256:1",
      inputs: {
        ...snapshotInputs,
        lockfileRefs: [...snapshotInputs.lockfileRefs].reverse(),
      },
    })
    const changed = buildOpenAgentsSandboxSnapshotIdentity({
      bridgeBootstrapRecipeRef: "bridge@sha256:1",
      inputs: {
        ...snapshotInputs,
        agentsSetupRef: ".agents/setup@sha256:changed",
      },
    })

    expect(second.identity).toBe(first.identity)
    expect(changed.identity).not.toBe(first.identity)
  })
})

class FixtureOpenAgentsSandboxClient implements OpenAgentsSandboxV1Client {
  readonly creates: OpenAgentsSandboxCreateSessionInput[] = []
  readonly destroyed: string[] = []
  readonly stopped: string[] = []
  private readonly files = new Map<string, Map<string, Uint8Array>>()
  private readonly sessions = new Map<string, {
    descriptor: OpenAgentsSandboxSessionDescriptor
    networkPolicy: HarnessV1NetworkPolicy | undefined
  }>()

  async createSession(
    input: OpenAgentsSandboxCreateSessionInput,
  ): Promise<OpenAgentsSandboxSessionDescriptor> {
    this.creates.push(input)
    const descriptor = {
      defaultWorkingDirectory: input.defaultWorkingDirectory,
      fresh: !this.sessions.has(input.sessionId),
      id: input.sessionId,
      ports: [...input.ports],
    }
    this.sessions.set(input.sessionId, {
      descriptor,
      networkPolicy: input.networkPolicy,
    })
    this.files.set(input.sessionId, this.files.get(input.sessionId) ?? new Map())
    return descriptor
  }

  async resumeSession(input: {
    sessionId: string
  }): Promise<OpenAgentsSandboxSessionDescriptor> {
    const session = this.requireSession(input.sessionId)
    return session.descriptor
  }

  async stopSession(input: { sessionId: string }): Promise<void> {
    this.requireSession(input.sessionId)
    this.stopped.push(input.sessionId)
  }

  async destroySession(input: { sessionId: string }): Promise<void> {
    this.requireSession(input.sessionId)
    this.destroyed.push(input.sessionId)
    this.sessions.delete(input.sessionId)
    this.files.delete(input.sessionId)
  }

  async readBinaryFile(input: {
    sessionId: string
    path: string
  }): Promise<Uint8Array | null> {
    this.requireSession(input.sessionId)
    return this.files.get(input.sessionId)?.get(input.path) ?? null
  }

  async writeBinaryFile(input: {
    sessionId: string
    path: string
    content: Uint8Array
  }): Promise<void> {
    this.requireSession(input.sessionId)
    const files = this.files.get(input.sessionId) ?? new Map()
    files.set(input.path, input.content)
    this.files.set(input.sessionId, files)
  }

  async run(input: {
    command: string
    env: Readonly<Record<string, string>>
    sessionId: string
  }): Promise<{ exitCode: number; stdout: string; stderr: string }> {
    const session = this.requireSession(input.sessionId)
    if (input.command === "print-openagents-env") {
      return {
        exitCode: 0,
        stderr: "",
        stdout: [
          `HOME=${input.env.HOME}`,
          `CODEX_HOME=${input.env.CODEX_HOME}`,
          `CLAUDE_CONFIG_DIR=${input.env.CLAUDE_CONFIG_DIR}`,
        ].join("\n"),
      }
    }
    const host = input.command.match(/^openagents-network-check (?<host>.+)$/)?.groups
      ?.host
    if (host !== undefined) {
      return networkCheckResult(session.networkPolicy, host)
    }
    return {
      exitCode: 0,
      stderr: "",
      stdout: "",
    }
  }

  async spawn(): Promise<Experimental_SandboxProcess> {
    return emptyProcess()
  }

  async getPortUrl(input: {
    port: number
    protocol?: "http" | "https" | "ws"
    sessionId: string
  }): Promise<string> {
    this.requireSession(input.sessionId)
    const scheme = input.protocol === "ws" ? "wss" : "https"
    return `${scheme}://sandbox.openagents.test/${input.sessionId}/ports/${input.port}`
  }

  async setPorts(input: {
    ports: ReadonlyArray<number>
    sessionId: string
  }): Promise<ReadonlyArray<number>> {
    const session = this.requireSession(input.sessionId)
    session.descriptor = {
      ...session.descriptor,
      ports: [...input.ports],
    }
    return session.descriptor.ports
  }

  async setNetworkPolicy(input: {
    policy: HarnessV1NetworkPolicy
    sessionId: string
  }): Promise<void> {
    this.requireSession(input.sessionId).networkPolicy = input.policy
  }

  private requireSession(sessionId: string): {
    descriptor: OpenAgentsSandboxSessionDescriptor
    networkPolicy: HarnessV1NetworkPolicy | undefined
  } {
    const session = this.sessions.get(sessionId)
    if (session === undefined) {
      throw new Error(`Missing fixture session ${sessionId}.`)
    }
    return session
  }
}

function networkCheckResult(
  policy: HarnessV1NetworkPolicy | undefined,
  host: string,
): { exitCode: number; stdout: string; stderr: string } {
  if (policy?.mode === "allow-all") {
    return { exitCode: 0, stderr: "", stdout: "allowed" }
  }
  if (policy?.mode === "custom" && policy.allowedHosts?.includes(host)) {
    return { exitCode: 0, stderr: "", stdout: "allowed" }
  }
  return { exitCode: 42, stderr: "blocked", stdout: "" }
}

function emptyProcess(): Experimental_SandboxProcess {
  const empty = new ReadableStream<Uint8Array>({
    start(controller) {
      controller.close()
    },
  })
  return {
    stderr: empty,
    stdout: empty,
    kill: async () => {},
    wait: async () => ({ exitCode: 0 }),
  }
}
