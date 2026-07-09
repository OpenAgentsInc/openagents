import { describe, expect, test } from "bun:test"
import { Window } from "happy-dom"
import { viewStructure } from "@effect-native/render-dom"
import { Effect, Exit, Scope } from "@effect-native/core/effect"
import { KhalaFleetIntent } from "@openagentsinc/khala-fleet-intents"
import { Schema as S } from "effect"

import {
  buildEnCockpitProjection,
  enCockpitFixtureStatus,
  enCockpitView,
  mountEnCockpitSurface,
  runControlToFleetIntent,
  type MountedEnCockpit,
} from "../src/ui/en-cockpit"

const decodeFleetIntent = S.decodeUnknownSync(KhalaFleetIntent)

const waitFor = async (
  predicate: () => boolean,
  timeoutMs = 500,
): Promise<void> => {
  const start = Date.now()
  while (!predicate()) {
    if (Date.now() - start > timeoutMs) {
      throw new Error("waitFor timed out")
    }
    await new Promise((resolve) => setTimeout(resolve, 5))
  }
}

describe("EN cockpit projection (MH-7 / EN-5)", () => {
  test("maps live-shaped fleet status into cockpit state", () => {
    const state = buildEnCockpitProjection(enCockpitFixtureStatus)

    expect(state.pylonStatusLabel).toBe("online")
    expect(state.capacityChips.map((chip) => chip.key)).toEqual([
      "chip-pylon",
      "chip-accounts",
      "chip-slots",
      "chip-runs",
    ])
    expect(
      state.capacityChips.find((chip) => chip.key === "chip-accounts")?.value,
    ).toBe("2/3")
    expect(
      state.capacityChips.find((chip) => chip.key === "chip-slots")?.value,
    ).toBe("1/3 free")

    expect(state.harnessRows.map((row) => row.harnessKind)).toEqual([
      "codex",
      "claude",
      "codex",
    ])
    expect(state.harnessRows[2]?.tone).toBe("blocked")

    // One assignment is `approval_required` → exactly one pending approval.
    expect(state.pendingApprovals).toHaveLength(1)
    expect(state.pendingApprovals[0]?.approvalRef).toBe("assignment.public.one")
    expect(state.runControlTargetRef).toBe("run.public.alpha")

    // Public-safe: no emails / account keys leaked into the render state.
    const serialized = JSON.stringify(state)
    expect(serialized).not.toContain("@")
    expect(serialized).not.toContain("account_key")
  })
})

describe("EN cockpit view (MH-7 / EN-5)", () => {
  test("authored cockpit is a typed Effect Native tree", () => {
    const tree = enCockpitView(buildEnCockpitProjection(enCockpitFixtureStatus))
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({ tag: "Stack", key: "cockpit-root" })
    expect(serialized).toContain('"catalogVersion":"effect-native/v5"')
    expect(serialized).toContain("Fleet cockpit")
    expect(serialized).toContain("CockpitRunControl")
    expect(serialized).toContain("CockpitApprovalDecision")
    expect(serialized).toContain("cockpit-run-control-pause")
    // Typed EN tree, not React/HTML.
    expect(serialized).not.toContain("className")
  })
})

describe("EN cockpit DOM render + dispatch (MH-7 / EN-5)", () => {
  const withMount = async (
    run: (input: {
      root: HTMLElement
      mounted: MountedEnCockpit
    }) => Promise<void>,
  ): Promise<void> => {
    const window = new Window({ url: "http://localhost/" })
    const document = window.document as unknown as Document
    const globalWithDocument = globalThis as { document?: Document | undefined }
    const previousDocument = globalWithDocument.document
    globalWithDocument.document = document

    const root = document.createElement("div")
    document.body.appendChild(root)

    const scope = await Effect.runPromise(Scope.make())
    try {
      const mounted = await Effect.runPromise(
        Scope.provide(scope)(
          mountEnCockpitSurface(root as unknown as HTMLElement, {
            initialStatus: enCockpitFixtureStatus,
            intentContext: {
              now: () => "2026-07-08T20:05:00.000Z",
              newIntentId: () => "intent_test_fixed",
            },
          }),
        ),
      )
      await run({ root: root as unknown as HTMLElement, mounted })
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void))
      globalWithDocument.document = previousDocument
    }
  }

  test("renders the cockpit through the real Effect Native DOM renderer", async () => {
    await withMount(async ({ root }) => {
      const text = root.textContent ?? ""
      expect(text).toContain("Fleet cockpit")
      expect(text).toContain("github.issue.openagents.8586")

      const runControls = root.querySelectorAll('[data-en-key^="cockpit-run-control-"]')
      expect(runControls.length).toBe(4)

      const pause = root.querySelector('[data-en-key="cockpit-run-control-pause"]')
      expect(pause).not.toBeNull()

      const allow = root.querySelector('[data-en-key="cockpit-approval-1-allow"]')
      expect(allow).not.toBeNull()
    })
  })

  test("dispatching pause produces the correct typed KhalaFleetIntent", async () => {
    await withMount(async ({ root, mounted }) => {
      const pause = root.querySelector(
        '[data-en-key="cockpit-run-control-pause"]',
      ) as HTMLButtonElement | null
      expect(pause).not.toBeNull()

      pause!.click()
      await waitFor(() => mounted.dispatchedIntents.length > 0)

      const intent = mounted.dispatchedIntents[0]!
      // The dispatched value is a valid shared KhalaFleetIntent (decode proves it).
      const decoded = decodeFleetIntent(intent)
      expect(decoded.kind).toBe("fleet_run_control")
      if (decoded.kind === "fleet_run_control") {
        expect(decoded.action).toBe("pause")
      }
      expect(intent.runRef).toBe("run.public.alpha")
      expect(intent.origin.surface).toBe("desktop")
      expect(intent.schema).toBe("khala.fleet_intent.v1")
    })
  })

  test("dispatching an approval allow produces the correct approval intent", async () => {
    await withMount(async ({ root, mounted }) => {
      const allow = root.querySelector(
        '[data-en-key="cockpit-approval-1-allow"]',
      ) as HTMLButtonElement | null
      expect(allow).not.toBeNull()

      allow!.click()
      await waitFor(() => mounted.dispatchedIntents.length > 0)

      const intent = mounted.dispatchedIntents[0]!
      expect(intent.kind).toBe("approval_decision")
      if (intent.kind === "approval_decision") {
        expect(intent.decision).toBe("allow")
        expect(intent.approvalRef).toBe("assignment.public.one")
      }
    })
  })
})

describe("EN cockpit intent converter (MH-7 / EN-5)", () => {
  test("run-control converter builds a decode-valid fleet intent", () => {
    const intent = runControlToFleetIntent(
      { action: "drain", runRef: "run.public.alpha" },
      { now: () => "2026-07-08T20:05:00.000Z", newIntentId: () => "intent_fixed" },
    )
    expect(intent).toMatchObject({
      schema: "khala.fleet_intent.v1",
      kind: "fleet_run_control",
      action: "drain",
      runRef: "run.public.alpha",
      intentId: "intent_fixed",
      idempotencyKey: "intent_fixed",
      createdAt: "2026-07-08T20:05:00.000Z",
      origin: { surface: "desktop" },
    })
  })
})
