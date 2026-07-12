/**
 * Fleet workspace unit tests (#8712): pure state -> expected component tree
 * for the loading / ready / unavailable phases, the Arbiter lit-dot evidence
 * rule (lit ONLY on decoded ready evidence), the read-only table, and the
 * typed intent loop run headlessly through the real registry.
 */
import { describe, expect, test } from "bun:test"
import { IntentRef, StaticPayload, resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  decodeFleetAccountsProjection,
  decodeFleetUsageEntry,
  emptyFleetWorkspaceState,
  fleetControlLaneForProvider,
  fleetDotEvidence,
  fleetReconnectRequired,
  fleetWorkspaceIntents,
  fleetWorkspaceView,
  formatFleetLocalTime,
  makeFleetWorkspaceHandlers,
  sortFleetAccounts,
  withFleetLedger,
  withFleetLoading,
  withFleetProjection,
  withFleetUsageChecking,
  withFleetUsageEntry,
  type FleetAccount,
  type FleetWorkspaceState,
} from "./fleet-workspace.ts"
import type { UsageLedgerSnapshot } from "../usage-ledger-contract.ts"
import type { FleetCockpitCard } from "../fleet-cockpit.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

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

const nodeByKey = (view: View, key: string): AnyNode | undefined =>
  collectNodes(view).find((node) => node.key === key)

const readyAccount: FleetAccount = {
  ref: "codex",
  provider: "codex",
  email: "owner@example.com",
  readiness: "ready",
}
const revokedAccount: FleetAccount = {
  ref: "codex-2",
  provider: "codex",
  email: null,
  readiness: "credentials-missing",
}
const unknownAccount: FleetAccount = {
  ref: "claude-pylon-3",
  provider: "claude_agent",
  email: null,
  readiness: "unknown",
}

const readyState: FleetWorkspaceState = withFleetProjection(emptyFleetWorkspaceState(), {
  ok: true,
  generatedAt: "2026-07-11T12:00:00.000Z",
  accounts: [readyAccount, revokedAccount, unknownAccount],
})

const runningCockpitCard: FleetCockpitCard = {
  threadRef: "thread.demo",
  title: "Ship the portable session",
  authority: "live",
  runRef: "run.demo",
  runVersion: 7,
  status: "running",
  provider: "codex",
  workContextRef: "worktree.demo",
  repositoryRef: "repo.openagents",
  agentRefs: ["agent.builder"],
  receiptRefs: ["receipt.test"],
  attention: [{
    interactionRef: "interaction.approval",
    turnRef: "run.demo",
    version: 3,
    kind: "tool_approval",
    title: "Approve filesystem write",
    actions: ["approve", "deny"],
  }],
  actions: ["pause", "cancel"],
}

describe("fleetWorkspaceView (state -> component tree)", () => {
  test("loading phase shows the honest loading copy and a disabled refresh", () => {
    const view = fleetWorkspaceView(withFleetLoading(emptyFleetWorkspaceState()))
    expect(nodeByKey(view, "fleet-loading")?._tag).toBe("Text")
    expect(nodeByKey(view, "fleet-refresh")?.disabled).toBe(true)
    expect(nodeByKey(view, "fleet-as-of")?.content).toBe("no snapshot yet")
    expect(nodeByKey(view, "fleet-accounts-table")).toBeUndefined()
  })

  test("unavailable phase renders an explicit failure with the public-safe reason", () => {
    const state = withFleetProjection(emptyFleetWorkspaceState(), {
      ok: false,
      reason: "pylon_runtime_unavailable",
    })
    const view = fleetWorkspaceView(state)
    expect(nodeByKey(view, "fleet-unavailable")?._tag).toBe("Text")
    expect(nodeByKey(view, "fleet-unavailable-reason")?.content).toBe("pylon_runtime_unavailable")
    expect(nodeByKey(view, "fleet-accounts-table")).toBeUndefined()
    expect(collectNodes(view).filter((node) => node._tag === "Icon" && node.color === "success")).toHaveLength(0)
  })

  test("ready phase renders the as-of caption, dots, table rows, and footer intents", () => {
    const view = fleetWorkspaceView(readyState)
    expect(nodeByKey(view, "fleet-title")?.content).toBe("Fleet")
    // Rendered in local wall-clock time (the owner saw a UTC clock drift).
    expect(nodeByKey(view, "fleet-as-of")?.content).toBe(
      `as of ${formatFleetLocalTime("2026-07-11T12:00:00.000Z")}`,
    )
    expect((nodeByKey(view, "fleet-refresh") as { onPress?: { name?: string } }).onPress?.name).toBe("FleetRefreshRequested")

    const table = nodeByKey(view, "fleet-accounts-table")
    expect(table?._tag).toBe("Table")
    // Deterministic order: ready accounts first, then provider, then ref —
    // the same order the dots use.
    expect((table?.rows as Array<{ id: string }>).map((row) => row.id)).toEqual(["codex", "claude-pylon-3", "codex-2"])
    const columns = table?.columns as Array<{ id: string; align?: string }>
    expect(columns.find((column) => column.id === "usage")?.align).toBe("end")
    expect(nodeByKey(view, "fleet-email-codex")?.content).toBe("owner@example.com")
    expect(nodeByKey(view, "fleet-email-codex-2")?.content).toBe("—")
    expect(nodeByKey(view, "fleet-email-codex-2")?.color).toBe("textMuted")
    expect(nodeByKey(view, "fleet-readiness-codex")?._tag).toBe("Badge")
    expect(nodeByKey(view, "fleet-readiness-codex")?.tone).toBe("success")
    expect(nodeByKey(view, "fleet-readiness-codex-2")?.tone).toBe("warn")
    expect(nodeByKey(view, "fleet-readiness-claude-pylon-3")?.tone).toBe("neutral")

    const check = nodeByKey(view, "fleet-usage-check-codex") as { onPress?: { name?: string } }
    expect(check.onPress?.name).toBe("FleetUsageCheckRequested")
    expect(JSON.stringify(check.onPress)).toContain("codex")

    const manage = nodeByKey(view, "fleet-manage-accounts") as { onPress?: { name?: string } }
    expect(manage.onPress?.name).toBe("DesktopSettingsToggled")
    const newChat = nodeByKey(view, "fleet-new-chat") as { onPress?: { name?: string } }
    expect(newChat.onPress?.name).toBe("DesktopNewChat")
  })

  test("authoritative work exposes exact run identity and opens its canonical conversation", () => {
    const state = {
      ...readyState,
      cockpitAuthority: "live" as const,
      cockpitCards: [runningCockpitCard],
    }
    const view = fleetWorkspaceView(state)
    expect(nodeByKey(view, "fleet-cockpit-title")?.content).toBe("Authoritative work")
    expect(nodeByKey(view, "fleet-cockpit-run.demo-title")?.content).toBe("Ship the portable session")
    expect(nodeByKey(view, "fleet-cockpit-run.demo-status")?.label).toBe("running")
    expect(nodeByKey(view, "fleet-cockpit-run.demo-refs")?.content).toBe(
      "worktree.demo · repo.openagents · agent.builder · receipt.test",
    )
    expect(nodeByKey(view, "fleet-cockpit-run.demo-attention")?.content).toBe("1 item needs attention")
    const open = nodeByKey(view, "fleet-cockpit-run.demo-open") as { onPress?: { name?: string } }
    expect(open.onPress?.name).toBe("DesktopChatSelected")
    expect(JSON.stringify(open.onPress)).toContain("thread.demo")
    expect((nodeByKey(view, "fleet-cockpit-run.demo-pause") as { onPress?: { name?: string } }).onPress?.name).toBe("FleetRunControlRequested")
    expect((nodeByKey(view, "fleet-cockpit-run.demo-cancel") as { onPress?: { name?: string } }).onPress?.name).toBe("FleetRunControlRequested")
    expect((nodeByKey(view, "fleet-cockpit-run.demo-interaction.approval-approve") as { onPress?: { name?: string } }).onPress?.name).toBe("FleetAttentionDecisionRequested")
    expect((nodeByKey(view, "fleet-cockpit-run.demo-interaction.approval-deny") as { onPress?: { name?: string } }).onPress?.name).toBe("FleetAttentionDecisionRequested")
  })

  test("non-live cockpit authority states honestly that controls are withheld", () => {
    const view = fleetWorkspaceView({
      ...readyState,
      cockpitAuthority: "stale",
      cockpitCards: [],
    })
    expect(nodeByKey(view, "fleet-cockpit-empty")?.content).toBe("Run authority stale; controls withheld.")
  })

  test("dots are a vertical flow of chips in the deterministic sort order (no horizontal strip)", () => {
    const view = fleetWorkspaceView(readyState)
    const dots = nodeByKey(view, "fleet-status-dots") as { direction?: string; children?: Array<AnyNode> }
    expect(dots.direction).toBe("column")
    expect((dots.children ?? []).map((chip) => chip.key)).toEqual([
      "fleet-dot-codex",
      "fleet-dot-claude-pylon-3",
      "fleet-dot-codex-2",
    ])
    // Chip anatomy: ref on the line, provider small and dim beside it.
    expect(nodeByKey(view, "fleet-dot-label-codex")?.content).toBe("codex")
    expect(nodeByKey(view, "fleet-dot-label-codex")?.color).toBe("textPrimary")
    expect(nodeByKey(view, "fleet-dot-provider-claude-pylon-3")?.content).toBe("claude_agent")
    expect(nodeByKey(view, "fleet-dot-provider-claude-pylon-3")?.color).toBe("textMuted")
  })

  test("sortFleetAccounts: ready first, then provider, then ref", () => {
    expect(sortFleetAccounts([unknownAccount, revokedAccount, readyAccount]).map((account) => account.ref))
      .toEqual(["codex", "claude-pylon-3", "codex-2"])
  })

  test("formatFleetLocalTime renders the local wall clock for an ISO instant", () => {
    const instant = new Date("2026-07-11T16:08:44.000Z")
    const expected = `${String(instant.getHours()).padStart(2, "0")}:${String(instant.getMinutes()).padStart(2, "0")}:44`
    expect(formatFleetLocalTime("2026-07-11T16:08:44.000Z")).toBe(expected)
    expect(formatFleetLocalTime("2026-07-11T16:08:44.000Z", false)).toBe(expected.slice(0, 5))
    // Unparseable input degrades to the bounded raw string, never NaN text.
    expect(formatFleetLocalTime("not-a-time")).toBe("not-a-time")
  })

  test("lit-dot rule: only decoded ready evidence lights a dot", () => {
    expect(fleetDotEvidence(readyState, readyAccount)).toBe("lit")
    expect(fleetDotEvidence(readyState, revokedAccount)).toBe("unlit")
    expect(fleetDotEvidence(readyState, unknownAccount)).toBe("evidence-unavailable")
    // Without a successfully decoded projection this session there is no lit
    // dot, whatever the account claims.
    expect(fleetDotEvidence({ ...readyState, phase: "loading" }, readyAccount)).toBe("evidence-unavailable")

    const view = fleetWorkspaceView(readyState)
    expect(nodeByKey(view, "fleet-dot-icon-codex")?.color).toBe("success")
    expect(nodeByKey(view, "fleet-dot-icon-codex-2")?.color).toBe("textMuted")
    expect(nodeByKey(view, "fleet-dot-icon-claude-pylon-3")?.color).toBe("textMuted")
    expect(nodeByKey(view, "fleet-dot-evidence-claude-pylon-3")?.content).toBe("evidence unavailable")
    expect(nodeByKey(view, "fleet-dot-evidence-codex")).toBeUndefined()
    expect(nodeByKey(view, "fleet-dot-evidence-codex-2")).toBeUndefined()
  })

  test("usage cells move through — / checking / checked / failed without optimism", () => {
    const idleCheck = nodeByKey(fleetWorkspaceView(readyState), "fleet-usage-check-codex")
    expect(nodeByKey(fleetWorkspaceView(readyState), "fleet-usage-empty-codex")?.content).toBe("—")
    expect(idleCheck?.label).toBe("Check")
    expect(idleCheck?.disabled).toBe(false)

    // In flight: the same button stays visible, disabled, labeled Checking….
    const checking = withFleetUsageChecking(readyState, "codex")
    const checkingButton = nodeByKey(fleetWorkspaceView(checking), "fleet-usage-check-codex")
    expect(checkingButton?._tag).toBe("Button")
    expect(checkingButton?.label).toBe("Checking…")
    expect(checkingButton?.disabled).toBe(true)

    const checked = withFleetUsageEntry(readyState, "codex", {
      state: "checked",
      refreshedAt: "2026-07-11T12:05:00.000Z",
      inputTokens: 1200,
      outputTokens: 340,
      totalTokens: 1540,
    })
    // Honest labeling: the number is the minimal `--refresh` probe turn, so
    // it carries the split, the probe tag, and the local refresh time.
    expect(nodeByKey(fleetWorkspaceView(checked), "fleet-usage-total-codex")?.content).toBe(
      `1,540 tokens · in 1,200 / out 340 · probe · ${formatFleetLocalTime("2026-07-11T12:05:00.000Z", false)}`,
    )

    const checkedNoSplit = withFleetUsageEntry(readyState, "codex", {
      state: "checked",
      refreshedAt: "2026-07-11T12:05:00.000Z",
      inputTokens: null,
      outputTokens: null,
      totalTokens: 8,
    })
    expect(nodeByKey(fleetWorkspaceView(checkedNoSplit), "fleet-usage-total-codex")?.content).toBe(
      `8 tokens · probe · ${formatFleetLocalTime("2026-07-11T12:05:00.000Z", false)}`,
    )

    const checkedEmpty = withFleetUsageEntry(readyState, "codex", {
      state: "checked",
      refreshedAt: "2026-07-11T12:05:00.000Z",
      inputTokens: null,
      outputTokens: null,
      totalTokens: null,
    })
    expect(nodeByKey(fleetWorkspaceView(checkedEmpty), "fleet-usage-total-codex")?.content).toBe(
      `no usage recorded · probe · ${formatFleetLocalTime("2026-07-11T12:05:00.000Z", false)}`,
    )

    const failed = withFleetUsageEntry(readyState, "codex", {
      state: "failed",
      reason: "projection_timeout",
    })
    const failedView = fleetWorkspaceView(failed)
    expect(nodeByKey(failedView, "fleet-usage-failed-codex")?.content).toBe(
      "evidence unavailable · projection_timeout",
    )
    expect(nodeByKey(failedView, "fleet-usage-check-codex")?._tag).toBe("Button")
  })

  test("the view carries no mutation intents (read-only surface)", () => {
    const intentNames = collectNodes(fleetWorkspaceView(readyState))
      .flatMap((node) => [node.onPress, node.onSelect, node.onChange, node.onSubmit])
      .flatMap((ref) => (typeof ref === "object" && ref !== null && "name" in ref ? [(ref as { name: string }).name] : []))
    expect(new Set(intentNames)).toEqual(new Set([
      "FleetRefreshRequested",
      "FleetUsageCheckRequested",
      "DesktopSettingsToggled",
      "DesktopNewChat",
    ]))
  })
})

// ---------------------------------------------------------------------------
// Session usage ledger + readiness honesty (#8712 Lane C)
// ---------------------------------------------------------------------------

const ledgerSnapshot: UsageLedgerSnapshot = {
  ok: true,
  generatedAt: "2026-07-11T12:10:00.000Z",
  evidence: "session ledger",
  rows: [
    {
      accountRef: "codex",
      provider: "codex",
      requestedModel: "gpt-5.6-sol",
      turns: 0,
      children: 0,
      inputTokens: 0,
      cachedInputTokens: 0,
      outputTokens: 0,
      reasoningTokens: 0,
      totalTokens: 0,
      reconnectRequired: true,
      updatedAt: "2026-07-11T12:10:00.000Z",
    },
    {
      accountRef: "codex-2",
      provider: "codex",
      requestedModel: "gpt-5.6-sol",
      turns: 0,
      children: 1,
      inputTokens: 1200,
      cachedInputTokens: 900,
      outputTokens: 180,
      reasoningTokens: 60,
      totalTokens: 1440,
      reconnectRequired: false,
      updatedAt: "2026-07-11T12:10:00.000Z",
    },
  ],
}

describe("session usage ledger section + readiness honesty override", () => {
  const withLedger = withFleetLedger(readyState, ledgerSnapshot)

  test("renders the evidence-labeled Session usage section with per-account rows and the requested codex model", () => {
    const view = fleetWorkspaceView(withLedger)
    expect(nodeByKey(view, "fleet-session-usage")).toBeDefined()
    expect(nodeByKey(view, "fleet-session-usage-evidence")?.content).toBe(
      `session ledger · as of ${formatFleetLocalTime("2026-07-11T12:10:00.000Z")}`,
    )
    expect(nodeByKey(view, "fleet-ledger-total-codex-2")?.content).toBe("1,440 tokens")
    expect(nodeByKey(view, "fleet-ledger-counts-codex-2")?.content).toBe("0 turn(s) · 1 child(ren)")
    // Codex rows carry the requested model as spawn-config truth.
    expect(nodeByKey(view, "fleet-ledger-model-codex-2")?.content).toBe("gpt-5.6-sol (requested)")
    expect(nodeByKey(view, "fleet-ledger-reconnect-codex")?.content).toBe("reconnect required")
    expect(nodeByKey(view, "fleet-ledger-reconnect-codex-2")).toBeUndefined()
  })

  test("no ledger evidence renders no Session usage section (never an empty optimistic block)", () => {
    expect(nodeByKey(fleetWorkspaceView(readyState), "fleet-session-usage")).toBeUndefined()
    const emptyRows = withFleetLedger(readyState, { ...ledgerSnapshot, rows: [] })
    expect(nodeByKey(fleetWorkspaceView(emptyRows), "fleet-session-usage")).toBeUndefined()
  })

  test("withFleetLedger refuses an invalid snapshot and keeps prior evidence", () => {
    expect(withFleetLedger(withLedger, { ok: false }).ledger).toEqual(ledgerSnapshot)
    expect(withFleetLedger(withLedger, null).ledger).toEqual(ledgerSnapshot)
  })

  test("READINESS HONESTY: a ledger reconnectRequired row supersedes presence-based ready in dot and cell", () => {
    // Upstream projection says codex is "ready" (auth.json presence only)…
    expect(readyAccount.readiness).toBe("ready")
    // …but the session observed a revoked credential: typed override.
    expect(fleetReconnectRequired(withLedger, readyAccount)).toBe(true)
    expect(fleetDotEvidence(withLedger, readyAccount)).toBe("reconnect-required")
    const view = fleetWorkspaceView(withLedger)
    expect(nodeByKey(view, "fleet-readiness-codex")?.label).toBe("reconnect required")
    expect(nodeByKey(view, "fleet-readiness-codex")?.tone).toBe("warn")
    expect(nodeByKey(view, "fleet-dot-icon-codex")?.color).toBe("warning")
    expect(nodeByKey(view, "fleet-dot-reconnect-codex")?.content).toBe("reconnect required")
    // Accounts without reconnect evidence keep their projection readiness.
    expect(fleetDotEvidence(withLedger, revokedAccount)).toBe("unlit")
    expect(nodeByKey(view, "fleet-readiness-codex-2")?.label).toBe("credentials-missing")
  })

  test("EP250: a SUCCESSFUL probe this session clears a stale ledger reconnect flag (probe evidence rules, both directions)", () => {
    // The ledger marked codex reconnect-required earlier in the session…
    expect(fleetReconnectRequired(withLedger, readyAccount)).toBe(true)
    // …then a UI reconnect happened and a fresh probe SUCCEEDED: the fresher
    // real-auth evidence supersedes the sticky session flag.
    const recovered = withFleetUsageEntry(withLedger, "codex", {
      state: "checked",
      refreshedAt: "2026-07-11T12:20:00.000Z",
      inputTokens: 10,
      outputTokens: 2,
      totalTokens: 12,
    })
    expect(fleetReconnectRequired(recovered, readyAccount)).toBe(false)
    expect(fleetDotEvidence(recovered, readyAccount)).toBe("lit")
    expect(nodeByKey(fleetWorkspaceView(recovered), "fleet-readiness-codex")?.label).toBe("ready")
    // A merely pending probe does NOT clear it.
    expect(fleetReconnectRequired(withFleetUsageChecking(withLedger, "codex"), readyAccount)).toBe(true)
  })

  test("EP250: broken-credential rows carry the Fix in Settings navigation (no account mutation from Fleet)", () => {
    const view = fleetWorkspaceView(withLedger)
    // reconnect-required row (ledger override on presence-ready codex)
    const fix = nodeByKey(view, "fleet-fix-codex") as { label?: string; onPress?: { name?: string } }
    expect(fix?.label).toBe("Fix in Settings")
    expect(fix?.onPress?.name).toBe("DesktopSettingsToggled")
    // credentials-missing row also deep-links
    const fixMissing = nodeByKey(view, "fleet-fix-codex-2") as { label?: string; onPress?: { name?: string } }
    expect(fixMissing?.label).toBe("Fix in Settings")
    expect(fixMissing?.onPress?.name).toBe("DesktopSettingsToggled")
    // a healthy ready row gets no fix affordance
    const healthy = fleetWorkspaceView(readyState)
    expect(nodeByKey(healthy, "fleet-fix-codex")).toBeUndefined()
  })

  test("READINESS HONESTY: a FAILED usage probe also supersedes presence-based ready", () => {
    const probeFailed = withFleetUsageEntry(readyState, "codex", {
      state: "failed",
      reason: "accounts_command_failed",
    })
    expect(fleetReconnectRequired(probeFailed, readyAccount)).toBe(true)
    expect(fleetDotEvidence(probeFailed, readyAccount)).toBe("reconnect-required")
    expect(nodeByKey(fleetWorkspaceView(probeFailed), "fleet-readiness-codex")?.label)
      .toBe("reconnect required")
    // A merely pending or successful probe never triggers the override.
    expect(fleetReconnectRequired(withFleetUsageChecking(readyState, "codex"), readyAccount)).toBe(false)
  })

  test("the ledger survives an accounts refresh (session truth, not projection evidence)", () => {
    const refreshed = withFleetProjection(withLedger, {
      ok: true,
      generatedAt: "2026-07-11T13:00:00.000Z",
      accounts: [readyAccount],
    })
    expect(refreshed.ledger).toEqual(ledgerSnapshot)
  })
})

describe("renderer bridge decode", () => {
  test("list decode drops out-of-grammar refs and bounds strings", () => {
    const decoded = decodeFleetAccountsProjection({
      ok: true,
      generatedAt: "2026-07-11T12:00:00.000Z",
      accounts: [
        { ref: "codex", provider: "codex", email: "owner@example.com", readiness: "ready" },
        { ref: "../escape", provider: "codex", email: null, readiness: "ready" },
      ],
    })
    expect(decoded).toEqual({
      ok: true,
      generatedAt: "2026-07-11T12:00:00.000Z",
      accounts: [{ ref: "codex", provider: "codex", email: "owner@example.com", readiness: "ready" }],
    })
    expect(decodeFleetAccountsProjection(undefined)).toEqual({ ok: false, reason: "invalid_bridge_payload" })
    expect(decodeFleetAccountsProjection({ ok: true, generatedAt: "x", accounts: [{ ref: "codex", provider: "codex", email: null, readiness: "sort_of_ready" }] })).toEqual({ ok: false, reason: "invalid_bridge_payload" })
  })

  test("usage decode refuses a mismatched ref", () => {
    expect(decodeFleetUsageEntry({ ok: false, ref: "codex", reason: "account_not_found" }, "codex")).toEqual({
      state: "failed",
      reason: "account_not_found",
    })
    expect(decodeFleetUsageEntry({ ok: false, ref: "codex", reason: "x" }, "codex-2")).toEqual({
      state: "failed",
      reason: "invalid_bridge_payload",
    })
  })
})

describe("typed fleet intent loop (registry -> state -> re-render)", () => {
  const makeHarness = (bridge: Parameters<typeof makeFleetWorkspaceHandlers>[1], manage?: () => Effect.Effect<void>) =>
    Effect.gen(function* () {
      const state = yield* SubscriptionRef.make<{ fleet: FleetWorkspaceState }>({ fleet: readyState })
      const registry = yield* makeIntentRegistry(
        fleetWorkspaceIntents,
        makeFleetWorkspaceHandlers(state, bridge, manage),
      )
      return { state, registry }
    })

  test("refresh loads through the bridge and replaces the projection", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { state, registry } = yield* makeHarness({
          list: async () => ({
            ok: true,
            generatedAt: "2026-07-11T13:00:00.000Z",
            accounts: [{ ref: "codex-2", provider: "codex", email: null, readiness: "ready" }],
          }),
          usage: async (ref) => ({ ok: false, ref, reason: "pylon_runtime_unavailable" }),
        })
        const view = fleetWorkspaceView((yield* SubscriptionRef.get(state)).fleet)
        const refresh = nodeByKey(view, "fleet-refresh") as { onPress: Parameters<typeof resolveIntentRef>[0] }
        yield* registry.dispatch(resolveIntentRef(refresh.onPress, null))
        const next = (yield* SubscriptionRef.get(state)).fleet
        expect(next.phase).toBe("ready")
        expect(next.generatedAt).toBe("2026-07-11T13:00:00.000Z")
        expect(next.accounts).toEqual([{ ref: "codex-2", provider: "codex", email: null, readiness: "ready" }])
        expect(next.usage).toEqual({})
      }),
    )
  })

  test("a failed refresh lands in the explicit unavailable phase", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { state, registry } = yield* makeHarness({
          list: async () => {
            throw new Error("bridge exploded")
          },
          usage: async (ref) => ({ ok: false, ref, reason: "pylon_runtime_unavailable" }),
        })
        yield* registry.dispatch(resolveIntentRef(
          (nodeByKey(fleetWorkspaceView(readyState), "fleet-refresh") as { onPress: Parameters<typeof resolveIntentRef>[0] }).onPress,
          null,
        ))
        const next = (yield* SubscriptionRef.get(state)).fleet
        expect(next.phase).toBe("unavailable")
        expect(next.reason).toBe("invalid_bridge_payload")
      }),
    )
  })

  test("usage check dispatches per row, records evidence, and refuses unknown refs", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const requested: Array<string> = []
        const { state, registry } = yield* makeHarness({
          list: async () => ({ ok: false, reason: "unused" }),
          usage: async (ref) => {
            requested.push(ref)
            return {
              ok: true,
              ref,
              refreshedAt: "2026-07-11T12:05:00.000Z",
              summary: { inputTokens: 10, outputTokens: 5, totalTokens: 15 },
            }
          },
        })
        const view = fleetWorkspaceView((yield* SubscriptionRef.get(state)).fleet)
        const check = nodeByKey(view, "fleet-usage-check-codex") as { onPress: Parameters<typeof resolveIntentRef>[0] }
        yield* registry.dispatch(resolveIntentRef(check.onPress, null))
        yield* registry.dispatch(resolveIntentRef(IntentRef("FleetUsageCheckRequested", StaticPayload("not-a-fleet-account")), null))
        const next = (yield* SubscriptionRef.get(state)).fleet
        expect(requested).toEqual(["codex"])
        expect(next.usage["codex"]).toEqual({
          state: "checked",
          refreshedAt: "2026-07-11T12:05:00.000Z",
          inputTokens: 10,
          outputTokens: 5,
          totalTokens: 15,
        })
      }),
    )
  })

  test("refresh also pulls the session ledger snapshot when the bridge exposes it", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { state, registry } = yield* makeHarness({
          list: async () => ({
            ok: true,
            generatedAt: "2026-07-11T13:00:00.000Z",
            accounts: [{ ref: "codex", provider: "codex", email: null, readiness: "ready" }],
          }),
          usage: async (ref) => ({ ok: false, ref, reason: "unused" }),
          ledger: async () => ledgerSnapshot,
        })
        yield* registry.dispatch(resolveIntentRef(IntentRef("FleetRefreshRequested", StaticPayload(null)), null))
        const next = (yield* SubscriptionRef.get(state)).fleet
        expect(next.ledger).toEqual(ledgerSnapshot)
      }),
    )
  })

  test("FleetLedgerUpdated re-pulls the snapshot through the bridge (push channel loop)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        let pulls = 0
        const { state, registry } = yield* makeHarness({
          list: async () => ({ ok: false, reason: "unused" }),
          usage: async (ref) => ({ ok: false, ref, reason: "unused" }),
          ledger: async () => {
            pulls += 1
            return ledgerSnapshot
          },
        })
        yield* registry.dispatch(resolveIntentRef(IntentRef("FleetLedgerUpdated", StaticPayload(null)), null))
        expect(pulls).toBe(1)
        const next = (yield* SubscriptionRef.get(state)).fleet
        expect(next.ledger).toEqual(ledgerSnapshot)
      }),
    )
  })

  test("FleetLedgerUpdated on a ledger-less bridge is a no-op (older hosts)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const { state, registry } = yield* makeHarness({
          list: async () => ({ ok: false, reason: "unused" }),
          usage: async (ref) => ({ ok: false, ref, reason: "unused" }),
        })
        yield* registry.dispatch(resolveIntentRef(IntentRef("FleetLedgerUpdated", StaticPayload(null)), null))
        expect((yield* SubscriptionRef.get(state)).fleet.ledger).toBeNull()
      }),
    )
  })

  test("manage-accounts delegates to the provided settings toggle", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        let toggles = 0
        const { registry } = yield* makeHarness(
          {
            list: async () => ({ ok: false, reason: "unused" }),
            usage: async (ref) => ({ ok: false, ref, reason: "unused" }),
          },
          () => Effect.sync(() => {
            toggles += 1
          }),
        )
        yield* registry.dispatch(resolveIntentRef(
          { name: "FleetManageAccountsRequested" } as Parameters<typeof resolveIntentRef>[0],
          null,
        ))
        expect(toggles).toBe(1)
      }),
    )
  })

  test("run control re-admits the current generation and refreshes confirmed state", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const submitted: unknown[] = []
      const confirmed = { ...runningCockpitCard, status: "canceled" as const, runVersion: 8, actions: ["resume", "retry", "close"] as const }
      const { state, registry } = yield* makeHarness({
        list: async () => ({ ok: false, reason: "unused" }),
        usage: async ref => ({ ok: false, ref, reason: "unused" }),
        control: async command => { submitted.push(command); return { status: "unknown_pending_reconcile" } },
        cockpit: async () => ({ authority: "live", cards: [confirmed] }),
      })
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        fleet: { ...current.fleet, cockpitAuthority: "live" as const, cockpitCards: [runningCockpitCard] },
      }))
      const pause = nodeByKey(fleetWorkspaceView((yield* SubscriptionRef.get(state)).fleet), "fleet-cockpit-run.demo-pause") as { onPress: Parameters<typeof resolveIntentRef>[0] }
      yield* registry.dispatch(resolveIntentRef(pause.onPress, null))
      // CUT-16: the control carries the card's exact confirmed provider lane
      // so the durable lane fence admits it (codex → codex_app_server).
      expect(submitted).toEqual([{ action: "pause", threadRef: "thread.demo", runRef: "run.demo", expectedVersion: 7, lane: "codex_app_server" }])
      expect((yield* SubscriptionRef.get(state)).fleet.cockpitCards[0]?.runVersion).toBe(8)
      expect((yield* SubscriptionRef.get(state)).fleet.cockpitCards[0]?.status).toBe("canceled")
    }))
  })

  test("run controls map every confirmed provider to its exact lane and omit unknown (CUT-16)", () => {
    expect(fleetControlLaneForProvider("codex")).toBe("codex_app_server")
    expect(fleetControlLaneForProvider("claude")).toBe("claude_pylon")
    expect(fleetControlLaneForProvider("openagents")).toBe("hosted_khala")
    expect(fleetControlLaneForProvider("unknown")).toBeNull()
  })

  test("attention decision carries exact interaction and run generations", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const submitted: unknown[] = []
      const { state, registry } = yield* makeHarness({
        list: async () => ({ ok: false, reason: "unused" }),
        usage: async ref => ({ ok: false, ref, reason: "unused" }),
        decideAttention: async command => { submitted.push(command); return { status: "pending_reconcile" } },
        cockpit: async () => ({ authority: "live", cards: [runningCockpitCard] }),
      })
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        fleet: { ...current.fleet, cockpitAuthority: "live" as const, cockpitCards: [runningCockpitCard] },
      }))
      const approve = nodeByKey(fleetWorkspaceView((yield* SubscriptionRef.get(state)).fleet), "fleet-cockpit-run.demo-interaction.approval-approve") as { onPress: Parameters<typeof resolveIntentRef>[0] }
      yield* registry.dispatch(resolveIntentRef(approve.onPress, null))
      expect(submitted).toEqual([{
        action: "approve",
        threadRef: "thread.demo",
        runRef: "run.demo",
        interactionRef: "interaction.approval",
        expectedRunVersion: 7,
        expectedInteractionVersion: 3,
      }])
    }))
  })
})
