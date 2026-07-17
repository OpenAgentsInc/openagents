import {
  Button,
  ComponentValueBinding,
  IntentRef,
  Stack,
  StaticPayload,
  Terminal,
  Text,
  type View,
} from "@effect-native/core"

import type { MobileRepositoryTerminalState } from "../coding/mobile-repository-terminal"
import type { MobileAccessibilityProfile } from "./khala-core"

export const renderMobileTerminalView = (
  state: MobileRepositoryTerminalState,
  accessibility: MobileAccessibilityProfile,
): View => {
  const active = state.sessions.find(session => session.terminalRef === state.activeRef) ?? null
  return Stack({
    key: "mobile-terminal-root",
    direction: "column",
    gap: "2",
    padding: "3",
    preserveScrollAnchor: true,
    style: { width: "full", height: "full", backgroundColor: "background" },
    a11y: { role: "region", label: "Terminal workbench" },
  }, [
    Stack({ key: "terminal-actions", direction: "row", gap: "2" }, [
      Button({ key: "terminal-conversation", label: "Conversation", variant: "ghost",
        onPress: IntentRef("WorkbenchConversationOpened", StaticPayload({})), style: { minHeight: accessibility.minTouchTarget } }),
      Button({ key: "terminal-refresh", label: "Reconnect", variant: "ghost", disabled: state.state === "loading" || state.submitting,
        onPress: IntentRef("RepositoryTerminalRefreshed", StaticPayload({})), style: { minHeight: accessibility.minTouchTarget } }),
      Button({ key: "terminal-create", label: "New shell", variant: "secondary", disabled: state.scope === null || state.submitting || state.sessions.length >= 12,
        onPress: IntentRef("RepositoryTerminalCreateRequested", StaticPayload({})), style: { minHeight: accessibility.minTouchTarget } }),
    ]),
    ...(state.state === "loading" ? [Text({ key: "terminal-loading", content: "Reconnecting to exact worktree terminals…", variant: "body", color: "textMuted" })] : []),
    ...(state.state === "failed" || state.state === "unavailable" ? [Text({ key: "terminal-failure", content: state.message ?? "Terminal is unavailable.", variant: "body", color: state.state === "failed" ? "danger" : "textMuted" })] : []),
    ...(state.sessions.length === 0 && state.state === "ready" ? [Text({ key: "terminal-empty", content: "No terminal sessions. Open a shell bound to this worktree.", variant: "body", color: "textMuted" })] : []),
    ...(state.sessions.length === 0 ? [] : [Stack({ key: "terminal-sessions", direction: "row", gap: "2" }, state.sessions.map(session => Button({
      key: `terminal-session-${session.terminalRef}`,
      label: `${session.label} · ${session.status}${session.gap ? " · gap" : ""}`,
      variant: session.terminalRef === state.activeRef ? "secondary" : "ghost",
      onPress: IntentRef("RepositoryTerminalSelected", StaticPayload({ terminalRef: session.terminalRef })),
      style: { minHeight: accessibility.minTouchTarget },
    })))]),
    ...(active === null ? [] : [
      Text({ key: "terminal-identity", content: `${active.label} · ${active.shellLabel} · ${active.cols}×${active.rows}${active.recovered ? " · recovered" : ""}`, variant: "caption", color: "textMuted" }),
      Terminal({
        key: `terminal-host-${active.terminalRef}-${active.sessionVersionRef}`,
        output: active.tail,
        cols: active.cols,
        rows: active.rows,
        autoFit: true,
        scrollbackLines: 5_000,
        readOnly: active.status !== "running" || state.submitting,
        onEvent: IntentRef("RepositoryTerminalHostEvent", ComponentValueBinding()),
        style: { width: "full", height: "full", minHeight: 280 },
        a11y: { label: `Terminal ${active.label}` },
      }),
      Stack({ key: "terminal-keyboard-accessory", direction: "row", gap: "2" }, [
        ...([[
          "Esc", "\u001b",
        ], ["Tab", "\t"], ["Ctrl-C", "\u0003"], ["↑", "\u001b[A"], ["↓", "\u001b[B"]] as const).map(([label, data]) => Button({
          key: `terminal-key-${label}`,
          label,
          variant: "ghost",
          disabled: active.status !== "running" || state.submitting,
          onPress: IntentRef("RepositoryTerminalAccessoryKeyPressed", StaticPayload({ data })),
          style: { minHeight: accessibility.minTouchTarget },
        })),
      ]),
      Stack({ key: "terminal-session-actions", direction: "row", gap: "2" }, [
        Button({ key: "terminal-interrupt", label: "Interrupt", variant: "ghost", disabled: active.status !== "running" || state.submitting,
          onPress: IntentRef("RepositoryTerminalInterruptRequested", StaticPayload({})) }),
        Button({ key: "terminal-restart", label: "Restart", variant: "secondary", disabled: state.submitting,
          onPress: IntentRef("RepositoryTerminalRestartRequested", StaticPayload({})) }),
        Button({ key: "terminal-close", label: "Close", variant: "primary", disabled: state.submitting,
          onPress: IntentRef("RepositoryTerminalCloseRequested", StaticPayload({})) }),
      ]),
    ]),
    ...(state.lastReceipt === null ? [] : [Text({ key: `terminal-receipt-${state.lastReceipt.receiptRef}`,
      content: `Terminal ${state.lastReceipt.op} recorded · ${state.lastReceipt.receiptRef}`, variant: "caption", color: "success" })]),
    ...(state.message === null || state.state === "failed" || state.state === "unavailable" ? [] : [Text({ key: "terminal-notice", content: state.message, variant: "caption", color: "warning" })]),
  ])
}
