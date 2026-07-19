import { Window } from "happy-dom"
import { act } from "react"
import { createRoot } from "react-dom/client"
import { afterEach, describe, expect, test } from "vite-plus/test"
import { resolveIntentRef, type IntentReporter } from "@effect-native/core"
import { Effect } from "@effect-native/core/effect"

import { IdeAgentCodeSnapshotSchema } from "../ide/agent-code-contract.ts"
import { ideAgentFixtureAttachment, ideAgentFixtureDigest } from "../ide/agent-code-fixture.ts"
import { IdeManagedSandboxSnapshotSchema } from "../ide/managed-sandbox-contract.ts"
import { IdePlacementRefSchema } from "../ide/project-contract.ts"
import { ManagedSandboxPlacement } from "./react-workspace-surfaces.tsx"
import { initialDesktopShellState, type DesktopShellState } from "./shell.ts"

const restores: Array<() => void> = []

const installDom = (): HTMLDivElement => {
  const window = new Window({ url: "http://localhost/" })
  const values = {
    window,
    document: window.document,
    navigator: window.navigator,
    Node: window.Node,
    Element: window.Element,
    HTMLElement: window.HTMLElement,
    HTMLButtonElement: window.HTMLButtonElement,
    SVGElement: window.SVGElement,
    Event: window.Event,
    MouseEvent: window.MouseEvent,
    getComputedStyle: window.getComputedStyle.bind(window),
    IS_REACT_ACT_ENVIRONMENT: true,
  }
  const previous = new Map<string, PropertyDescriptor | undefined>()
  for (const [name, value] of Object.entries(values)) {
    previous.set(name, Object.getOwnPropertyDescriptor(globalThis, name))
    Object.defineProperty(globalThis, name, { configurable: true, writable: true, value })
  }
  restores.push(() => {
    for (const [name, descriptor] of previous) {
      if (descriptor === undefined) delete (globalThis as Record<string, unknown>)[name]
      else Object.defineProperty(globalThis, name, descriptor)
    }
  })
  const container = window.document.createElement("div") as unknown as HTMLDivElement
  window.document.body.appendChild(container as never)
  return container
}

afterEach(() => {
  while (restores.length > 0) restores.pop()?.()
})

describe("managed sandbox placement surface", () => {
  test("shows exact public facts and emits only typed lifecycle controls", async () => {
    const container = installDom()
    const root = createRoot(container)
    const received: Array<{ name: string; payload: unknown }> = []
    const report: IntentReporter = (ref, payload) => Effect.sync(() => received.push(resolveIntentRef(ref, payload)))
    const base = initialDesktopShellState("test-host", "12:00", "files")
    const attachment = ideAgentFixtureAttachment()
    const target = {
      targetRef: "target.gcp.desktop",
      targetClass: "openagents_managed" as const,
      provider: "google_cloud" as const,
      adapterRef: "adapter.gcp.desktop",
      region: "us-central1",
      isolation: "gce_vm" as const,
      dataPosture: "openagents_managed_region" as const,
    }
    const lease = {
      leaseRef: "lease.desktop.fixture",
      state: "active" as const,
      issuedAt: "2026-07-19T20:00:00.000Z",
      expiresAt: "2026-07-19T20:30:00.000Z",
      ttlSeconds: 1_800,
      renewable: true,
    }
    const budget = {
      currency: "USD" as const,
      maxCostMicros: 2_000_000,
      maxCpuMillis: 600_000,
      maxNetworkBytes: 0,
      maxArtifactBytes: 16 * 1024 * 1024,
      maxLifetimeSeconds: 1_800,
    }
    const capability = {
      capabilityRef: "capability.desktop.agent-turn",
      kind: "agent_turn" as const,
      state: "active" as const,
      expiresAt: lease.expiresAt,
    }
    const managedSandbox = IdeManagedSandboxSnapshotSchema.make({
      schemaVersion: "openagents.desktop.ide-managed-sandbox.v1",
      revision: 3,
      admission: {
        _tag: "Available",
        target,
        imageDigest: ideAgentFixtureDigest("a"),
        profileRef: "profile.desktop.codex",
        lease,
        budget,
        requestedCapabilities: [capability],
        networkPosture: "deny_all",
        custody: "openagents_managed_region",
        retentionRef: "retention.desktop.fixture",
        checkedAt: lease.issuedAt,
      },
      binding: {
        projectRef: attachment.projectRef,
        rootRef: attachment.rootRef,
        worktreeRef: attachment.worktreeRef,
        sessionRef: attachment.sessionRef,
        agentAttachmentRef: attachment.agentAttachmentRef,
        attachmentGeneration: attachment.attachmentGeneration,
        placementGeneration: attachment.placementGeneration,
        placementRef: IdePlacementRefSchema.make("ide.placement.managed.fixture"),
        workUnitRef: "work-unit.desktop.fixture",
        sandboxRef: "sandbox.desktop.fixture",
      },
      resource: {
        sandboxRef: "sandbox.desktop.fixture",
        workUnitRef: "work-unit.desktop.fixture",
        attachmentRef: attachment.agentAttachmentRef,
        attachmentGeneration: attachment.attachmentGeneration,
        resourceGeneration: 2,
        version: 3,
        lastEventSequence: 3,
        target,
        imageDigest: ideAgentFixtureDigest("a"),
        profileRef: "profile.desktop.codex",
        lease,
        budget,
        capabilities: [capability],
        facts: {
          lifecycle: "running",
          leaseState: "active",
          guestState: "present",
          filesystemState: "attached",
          ingressState: "broker_only",
          runtimeState: "running",
          acceptingWork: true,
          cleanupComplete: false,
        },
        createdAt: lease.issuedAt,
        updatedAt: lease.issuedAt,
      },
      projectCapability: null,
      turn: {
        turnRef: "turn.desktop.fixture",
        commandRef: "command.desktop.fixture",
        capabilityRef: capability.capabilityRef,
        turnSequence: 1,
        lastEventSequence: 1,
        runtime: { provider: "codex", modelRef: "model.gpt-5", harnessRef: "harness.codex" },
        status: "running",
        usage: null,
        createdAt: lease.issuedAt,
        startedAt: lease.issuedAt,
        settledAt: null,
      },
      events: [],
      receipts: [],
      freshness: "live",
      latencyClass: "remote_interactive",
      lastError: null,
    })
    const state: DesktopShellState = {
      ...base,
      agentCode: IdeAgentCodeSnapshotSchema.make({
        ...base.agentCode,
        lifecycle: "attached",
        attachment,
        revision: 1,
      }),
      managedSandbox,
    }
    await act(async () => root.render(<ManagedSandboxPlacement state={state} report={report} />))

    expect(container.textContent).toContain("GCE VM · us-central1")
    expect(container.textContent).toContain("openagents managed region")
    expect(container.textContent).toContain("live · remote interactive")
    expect(container.textContent).toContain("2 / v3")
    expect(container.textContent).toContain("$2.00 max")
    expect(container.textContent).toContain("agent_turn:active")
    const button = (label: string) => [...container.querySelectorAll<HTMLButtonElement>("button")]
      .find(candidate => candidate.textContent === label)
    expect(button("Create")?.disabled).toBe(true)
    expect(button("Stop")?.disabled).toBe(false)
    expect(button("Resume")?.disabled).toBe(true)
    expect(button("Interrupt")?.disabled).toBe(false)
    await act(async () => {
      button("Inspect")?.click()
      button("Interrupt")?.click()
    })
    expect(received).toEqual([
      { name: "DesktopManagedSandboxInspectRequested", payload: null },
      { name: "DesktopManagedSandboxInterruptRequested", payload: null },
    ])
    await act(async () => root.unmount())
  })
})
