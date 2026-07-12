import type { CodexConnectService } from "./codex-connect.ts"
import type { CodexHistoryHost } from "./codex-history-host.ts"
import type { DesktopSyncHost } from "./desktop-sync-host.ts"
import type { DesktopRuntimeGateway } from "./runtime-gateway.ts"
import type { DesktopWorkspaceService } from "./workspace-service.ts"
import type { DesktopVoiceHost } from "./voice-host.ts"

type Slot<Value> = Readonly<{
  value: Value
  close: () => void
}>

export type DesktopHostLifecycleSnapshot = Readonly<{
  disposed: boolean
  runtime: boolean
  workspace: boolean
  sync: boolean
  account: boolean
  history: boolean
  voice: boolean
  windowCount: number
}>

export type DesktopHostLifecycle = Readonly<{
  runtime: () => DesktopRuntimeGateway | null
  workspace: () => DesktopWorkspaceService | null
  sync: () => DesktopSyncHost | null
  account: () => CodexConnectService | null
  history: () => CodexHistoryHost | null
  voice: () => DesktopVoiceHost | null
  replaceRuntime: (service: DesktopRuntimeGateway) => DesktopRuntimeGateway | null
  replaceWorkspace: (service: DesktopWorkspaceService | null) => DesktopWorkspaceService | null
  replaceSync: (service: DesktopSyncHost | null) => DesktopSyncHost | null
  replaceAccount: (service: CodexConnectService) => CodexConnectService | null
  replaceHistory: (service: CodexHistoryHost) => CodexHistoryHost | null
  replaceVoice: (service: DesktopVoiceHost | null) => DesktopVoiceHost | null
  registerWindow: (windowRef: string, close: () => void) => () => void
  snapshot: () => DesktopHostLifecycleSnapshot
  dispose: () => void
}>

const once = (close: () => void): (() => void) => {
  let closed = false
  return () => {
    if (closed) return
    closed = true
    close()
  }
}

/**
 * Process composition for replaceable Desktop services. Each slot owns one
 * exactly-once closer; replacing a narrower session/WorkContext closes the old
 * slot before publishing the new one, and app disposal drains every slot.
 */
export const makeDesktopHostLifecycle = (initial: Readonly<{
  runtime: DesktopRuntimeGateway
  account: CodexConnectService
  history: CodexHistoryHost
}>): DesktopHostLifecycle => {
  let disposed = false
  let runtime: Slot<DesktopRuntimeGateway> | null = { value: initial.runtime, close: once(initial.runtime.dispose) }
  let workspace: Slot<DesktopWorkspaceService> | null = null
  let sync: Slot<DesktopSyncHost> | null = null
  let account: Slot<CodexConnectService> | null = { value: initial.account, close: once(initial.account.dispose) }
  let history: Slot<CodexHistoryHost> | null = { value: initial.history, close: once(initial.history.dispose) }
  let voice: Slot<DesktopVoiceHost> | null = null
  const windows = new Map<string, () => void>()

  const replace = <Value>(
    current: Slot<Value> | null,
    next: Value | null,
    close: (value: Value) => void,
  ): Slot<Value> | null => {
    current?.close()
    if (next === null) return null
    const slot = { value: next, close: once(() => close(next)) }
    if (disposed) {
      slot.close()
      return null
    }
    return slot
  }

  return {
    runtime: () => runtime?.value ?? null,
    workspace: () => workspace?.value ?? null,
    sync: () => sync?.value ?? null,
    account: () => account?.value ?? null,
    history: () => history?.value ?? null,
    voice: () => voice?.value ?? null,
    replaceRuntime: service => {
      runtime = replace(runtime, service, value => value.dispose())
      return runtime?.value ?? null
    },
    replaceWorkspace: service => {
      workspace = replace(workspace, service, value => value.dispose())
      return workspace?.value ?? null
    },
    replaceSync: service => {
      sync = replace(sync, service, value => value.close())
      return sync?.value ?? null
    },
    replaceAccount: service => {
      account = replace(account, service, value => value.dispose())
      return account?.value ?? null
    },
    replaceHistory: service => {
      history = replace(history, service, value => value.dispose())
      return history?.value ?? null
    },
    replaceVoice: service => {
      voice = replace(voice, service, value => value.dispose())
      return voice?.value ?? null
    },
    registerWindow: (windowRef, close) => {
      if (disposed) {
        once(close)()
        return () => undefined
      }
      if (windows.has(windowRef)) throw new Error(`duplicate_window_ref:${windowRef}`)
      const closeOnce = once(close)
      windows.set(windowRef, closeOnce)
      return () => {
        const owned = windows.get(windowRef)
        if (owned === undefined) return
        windows.delete(windowRef)
        owned()
      }
    },
    snapshot: () => ({
      disposed,
      runtime: runtime !== null,
      workspace: workspace !== null,
      sync: sync !== null,
      account: account !== null,
      history: history !== null,
      voice: voice !== null,
      windowCount: windows.size,
    }),
    dispose: () => {
      if (disposed) return
      disposed = true
      for (const close of windows.values()) close()
      windows.clear()
      runtime?.close(); runtime = null
      workspace?.close(); workspace = null
      account?.close(); account = null
      history?.close(); history = null
      voice?.close(); voice = null
      sync?.close(); sync = null
    },
  }
}
