import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import path from "node:path"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"
import { openAgentsDesktopUxContractRegistry } from "../contracts/ux-contracts.ts"
import {
  assuranceSpecWorkspaceStateFromSource,
  assuranceSpecWorkspaceView,
  makeAssuranceSpecWorkspaceHandlers,
  mvpAssuranceSpecRelativePath,
} from "./assurance-spec-workspace.ts"

type AnyNode = Readonly<Record<string, unknown>>

const collectNodes = (root: unknown): ReadonlyArray<AnyNode> => {
  const nodes: AnyNode[] = []
  const visit = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const child of value) visit(child)
      return
    }
    if (typeof value !== "object" || value === null) return
    const record = value as AnyNode
    if (typeof record._tag === "string") nodes.push(record)
    for (const [key, child] of Object.entries(record)) {
      if (key !== "style" && key !== "a11y") visit(child)
    }
  }
  visit(root)
  return nodes
}

const byKey = (root: unknown, key: string): AnyNode | undefined =>
  collectNodes(root).find(node => node.key === key)

const textContent = (root: unknown): ReadonlyArray<string> =>
  collectNodes(root)
    .filter(node => node._tag === "Text")
    .map(node => String(node.content ?? ""))

const source = readFileSync(
  path.resolve(path.dirname(new URL(import.meta.url).pathname), "../../../../docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md"),
  "utf8",
)

describe("AssuranceSpec document support", () => {
  test("renders the admitted proof design without inventing execution evidence", () => {
    const workspace = assuranceSpecWorkspaceStateFromSource(source)
    expect(workspace.projection.state).toBe("ready")
    if (workspace.projection.state !== "ready") throw new Error("fixture must be valid")
    expect(workspace.projection.assessment.coverage).toEqual({
      criteria: 18,
      obligations: 18,
      ready: 18,
      needs_design: 0,
    })
    expect(workspace.projection.document.repositoryInventory.candidateCount).toBe(400)
    expect(JSON.stringify(workspace.projection)).not.toContain("candidate_artifact_refs")
    expect(JSON.stringify(workspace.projection)).not.toContain("apps/autopilot-desktop")

    const view = assuranceSpecWorkspaceView(workspace)
    expect(byKey(view, "assurance-spec-structure")).toMatchObject({ label: "Structure valid", tone: "success" })
    expect(byKey(view, "assurance-spec-lifecycle")).toMatchObject({ label: "Admitted", tone: "success" })
    expect(byKey(view, "assurance-spec-design-state")).toMatchObject({ label: "18 proof designs ready", tone: "success" })
    expect(byKey(view, "assurance-spec-execution-state")).toMatchObject({ label: "Evidence not loaded", tone: "neutral" })
    expect(byKey(view, "assurance-selected-criterion")).toMatchObject({ label: "CW-AC-04", tone: "info" })
    expect(byKey(view, "assurance-proof-design-table")).toMatchObject({ _tag: "Table" })
    expect(byKey(view, "assurance-repository-policy-value")?.content).toBe("400 candidates · unmapped")

    const labels = collectNodes(view)
      .filter(node => node._tag === "Button")
      .map(node => String(node.label ?? ""))
    expect(labels).toHaveLength(18)
    expect(labels.some(label => /^(Run|Admit|Verify|Release)\b/.test(label))).toBe(false)
    expect(textContent(view).join("\n")).toContain("Read-only review. Admission is recorded")
  })

  test("changes the selected obligation through one typed intent", async () => {
    const initial = { assuranceSpec: assuranceSpecWorkspaceStateFromSource(source) }
    const state = await Effect.runPromise(SubscriptionRef.make(initial))
    const handlers = makeAssuranceSpecWorkspaceHandlers(state)
    await Effect.runPromise(handlers.AssuranceSpecObligationSelected("AO-CW-AC-10-01", {
      name: "AssuranceSpecObligationSelected",
      payload: "AO-CW-AC-10-01",
    }))
    expect((await Effect.runPromise(SubscriptionRef.get(state))).assuranceSpec.selectedObligationId).toBe("AO-CW-AC-10-01")
  })

  test("invalid source fails closed instead of partially rendering proposal facts", () => {
    const workspace = assuranceSpecWorkspaceStateFromSource("# not an assurance spec", "docs/broken.assurance-spec.md")
    expect(workspace.projection.state).toBe("invalid")
    const view = assuranceSpecWorkspaceView(workspace)
    expect(byKey(view, "assurance-spec-invalid-badge")).toMatchObject({ label: "Invalid AssuranceSpec", tone: "danger" })
    expect(byKey(view, "assurance-summary-strip")).toBeUndefined()
  })

  test("registers the owner-authored visualization behavior contract", () => {
    expect(validateBehaviorContractRegistry(openAgentsDesktopUxContractRegistry).ok).toBe(true)
    expect(openAgentsDesktopUxContractRegistry.contracts.find(
      contract => contract.contractId === "openagents_desktop.assurance_spec.document_visualization.v1",
    )?.state).toBe("enforced")
  })
})
