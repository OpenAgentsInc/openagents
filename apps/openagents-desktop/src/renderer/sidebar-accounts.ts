/**
 * Sidebar connected-accounts box (EP250 #8712).
 *
 * Owner statement (verbatim, contract
 * openagents_desktop.sidebar.connected_accounts_usage_box.v1): "in the left
 * sidebar, in a bottom box, like letting the chats flex up but show up to 5
 * connected accounts with a progress bar showing remaining weekly/hourly
 * usage (grayed out if we dont have that data)."
 *
 * Pure Effect Native data — `FleetWorkspaceState -> View | null` over the
 * shared catalog. This module CONSUMES the fleet accounts projection and the
 * per-account usage entries that the existing list/usage/probe flows already
 * populate; it never probes providers itself and adds no polling loop.
 *
 * Evidence discipline: the bar is MEASURED only when a decoded usage entry
 * for that account carries provider rate-limit windows (pylon
 * `truth.provider.snapshots` — codex-rs RateLimitSnapshot 5h/weekly lineage).
 * Everything else renders the grayed track with the honest reason
 * ("no usage-window data for this provider") in the tooltip and accessible
 * label — capability-truthful, never a fake fill.
 */
import {
  Accordion,
  Button,
  ComponentValueBinding,
  Divider,
  Icon,
  IntentRef,
  Meter,
  Stack,
  StaticPayload,
  Text,
  Tooltip,
  type IconName,
  type View,
} from "@effect-native/core"

import {
  sortFleetAccounts,
  type FleetAccount,
  type FleetUsageWindow,
  type FleetWorkspaceState,
} from "./fleet-workspace.ts"

/** Owner statement: "show up to 5 connected accounts". */
export const sidebarAccountsCap = 5

/** Honest aria/tooltip reason for a grayed bar (owner: "grayed out if we dont have that data"). */
export const sidebarAccountsNoDataReason = "no usage-window data for this provider"

/** Bounded ref rendering for the narrow sidebar row. */
export const truncateSidebarAccountRef = (ref: string): string =>
  ref.length <= 24 ? ref : `${ref.slice(0, 23)}…`

/** Provider glyph slot from the existing catalog icon set — no new icons. */
export const sidebarProviderIcon = (provider: string): IconName => {
  if (provider === "codex") return "Code"
  if (provider === "claude_agent") return "Sparkles"
  if (provider === "grok") return "Agent"
  return "Circle"
}

export type SidebarAccountBarModel =
  | Readonly<{
      /** A real provider rate-limit window was decoded this session. */
      kind: "measured"
      /** Remaining share of the tightest window, in [0, 1] (the bar value). */
      remainingRatio: number
      /** The tightest (lowest-remaining) window backing the bar. */
      window: FleetUsageWindow
      /** All decoded windows, e.g. "37% 5h remaining · 82% weekly remaining". */
      detail: string
    }>
  | Readonly<{ kind: "no-data"; reason: string }>

/**
 * Bar evidence for one account: the tightest decoded window rules the fill
 * (that is the limit the operator will actually hit first); every decoded
 * window is spelled out in the detail/aria text. No windows -> no-data.
 */
export const sidebarAccountBarModel = (
  fleet: FleetWorkspaceState,
  account: FleetAccount,
): SidebarAccountBarModel => {
  const entry = fleet.usage[account.ref]
  const windows = entry?.state === "checked" ? entry.windows ?? [] : []
  if (windows.length === 0) return { kind: "no-data", reason: sidebarAccountsNoDataReason }
  const tightest = [...windows].sort(
    (left, right) => left.remainingPercent - right.remainingPercent,
  )[0]!
  return {
    kind: "measured",
    remainingRatio: tightest.remainingPercent / 100,
    window: tightest,
    detail: windows
      .map((window) => `${Math.round(window.remainingPercent)}% ${window.label} remaining`)
      .join(" · "),
  }
}

const sidebarAccountMeter = (fleet: FleetWorkspaceState, account: FleetAccount): View => {
  const bar = sidebarAccountBarModel(fleet, account)
  if (bar.kind === "measured") {
    // Thin determinate track: accent-family fill (catalog Meter tone) over a
    // state-token track.
    return Meter({
      key: `sidebar-account-bar-${account.ref}`,
      value: bar.remainingRatio,
      tone: "info",
      label: `${account.ref}: ${bar.detail}`,
      style: {
        width: "full",
        height: "4xs",
        borderRadius: "full",
        backgroundColor: "stateSelected",
      },
    })
  }
  // Grayed: borderSubtle track, zero fill, reduced opacity — with the honest
  // reason on hover (catalog Tooltip) and in the accessible label.
  return Tooltip(
    { key: `sidebar-account-bar-reason-${account.ref}`, content: bar.reason, placement: { side: "top", align: "start" } },
    [
      Meter({
        key: `sidebar-account-bar-${account.ref}`,
        value: 0,
        tone: "neutral",
        label: `${account.ref}: ${bar.reason}`,
        style: {
          width: "full",
          height: "4xs",
          borderRadius: "full",
          backgroundColor: "borderSubtle",
          opacity: 0.45,
        },
      }),
    ],
  )
}

const sidebarAccountRow = (fleet: FleetWorkspaceState, account: FleetAccount): View =>
  Stack(
    {
      key: `sidebar-account-${account.ref}`,
      direction: "column",
      gap: "0.5",
      style: { width: "full", minWidth: 0 },
    },
    [
      Stack(
        {
          key: `sidebar-account-id-${account.ref}`,
          direction: "row",
          gap: "1",
          align: "center",
          style: { width: "full", minWidth: 0 },
        },
        [
          Icon({
            key: `sidebar-account-icon-${account.ref}`,
            name: sidebarProviderIcon(account.provider),
            size: "sm",
            color: account.readiness === "ready" ? "textMuted" : "textFaint",
            label: `${account.provider} account ${account.ref}${account.readiness === "ready" ? "" : ` (${account.readiness})`}`,
          }),
          Text({
            key: `sidebar-account-ref-${account.ref}`,
            content: truncateSidebarAccountRef(account.ref),
            variant: "caption",
            color: account.readiness === "ready" ? "textPrimary" : "textFaint",
          }),
        ],
      ),
      sidebarAccountMeter(fleet, account),
    ],
  )

/**
 * The bottom box. Returns null when no accounts are connected (the box is
 * absent, the history list keeps the full column). Ordering reuses the fleet
 * projection's evidence order: ready first, then provider, then ref. More
 * than five accounts render a dim "+N more" row deep-linking to the Fleet
 * workspace through the existing DesktopWorkspaceSelected intent.
 */
export const sidebarAccountsView = (fleet: FleetWorkspaceState, expanded = false): View | null => {
  const sorted = sortFleetAccounts(fleet.accounts)
  if (sorted.length === 0) return null
  const shown = sorted.slice(0, sidebarAccountsCap)
  const hidden = sorted.length - shown.length
  return Stack(
    {
      key: "sidebar-accounts",
      direction: "column",
      gap: "1",
      style: { width: "full", minWidth: 0, paddingTop: "1" },
      a11y: { role: "list", label: `${sorted.length} connected accounts` },
    },
    [
      // Hairline top edge: the box is visually pinned under the flexed list.
      Divider({ key: "sidebar-accounts-hairline", style: { width: "full" } }),
      Accordion({
        key: "sidebar-accounts-disclosure",
        mode: "single",
        expandedIds: expanded ? ["accounts"] : [],
        onToggle: IntentRef("DesktopSidebarAccountsToggled", ComponentValueBinding()),
        items: [{
          id: "accounts",
          header: `Accounts · ${sorted.length}`,
          content: [
            ...shown.map((account) => sidebarAccountRow(fleet, account)),
            ...(hidden > 0
              ? [Button({
                  key: "sidebar-accounts-more",
                  label: `+${hidden} more`,
                  variant: "ghost",
                  onPress: IntentRef("DesktopWorkspaceSelected", StaticPayload("fleet")),
                  style: { color: "textFaint", typeScale: "caption", padding: "0", alignSelf: "start" },
                  a11y: { label: `Open the Fleet workspace to see ${hidden} more connected accounts` },
                })]
              : []),
          ],
        }],
        style: { width: "full" },
        a11y: { label: `${sorted.length} connected accounts, ${expanded ? "expanded" : "collapsed"}` },
      }),
    ],
  )
}
