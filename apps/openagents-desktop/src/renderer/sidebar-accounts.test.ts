/**
 * Sidebar connected-accounts box tests (EP250 #8712).
 *
 * Owner statement (verbatim): "in the left sidebar, in a bottom box, like
 * letting the chats flex up but show up to 5 connected accounts with a
 * progress bar showing remaining weekly/hourly usage (grayed out if we dont
 * have that data)."
 */
import { describe, expect, test } from "vite-plus/test"
import type { View } from "@effect-native/core"
import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"

import { openAgentsDesktopUxContractRegistry } from "../contracts/ux-contracts.ts"
import {
  emptyFleetWorkspaceState,
  type FleetAccount,
  type FleetUsageWindow,
  type FleetWorkspaceState,
} from "./fleet-workspace.ts"
import {
  sidebarAccountBarModel,
  sidebarAccountsCap,
  sidebarAccountsNoDataReason,
  sidebarAccountsView,
  sidebarProviderIcon,
  truncateSidebarAccountRef,
} from "./sidebar-accounts.ts"

type AnyNode = Readonly<Record<string, unknown>>

const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: Array<AnyNode> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) {
      for (const item of value) walk(item)
      return
    }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag" || prop === "style" || prop === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}

const nodeByKey = (view: View | null, key: string): AnyNode | undefined =>
  view === null ? undefined : collectNodes(view).find((node) => node.key === key)

const account = (
  ref: string,
  provider = "codex",
  readiness: FleetAccount["readiness"] = "ready",
): FleetAccount => ({ ref, provider, email: null, readiness })

const codexWindows: ReadonlyArray<FleetUsageWindow> = [
  { label: "5h", usedPercent: 63, remainingPercent: 37, windowMinutes: 300, resetsAt: "2026-07-11T03:00:00.000Z" },
  { label: "weekly", usedPercent: 18, remainingPercent: 82, windowMinutes: 10080, resetsAt: "2026-07-15T00:00:00.000Z" },
]

const fleetWith = (
  accounts: ReadonlyArray<FleetAccount>,
  usage: FleetWorkspaceState["usage"] = {},
): FleetWorkspaceState => ({
  ...emptyFleetWorkspaceState(),
  phase: "ready",
  generatedAt: "2026-07-11T12:00:00.000Z",
  accounts,
  usage,
})

describe("contract registration (EP250 sidebar accounts box)", () => {
  test("the verbatim owner statement is registered and enforced", () => {
    expect(validateBehaviorContractRegistry(openAgentsDesktopUxContractRegistry).ok).toBe(true)
    const contract = openAgentsDesktopUxContractRegistry.contracts.find(
      (entry) => entry.contractId === "openagents_desktop.sidebar.connected_accounts_usage_box.v1",
    )
    expect(contract?.state).toBe("retired")
    expect(contract?.statement).toBe(
      "in the left sidebar, in a bottom box, like letting the chats flex up but show up to 5 connected accounts with a progress bar showing remaining weekly/hourly usage (grayed out if we dont have that data).",
    )
  })
})

describe("sidebarAccountsView structure", () => {
  test("zero connected accounts render no box at all", () => {
    expect(sidebarAccountsView(emptyFleetWorkspaceState())).toBeNull()
    expect(sidebarAccountsView(fleetWith([]))).toBeNull()
  })

  test("the box is hairline-topped and uses a collapsed Effect Native accordion by default", () => {
    const view = sidebarAccountsView(fleetWith([account("codex"), account("claude-1", "claude_agent")]))
    expect(view).not.toBeNull()
    const nodes = collectNodes(view)
    expect(nodes[0]?.key).toBe("sidebar-accounts")
    // The hairline divider is the FIRST child inside the box (top edge).
    const divider = nodeByKey(view, "sidebar-accounts-hairline")
    expect(divider?._tag).toBe("Divider")
    const disclosure = nodeByKey(view, "sidebar-accounts-disclosure") as {
      _tag?: string
      expandedIds?: ReadonlyArray<string>
      onToggle?: { name?: string }
      items?: ReadonlyArray<{ header?: string }>
    }
    expect(disclosure?._tag).toBe("Accordion")
    expect(disclosure?.expandedIds).toEqual([])
    expect(disclosure?.onToggle?.name).toBe("DesktopSidebarAccountsToggled")
    expect(disclosure?.items?.[0]?.header).toBe("Accounts · 2")
    expect(nodeByKey(view, "sidebar-account-codex")).toBeDefined()
    expect(nodeByKey(view, "sidebar-account-claude-1")).toBeDefined()
    expect((nodeByKey(sidebarAccountsView(fleetWith([account("codex")]), true), "sidebar-accounts-disclosure") as { expandedIds?: ReadonlyArray<string> }).expandedIds).toEqual(["accounts"])
  })

  test("shows at most 5 accounts with a dim '+N more' row deep-linking to the Fleet workspace", () => {
    const accounts = ["a-1", "a-2", "a-3", "a-4", "a-5", "a-6", "a-7"].map((ref) => account(ref))
    const view = sidebarAccountsView(fleetWith(accounts))
    const rows = collectNodes(view).filter(
      (node) => typeof node.key === "string" && /^sidebar-account-a-\d$/.test(node.key as string),
    )
    expect(rows).toHaveLength(sidebarAccountsCap)
    const more = nodeByKey(view, "sidebar-accounts-more") as {
      label?: string
      variant?: string
      onPress?: { name?: string; payload?: { value?: unknown } }
      style?: Record<string, unknown>
    }
    expect(more?.label).toBe("+2 more")
    expect(more?.variant).toBe("ghost")
    expect(more?.style).toMatchObject({ color: "textFaint", typeScale: "caption" })
    expect(more?.onPress?.name).toBe("DesktopWorkspaceSelected")
    expect(more?.onPress?.payload?.value).toBe("fleet")
    // Exactly five accounts render no overflow row.
    const exact = sidebarAccountsView(fleetWith(accounts.slice(0, 5)))
    expect(nodeByKey(exact, "sidebar-accounts-more")).toBeUndefined()
  })

  test("ordering is the fleet evidence order: ready first, then provider, then ref", () => {
    const view = sidebarAccountsView(fleetWith([
      account("codex-9", "codex", "credentials-missing"),
      account("grok-1", "grok", "ready"),
      account("codex-1", "codex", "ready"),
      account("claude-1", "claude_agent", "ready"),
    ]))
    const order = collectNodes(view)
      .map((node) => node.key)
      .filter((key): key is string => typeof key === "string" && /^sidebar-account-(codex|grok|claude)/.test(key))
    expect(order).toEqual([
      "sidebar-account-claude-1",
      "sidebar-account-codex-1",
      "sidebar-account-grok-1",
      "sidebar-account-codex-9",
    ])
  })

  test("provider glyphs come from the existing catalog icon set", () => {
    expect(sidebarProviderIcon("codex")).toBe("Code")
    expect(sidebarProviderIcon("claude_agent")).toBe("Sparkles")
    expect(sidebarProviderIcon("grok")).toBe("Agent")
    expect(sidebarProviderIcon("something-else")).toBe("Circle")
    const view = sidebarAccountsView(fleetWith([account("codex")]))
    const icon = nodeByKey(view, "sidebar-account-icon-codex") as { name?: string; size?: string }
    expect(icon?.name).toBe("Code")
    expect(icon?.size).toBe("sm")
  })

  test("long refs render bounded", () => {
    const longRef = "codex-with-a-very-long-account-reference-name"
    expect(truncateSidebarAccountRef(longRef)).toBe(`${longRef.slice(0, 23)}…`)
    expect(truncateSidebarAccountRef("codex")).toBe("codex")
    const view = sidebarAccountsView(fleetWith([account(longRef)]))
    const text = nodeByKey(view, `sidebar-account-ref-${longRef}`) as { content?: string }
    expect(text?.content).toBe(`${longRef.slice(0, 23)}…`)
  })
})

describe("usage bar evidence", () => {
  test("measured bar: Meter value is the tightest window's remaining share with every window in the label", () => {
    const fleet = fleetWith([account("codex")], {
      codex: {
        state: "checked",
        refreshedAt: "2026-07-11T12:00:00.000Z",
        inputTokens: 1200,
        outputTokens: 340,
        totalTokens: 1540,
        windows: codexWindows,
      },
    })
    const model = sidebarAccountBarModel(fleet, fleet.accounts[0]!)
    if (model.kind !== "measured") throw new Error("expected a measured bar")
    expect(model.remainingRatio).toBeCloseTo(0.37, 5)
    expect(model.window.label).toBe("5h")
    expect(model.detail).toBe("37% 5h remaining · 82% weekly remaining")
    const view = sidebarAccountsView(fleet)
    const meter = nodeByKey(view, "sidebar-account-bar-codex") as {
      _tag?: string
      value?: number
      tone?: string
      label?: string
      style?: Record<string, unknown>
    }
    expect(meter?._tag).toBe("Meter")
    expect(meter?.value).toBeCloseTo(0.37, 5)
    expect(meter?.tone).toBe("info")
    expect(meter?.label).toBe("codex: 37% 5h remaining · 82% weekly remaining")
    // Thin token-styled track (state token, hairline-adjacent height).
    expect(meter?.style).toMatchObject({ backgroundColor: "stateSelected", borderRadius: "full", width: "full" })
    // No grayed tooltip wraps a measured bar.
    expect(nodeByKey(view, "sidebar-account-bar-reason-codex")).toBeUndefined()
  })

  test("the weekly window rules the bar when it is the tighter one", () => {
    const fleet = fleetWith([account("codex")], {
      codex: {
        state: "checked",
        refreshedAt: "2026-07-11T12:00:00.000Z",
        inputTokens: null,
        outputTokens: null,
        totalTokens: null,
        windows: [
          { label: "5h", usedPercent: 10, remainingPercent: 90, windowMinutes: 300, resetsAt: null },
          { label: "weekly", usedPercent: 95, remainingPercent: 5, windowMinutes: 10080, resetsAt: null },
        ],
      },
    })
    const model = sidebarAccountBarModel(fleet, fleet.accounts[0]!)
    if (model.kind !== "measured") throw new Error("expected a measured bar")
    expect(model.window.label).toBe("weekly")
    expect(model.remainingRatio).toBeCloseTo(0.05, 5)
  })

  test("no usage entry, failed probes, checking, and windowless checks all render the grayed bar with the honest reason", () => {
    const accounts = [account("codex")]
    const noEntry = fleetWith(accounts)
    const checking = fleetWith(accounts, { codex: { state: "checking" } })
    const failed = fleetWith(accounts, { codex: { state: "failed", reason: "account_rate_limited" } })
    const windowless = fleetWith(accounts, {
      codex: {
        state: "checked",
        refreshedAt: "2026-07-11T12:00:00.000Z",
        inputTokens: 1,
        outputTokens: 2,
        totalTokens: 3,
      },
    })
    for (const fleet of [noEntry, checking, failed, windowless]) {
      const model = sidebarAccountBarModel(fleet, fleet.accounts[0]!)
      expect(model).toEqual({ kind: "no-data", reason: sidebarAccountsNoDataReason })
      const view = sidebarAccountsView(fleet)
      const tooltip = nodeByKey(view, "sidebar-account-bar-reason-codex") as {
        _tag?: string
        content?: string
      }
      expect(tooltip?._tag).toBe("Tooltip")
      expect(tooltip?.content).toBe(sidebarAccountsNoDataReason)
      const meter = nodeByKey(view, "sidebar-account-bar-codex") as {
        value?: number
        tone?: string
        label?: string
        style?: Record<string, unknown>
      }
      expect(meter?.value).toBe(0)
      expect(meter?.tone).toBe("neutral")
      expect(meter?.label).toBe(`codex: ${sidebarAccountsNoDataReason}`)
      expect(meter?.style).toMatchObject({ backgroundColor: "borderSubtle", opacity: 0.45 })
    }
  })
})
