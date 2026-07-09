/**
 * SQ-7 (#8624): the Sarah surface consumes the Effect Native catalog pieces
 * that replaced its local workarounds — the transcript is the EN `Transcript`
 * primitive (effect-native#35) and the avatar pane is the EN `MediaVideo`
 * host (effect-native#67, catalog v26) mounted for the session lifetime.
 */
import { describe, expect, test } from "bun:test"

// main.ts boots against the real page on import; give it an inert document so
// the module loads headlessly and boot() no-ops (no #sarah-root here).
;(globalThis as { document?: unknown }).document ??= {
  readyState: "complete",
  getElementById: () => null,
  addEventListener: () => {},
}

const { sarahSurfaceView, sarahAvatarPaneView } = await import("./main.ts")

type SurfaceState = Parameters<typeof sarahSurfaceView>[0]

const baseState: SurfaceState = {
  status: "idle",
  avatarArmed: true,
  avatarActive: false,
  avatarSessionOpen: false,
  sandbox: false,
  input: "",
  transcript: [{ key: "welcome", role: "assistant", text: "Hello from Sarah" }],
  cards: [],
  accountPhase: "anonymous",
  accountEmail: null,
  activePanel: "blueprint",
  pendingAction: null,
  blueprintProspectRef: null,
  blueprintDraft: null,
  blueprintFacts: [],
  blueprintContactEmail: null,
  receiptsProspectRef: null,
  receipts: [],
}

type AnyNode = { readonly _tag?: string; readonly [key: string]: unknown }

const findByTag = (node: unknown, tag: string): AnyNode | null => {
  if (node === null || typeof node !== "object") return null
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = findByTag(child, tag)
      if (found) return found
    }
    return null
  }
  const record = node as AnyNode
  if (record._tag === tag) return record
  for (const value of Object.values(record)) {
    const found = findByTag(value, tag)
    if (found) return found
  }
  return null
}

const findAllByTag = (node: unknown, tag: string): ReadonlyArray<AnyNode> => {
  if (node === null || typeof node !== "object") return []
  if (Array.isArray(node)) return node.flatMap((child) => findAllByTag(child, tag))
  const record = node as AnyNode
  return [
    ...(record._tag === tag ? [record] : []),
    ...Object.values(record).flatMap((value) => findAllByTag(value, tag)),
  ]
}

const findByKey = (node: unknown, key: string): AnyNode | null => {
  if (node === null || typeof node !== "object") return null
  if (Array.isArray(node)) {
    for (const child of node) {
      const found = findByKey(child, key)
      if (found) return found
    }
    return null
  }
  const record = node as AnyNode
  if (record.key === key) return record
  for (const value of Object.values(record)) {
    const found = findByKey(value, key)
    if (found) return found
  }
  return null
}

describe("sarah surface consumes the EN catalog (SQ-7 #8624)", () => {
  test("the transcript is the EN Transcript primitive, pinned, with keyed role-tagged messages", () => {
    const view = sarahSurfaceView(baseState)
    const transcript = findByTag(view, "Transcript")
    expect(transcript).not.toBeNull()
    expect(transcript?.pinToEnd).toBe(true)
    const messages = transcript?.messages as ReadonlyArray<{
      key: string
      role: string
      body: ReadonlyArray<{ _tag: string }>
    }>
    expect(messages.length).toBe(1)
    expect(messages[0]?.key).toBe("welcome")
    expect(messages[0]?.role).toBe("assistant")
    // The message body keeps the Card visual of the previous List+Card shell.
    expect(messages[0]?.body[0]?._tag).toBe("Card")
  })

  test("an open avatar session mounts the media-video host attach target", () => {
    const pane = sarahAvatarPaneView({ ...baseState, avatarSessionOpen: true })
    const media = findByTag(pane, "Host")
    expect(media).not.toBeNull()
    expect(media?.kind).toBe("media-video")
    expect(findByKey(pane, "avatar-overlay")).not.toBeNull()
  })

  test("with no open session the avatar pane renders no media host but keeps EN overlay controls", () => {
    const pane = sarahAvatarPaneView(baseState)
    expect(findByTag(pane, "Host")).toBeNull()
    expect(findByKey(pane, "avatar-start-overlay")).not.toBeNull()
  })
})

describe("contract sarah.split_screen_blueprint_map.v1 (BM-3 #8629)", () => {
  test("the right pane is an Effect Native Tabs surface with Blueprint map default", () => {
    const view = sarahSurfaceView(baseState)
    const tabs = findByTag(view, "Tabs") as
      | {
          selectedId?: string
          keepMounted?: boolean
          tabs?: ReadonlyArray<{ id: string; label: string }>
          panels?: ReadonlyArray<{ id: string }>
        }
      | null
    expect(tabs).not.toBeNull()
    expect(tabs?.selectedId).toBe("blueprint")
    expect(tabs?.keepMounted).toBe(true)
    expect(tabs?.tabs?.map((tab) => tab.label)).toEqual([
      "Blueprint map",
      "Chat",
      "Actions",
      "Receipts",
    ])
    expect(tabs?.panels?.map((panel) => panel.id)).toEqual([
      "blueprint",
      "chat",
      "actions",
      "receipts",
    ])
    const graph = findByTag(view, "GraphFigure") as
      | { nodes?: ReadonlyArray<{ id: string }>; edges?: ReadonlyArray<{ id: string }> }
      | null
    expect(graph).not.toBeNull()
    expect(graph?.nodes?.some((node) => node.id === "prospect")).toBe(true)
    expect(graph?.edges?.some((edge) => edge.id === "edge:prospect:account")).toBe(true)
  })

  test("the cut list is absent from the EN surface tree", () => {
    const serialized = JSON.stringify(sarahSurfaceView(baseState))
    expect(serialized).not.toContain("OpenAgents sales · openagents.com/sarah")
    expect(serialized).not.toContain("avatar-controls")
    expect(serialized).not.toContain('"key":"title"')
    expect(findByKey(sarahSurfaceView(baseState), "sarah-toolbar")).not.toBeNull()
  })

  test("the transcript, composer, actions, and receipts stay inside tab panels", () => {
    const view = sarahSurfaceView({
      ...baseState,
      cards: [{ key: "receipt-1", title: "Receipt", body: "Tool call recorded" }],
    })
    expect(findByKey(view, "chat-panel")).not.toBeNull()
    expect(findByKey(view, "composer")).not.toBeNull()
    expect(findByKey(view, "actions-panel")).not.toBeNull()
    expect(findByKey(view, "actions-book-human")).not.toBeNull()
    expect(findByKey(view, "actions-checkout-empty")).not.toBeNull()
    expect(findByKey(view, "receipts-panel")).not.toBeNull()
    expect(findByKey(view, "receipts-cards")).not.toBeNull()
    expect(findByKey(view, "cards")).toBeNull()
    expect(findAllByTag(view, "List").length).toBe(1)
  })

  test("BM-4 actions expose recorded checkout and receipts show Blueprint code", () => {
    const view = sarahSurfaceView({
      ...baseState,
      accountPhase: "linked",
      accountEmail: "buyer@example.com",
      activePanel: "receipts",
      blueprintDraft: {
        schema: "sarah.customer_blueprint_draft.v1",
        prospectRef: "prospect-actions",
        revision: 7,
        createdAt: "2026-07-09T17:00:00.000Z",
        business: { facts: [] },
        contacts: { email: "buyer@example.com", contactId: "oa_user:buyer" },
        needs: [],
        suggestedModules: [],
        sources: {
          turnIds: [],
          factCount: 0,
          provenance:
            "sarah_prospect_profile + sarah_transcript_turns (per-fact source turn ids)",
        },
        handoff: {
          pipeline: "operator_assisted_business_workspace",
          automatedProvisioning: false,
          convergesWith:
            "CB-1.4 prefill pipeline (intake -> public-data research -> seeded workspace)",
          note: "Draft only.",
        },
      },
      receipts: [
        {
          id: "tool:sarah.checkout.test",
          key: "receipt-tool-sarah.checkout.test",
          title: "Checkout link recorded",
          body: "Prepared a test-mode checkout quote.",
          href: "https://openagents.com/business",
          toolName: "checkout_link_create",
          receiptRef: "sarah.checkout.test",
          mode: "dry_run",
          ok: true,
        },
      ],
    })
    expect(findByKey(view, "actions-account-linked")).not.toBeNull()
    expect(findByKey(view, "actions-open-checkout")).not.toBeNull()
    expect(findByKey(view, "receipts-blueprint-code")).not.toBeNull()
    expect(JSON.stringify(view)).toContain("sarah.customer_blueprint_draft.v1")
  })
})
