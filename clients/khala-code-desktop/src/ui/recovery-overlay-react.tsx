import * as React from "react"
import { flushSync } from "react-dom"
import { createRoot, type Root } from "react-dom/client"

import { Button } from "@openagentsinc/ui/react"

import {
  dispatchKhalaCodeDesktopRecoveryAction,
  khalaCodeDesktopRecoveryActionsFor,
  KHALA_CODE_DESKTOP_RECOVERY_ACTION_LABELS,
  type KhalaCodeDesktopRecoveryAction,
  type KhalaCodeDesktopRecoveryActionDispatch,
  type KhalaCodeDesktopRecoveryState,
} from "../shared/recovery-state.js"

export type KhalaCodeDesktopRecoveryOverlayHandle = Readonly<{
  hide: () => void
  show: (state: KhalaCodeDesktopRecoveryState) => void
}>

export type KhalaCodeDesktopRecoveryOverlayOptions = Readonly<{
  dispatch: KhalaCodeDesktopRecoveryActionDispatch
  onExported?: (path: string) => void
}>

const RECOVERY_TITLE: Readonly<Record<Exclude<KhalaCodeDesktopRecoveryState["kind"], "none">, string>> = {
  load_failure: "Khala Code failed to load",
  unresponsive: "Khala Code isn't responding",
}

type OverlayRenderState = Readonly<{
  actionPending: KhalaCodeDesktopRecoveryAction | null
  exportedPath: string | null
  recovery: KhalaCodeDesktopRecoveryState
}>

const RecoveryOverlayView = (
  props: Readonly<{
    onAction: (action: KhalaCodeDesktopRecoveryAction) => void
    render: OverlayRenderState
  }>,
): React.ReactElement | null => {
  const { recovery } = props.render
  if (recovery.kind === "none") return null

  const actions = khalaCodeDesktopRecoveryActionsFor(recovery.kind)

  return (
    <div
      className="fixed inset-0 z-[999] flex items-center justify-center bg-black/60"
      data-khala-code-recovery-overlay
      data-khala-code-recovery-kind={recovery.kind}
    >
      <div
        aria-live="assertive"
        aria-modal="true"
        className="w-full max-w-md rounded-lg border border-white/10 bg-neutral-950 p-6 text-neutral-100 shadow-xl"
        role="alertdialog"
      >
        <h2 className="text-base font-semibold">{RECOVERY_TITLE[recovery.kind]}</h2>
        <p className="mt-2 text-sm text-neutral-300" data-khala-code-recovery-detail>
          {recovery.detail}
        </p>
        <div className="mt-4 flex flex-wrap gap-2">
          {actions.map(action => (
            <Button
              data-khala-code-recovery-action={action}
              disabled={props.render.actionPending !== null}
              key={action}
              onClick={() => props.onAction(action)}
              type="button"
              variant={
                action === "quit"
                  ? "danger"
                  : action === "relaunch"
                    ? "primary"
                    : "secondary"
              }
            >
              {KHALA_CODE_DESKTOP_RECOVERY_ACTION_LABELS[action]}
            </Button>
          ))}
        </div>
        {props.render.exportedPath !== null && (
          <p className="mt-3 text-xs text-neutral-400" data-khala-code-recovery-exported-path>
            Exported debug logs to {props.render.exportedPath}
          </p>
        )}
      </div>
    </div>
  )
}

export const mountKhalaCodeRecoveryOverlay = (
  container: HTMLElement,
  options: KhalaCodeDesktopRecoveryOverlayOptions,
): KhalaCodeDesktopRecoveryOverlayHandle => {
  let reactRoot: Root | null = null
  let renderState: OverlayRenderState = {
    actionPending: null,
    exportedPath: null,
    recovery: { kind: "none" },
  }

  const root = (): Root => {
    reactRoot ??= createRoot(container)
    return reactRoot
  }

  const render = (): void => {
    flushSync(() => {
      root().render(<RecoveryOverlayView onAction={handleAction} render={renderState} />)
    })
  }

  function handleAction(action: KhalaCodeDesktopRecoveryAction): void {
    renderState = { ...renderState, actionPending: action }
    render()
    void dispatchKhalaCodeDesktopRecoveryAction(action, options.dispatch).then(outcome => {
      if (outcome.kind === "export") {
        options.onExported?.(outcome.path)
        renderState = { ...renderState, actionPending: null, exportedPath: outcome.path }
        render()
        return
      }
      if (outcome.kind === "noop") {
        // "keep_waiting": dismiss the dialog now; the watchdog re-shows it
        // later if the app is still unresponsive after another timeout.
        renderState = { actionPending: null, exportedPath: null, recovery: { kind: "none" } }
        render()
        return
      }
      // "dismiss" (relaunch/quit): the process is exiting/respawning, so
      // there is nothing further to render in this window.
      renderState = { ...renderState, actionPending: null }
      render()
    })
  }

  const show = (state: KhalaCodeDesktopRecoveryState): void => {
    renderState = { actionPending: null, exportedPath: null, recovery: state }
    render()
  }

  const hide = (): void => {
    renderState = { actionPending: null, exportedPath: null, recovery: { kind: "none" } }
    render()
  }

  render()

  return { hide, show }
}
