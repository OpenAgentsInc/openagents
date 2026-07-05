/**
 * Recovery-state model for Khala Code desktop native-shell recovery UX
 * (issue #8441): load-failure and unresponsive-window recovery.
 *
 * Pure and framework-free so the state shape and action wiring can be unit
 * tested without mounting React, a real webview, or Electrobun.
 */

export type KhalaCodeDesktopRecoveryKind = "load_failure" | "none" | "unresponsive"

export type KhalaCodeDesktopRecoveryAction =
  | "export_logs"
  | "keep_waiting"
  | "quit"
  | "relaunch"

export type KhalaCodeDesktopRecoveryState =
  | Readonly<{ kind: "none" }>
  | Readonly<{
    detail: string
    kind: "load_failure"
    since: string
  }>
  | Readonly<{
    detail: string
    kind: "unresponsive"
    since: string
  }>

/**
 * The recovery choices offered for each non-"none" state, in display order.
 * "keep_waiting" only makes sense for "unresponsive" (something is still
 * running and might finish); a failed load has nothing to wait on, so its
 * choices are limited to relaunch/export/quit.
 */
export const khalaCodeDesktopRecoveryActionsFor = (
  kind: Exclude<KhalaCodeDesktopRecoveryKind, "none">,
): readonly KhalaCodeDesktopRecoveryAction[] =>
  kind === "unresponsive"
    ? ["relaunch", "export_logs", "keep_waiting", "quit"]
    : ["relaunch", "export_logs", "quit"]

export const KHALA_CODE_DESKTOP_RECOVERY_ACTION_LABELS: Readonly<
  Record<KhalaCodeDesktopRecoveryAction, string>
> = {
  export_logs: "Export debug logs",
  keep_waiting: "Keep waiting",
  quit: "Quit",
  relaunch: "Relaunch",
}

export type KhalaCodeDesktopRecoveryActionOutcome =
  | Readonly<{ kind: "dismiss" }>
  | Readonly<{ kind: "export"; path: string }>
  | Readonly<{ kind: "noop" }>

export type KhalaCodeDesktopRecoveryActionDispatch = Readonly<{
  exportDebugLogs: () => Promise<{ readonly path: string }>
  quit: () => Promise<void>
  relaunch: () => Promise<void>
}>

/**
 * Routes a chosen recovery action to the matching RPC/dispatch call. This is
 * the single place that maps the four user-facing choices to concrete
 * effects, so a UI surface (renderer overlay, native message-box button
 * index, a future menu command) only has to pick an action name and call
 * this — it never needs its own copy of the mapping.
 */
export const dispatchKhalaCodeDesktopRecoveryAction = async (
  action: KhalaCodeDesktopRecoveryAction,
  dispatch: KhalaCodeDesktopRecoveryActionDispatch,
): Promise<KhalaCodeDesktopRecoveryActionOutcome> => {
  switch (action) {
    case "keep_waiting":
      return { kind: "noop" }
    case "export_logs": {
      const result = await dispatch.exportDebugLogs()
      return { kind: "export", path: result.path }
    }
    case "relaunch":
      await dispatch.relaunch()
      return { kind: "dismiss" }
    case "quit":
      await dispatch.quit()
      return { kind: "dismiss" }
  }
}

export const khalaCodeDesktopLoadFailureState = (
  detail: string,
  since: string,
): KhalaCodeDesktopRecoveryState => ({ detail, kind: "load_failure", since })

export const khalaCodeDesktopUnresponsiveState = (
  detail: string,
  since: string,
): KhalaCodeDesktopRecoveryState => ({ detail, kind: "unresponsive", since })

export const khalaCodeDesktopRecoveryNoneState: KhalaCodeDesktopRecoveryState = { kind: "none" }
