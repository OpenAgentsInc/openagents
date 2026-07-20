import { Schema as S } from "effect"
import type { View } from "@effect-native/core"
import { describe, expect, test } from "vite-plus/test"

import { DesktopTurnEventFrame } from "../turn/desktop-turn-ipc.ts"
import {
  AFS_DELEGATION_CHILD_REF,
  delegationCardFromProjection,
  delegationInspectorView,
  initialDesktopShellState,
  seedDelegationCard,
  withDelegationFrame,
  type DesktopNoteEntry,
  type DesktopShellState,
} from "./shell.ts"

// AFS-04 delegation-error surfacing: the codex subagent card must show WHAT
// failed (not a bare ERRORED badge) and must open its right-pane inspector with
// the failure reason. These tests pin the renderer-side propagation of the
// bounded, public-safe `failureReason` the safe turn projection now carries.

const decodeFrame = S.decodeUnknownSync(DesktopTurnEventFrame)

const DELEGATION_REF = "request.delegation.1"
const FAILURE_REASON = "session_failed: delegate lane stopped"

const failedFrame = (reason: string = FAILURE_REASON): DesktopTurnEventFrame =>
  decodeFrame({
    kind: "terminal",
    requestRef: DELEGATION_REF,
    generation: 1,
    projection: {
      schema: "openagents.agent_turn_projection.v1",
      threadRef: "thread.1",
      requestRef: DELEGATION_REF,
      cardState: "failed",
      failureReason: reason,
      dataDestination: "on_device_local",
      usageTruth: "unknown",
      localOnly: true,
      updatedAt: "2026-07-20T00:00:00.000Z",
      messageChain: [],
      evidenceRefs: [],
    },
    receipt: {
      schema: "openagents.agent_turn_receipt.v1",
      requestRef: DELEGATION_REF,
      routeDecisionRef: "route.delegation.1",
      decision: "failed",
      usageTruth: "unknown",
      evidenceRefs: [],
    },
  })

const seededState = (): DesktopShellState => {
  const note: DesktopNoteEntry = {
    key: `${DELEGATION_REF}-child-${AFS_DELEGATION_CHILD_REF}`,
    role: "system",
    text: "Codex subagent",
    timestamp: "05:41",
    runtime: seedDelegationCard(DELEGATION_REF, "Delegate a demo task to codex"),
  }
  return { ...initialDesktopShellState("test"), notes: [note] }
}

const anyNodes = (root: unknown): Array<Readonly<Record<string, unknown>>> => {
  const found: Array<Readonly<Record<string, unknown>>> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) return value.forEach(walk)
    if (typeof value !== "object" || value === null) return
    const node = value as Readonly<Record<string, unknown>>
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag" || prop === "style" || prop === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}

describe("delegation error surfacing (AFS-04)", () => {
  test("delegationCardFromProjection puts the failure reason in the card detail AND transcript", () => {
    const frame = failedFrame()
    const card = delegationCardFromProjection(frame.projection, "Codex subagent", "prior objective")
    expect(card.status).toBe("failed")
    // Detail is the reason (overriding the seeded objective), so the card reads
    // "Codex subagent — ERRORED — <reason>" without a click.
    expect(card.detail).toBe(FAILURE_REASON)
    const last = card.transcript?.at(-1)
    expect(last).toEqual({ role: "system", text: FAILURE_REASON })
  })

  test("a non-terminal projection keeps the prior detail and appends no error line", () => {
    const running = decodeFrame({
      kind: "progress",
      requestRef: DELEGATION_REF,
      generation: 1,
      projection: {
        schema: "openagents.agent_turn_projection.v1",
        threadRef: "thread.1",
        requestRef: DELEGATION_REF,
        cardState: "running",
        dataDestination: "on_device_local",
        usageTruth: "unknown",
        localOnly: true,
        updatedAt: "2026-07-20T00:00:00.000Z",
        messageChain: [],
        evidenceRefs: [],
      },
    })
    const card = delegationCardFromProjection(running.projection, "Codex subagent", "reading files")
    expect(card.status).toBe("running")
    expect(card.detail).toBe("reading files")
    expect(card.transcript).toEqual([])
  })

  test("withDelegationFrame stores the reason on the note runtime detail and transcript", () => {
    const next = withDelegationFrame(seededState(), failedFrame())
    const runtime = next.notes[0]!.runtime
    expect(runtime?.kind).toBe("child")
    if (runtime?.kind !== "child") throw new Error("expected child runtime")
    expect(runtime.status).toBe("failed")
    expect(runtime.detail).toBe(FAILURE_REASON)
    expect(runtime.transcript).toContainEqual({ role: "system", text: FAILURE_REASON })
  })

  test("the right-pane inspector renders the failure reason instead of the running placeholder", () => {
    const next = withDelegationFrame(seededState(), failedFrame())
    const runtime = next.notes[0]!.runtime
    if (runtime?.kind !== "child") throw new Error("expected child runtime")
    const view: View = delegationInspectorView("agent-ref", runtime.transcript ?? [])
    const texts = anyNodes(view)
      .filter((node) => node._tag === "Text")
      .map((node) => String(node.content ?? ""))
    expect(texts).toContain(FAILURE_REASON)
    expect(texts).not.toContain("Running. No output yet.")
  })
})
