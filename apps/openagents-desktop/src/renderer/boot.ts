/**
 * Renderer entrypoint (#8574): boots the OpenAgents Desktop shell as one
 * Effect Native program — SubscriptionRef state, `makeViewProgramFromState`,
 * a typed intent registry, and the DOM renderer from the shared vendored
 * catalog. It follows the standard Effect Native consumer pattern.
 *
 * Boundary: this file runs sandboxed (contextIsolation on, nodeIntegration
 * off). The only host input is the frozen `openagentsDesktop` bridge object
 * from the preload, decoded with Effect Schema — never trusted raw.
 */
import {
  IntentRef,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  StaticPayload,
  type IntentReporter,
} from "@effect-native/core"
import { Effect, Exit, Schema, Scope, SubscriptionRef } from "@effect-native/core/effect"
import { makeDomRenderer } from "@effect-native/render-dom"

import {
  unavailableCodexSettingsBridge,
  unavailableProviderAccountsSettingsBridge,
  type CodexSettingsBridge,
  type OpenAgentsSessionSettingsBridge,
  type ProviderAccountsSettingsBridge,
} from "./settings.ts"
import {
  unavailableFleetAccountsBridge,
  type FleetAccountsBridge,
} from "./fleet-workspace.ts"
import {
  desktopShellIntents,
  desktopShellView,
  initialDesktopShellState,
  makeDesktopShellHandlers,
} from "./shell.ts"
import { restorableHistoryThreadRef } from "./history-restore.ts"
import { openagentsDesktopTheme } from "./theme.ts"
import { selectDesktopChatHostSelection } from "./runtime-conversation.ts"
import {
  makeLocalHarnessChatHost,
  type FableLocalRendererBridge,
} from "./local-harness.ts"
import { withHarnessLanes, type HarnessLanes } from "./shell.ts"
import {
  decodeFableLocalAvailability,
  type FableLocalAvailability,
  type FableLocalEventEnvelope,
} from "../fable-local-contract.ts"
import { type DesktopThread } from "../chat-contract.ts"
import {
  type DesktopWorkspaceFile,
  type DesktopWorkspaceGitDiff,
  type DesktopWorkspaceGitStatus,
  type DesktopWorkspaceSaveResult,
  type DesktopWorkspaceSnapshot,
} from "../workspace-contract.ts"
import type {
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayResponse,
} from "../runtime-gateway-contract.ts"
import type { CodexHistoryCatalog, CodexHistoryPage } from "../codex-history-contract.ts"
import { historyCatalogPageSize } from "./history-workspace.ts"

/** Effect Schema at the preload boundary (issue #8574: Schema, not Zod). */
const DesktopBridgeSchema = Schema.Struct({
  host: Schema.String,
  platform: Schema.String,
})

type DesktopBridge = Readonly<{
  host: string
  platform: string
  runtimeRequest?: (value: unknown) => Promise<DesktopRuntimeGatewayResponse>
  runtimeSubscribe?: (listener: (event: DesktopRuntimeGatewayEvent) => void) => () => void
  stageFleet?: (value: unknown) => Promise<unknown>
  listThreads?: () => Promise<unknown>
  newThread?: () => Promise<unknown>
  openThread?: (value: unknown) => Promise<unknown>
  hydrateThread?: (value: unknown) => Promise<unknown>
  sendMessage?: (value: unknown) => Promise<unknown>
  workspaceSummary?: () => Promise<unknown>
  chooseWorkspace?: () => Promise<unknown>
  readWorkspaceFile?: (value: unknown) => Promise<unknown>
  saveWorkspaceFile?: (value: unknown) => Promise<unknown>
  workspaceGitStatus?: () => Promise<unknown>
  workspaceGitDiff?: (value: unknown) => Promise<unknown>
  codexAccounts?: () => Promise<unknown>
  codexConnectStart?: () => Promise<unknown>
  codexReconnectStart?: (ref: string) => Promise<unknown>
  codexConnectStatus?: () => Promise<unknown>
  codexConnectOpenVerification?: () => Promise<unknown>
  providerAccounts?: Readonly<{
    list?: () => Promise<unknown>
    usage?: (ref: string) => Promise<unknown>
  }>
  fableLocal?: Readonly<{
    availability?: () => Promise<unknown>
    start?: (value: unknown) => Promise<unknown>
    interrupt?: (value: unknown) => Promise<unknown>
    onEvent?: (listener: (envelope: FableLocalEventEnvelope) => void) => () => void
  }>
  usageLedger?: Readonly<{
    snapshot?: () => Promise<unknown>
    onEvent?: (listener: (snapshot: unknown) => void) => () => void
  }>
}>

const readBridge = (): DesktopBridge | undefined =>
  (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop

/**
 * Codex settings bridge over the preload surface. Each call degrades to the
 * honest unavailable projection when the bridge is absent; the settings
 * handlers schema-decode every response before it touches state.
 */
const codexSettingsBridge: CodexSettingsBridge = {
  listAccounts: () => {
    const bridge = readBridge()
    return typeof bridge?.codexAccounts === "function"
      ? bridge.codexAccounts()
      : unavailableCodexSettingsBridge.listAccounts()
  },
  connectStart: () => {
    const bridge = readBridge()
    return typeof bridge?.codexConnectStart === "function"
      ? bridge.codexConnectStart()
      : unavailableCodexSettingsBridge.connectStart()
  },
  reconnectStart: (ref: string) => {
    const bridge = readBridge()
    return typeof bridge?.codexReconnectStart === "function"
      ? bridge.codexReconnectStart(ref)
      : unavailableCodexSettingsBridge.reconnectStart!(ref)
  },
  connectStatus: () => {
    const bridge = readBridge()
    return typeof bridge?.codexConnectStatus === "function"
      ? bridge.codexConnectStatus()
      : unavailableCodexSettingsBridge.connectStatus()
  },
  openVerification: () => {
    const bridge = readBridge()
    return typeof bridge?.codexConnectOpenVerification === "function"
      ? bridge.codexConnectOpenVerification()
      : unavailableCodexSettingsBridge.openVerification()
  },
}

/**
 * Fleet accounts bridge over the preload surface. Each call degrades to the
 * honest unavailable projection when the bridge is absent; the fleet handlers
 * schema-decode every response before it touches state.
 */
const fleetAccountsBridge: FleetAccountsBridge = {
  list: () => {
    const bridge = readBridge()
    return typeof bridge?.providerAccounts?.list === "function"
      ? bridge.providerAccounts.list()
      : unavailableFleetAccountsBridge.list()
  },
  usage: (ref) => {
    const bridge = readBridge()
    return typeof bridge?.providerAccounts?.usage === "function"
      ? bridge.providerAccounts.usage(ref)
      : unavailableFleetAccountsBridge.usage(ref)
  },
  // Session usage ledger snapshot (#8712 Lane C): absent-bridge hosts simply
  // render no Session usage section (the fleet decode drops a null).
  ledger: () => {
    const bridge = readBridge()
    return typeof bridge?.usageLedger?.snapshot === "function"
      ? bridge.usageLedger.snapshot()
      : Promise.resolve(null)
  },
}

/**
 * Settings Claude-accounts bridge over the same preload providerAccounts
 * surface. Degrades to the explicit unavailable projection when the bridge
 * is absent; the settings handlers schema-decode every response.
 */
const providerAccountsSettingsBridge: ProviderAccountsSettingsBridge = {
  list: () => {
    const bridge = readBridge()
    return typeof bridge?.providerAccounts?.list === "function"
      ? bridge.providerAccounts.list()
      : unavailableProviderAccountsSettingsBridge.list()
  },
}

let sessionRequestSequence = 0
const openAgentsSessionSettingsBridge: OpenAgentsSessionSettingsBridge = {
  status: async () => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    return bridge.runtimeRequest({
      kind: "query",
      requestId: `renderer-session-status-${++sessionRequestSequence}`,
      query: { id: "runtime.bootstrap" },
    })
  },
  signIn: async () => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    return bridge.runtimeRequest({
      kind: "command",
      commandId: `renderer-session-sign-in-${++sessionRequestSequence}`,
      command: { id: "session.sign_in" },
    })
  },
  signOut: async () => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    return bridge.runtimeRequest({
      kind: "command",
      commandId: `renderer-session-sign-out-${++sessionRequestSequence}`,
      command: { id: "session.sign_out" },
    })
  },
}

export const decodeBridgeHost = (bridge: unknown): string => {
  const decoded = Schema.decodeUnknownExit(DesktopBridgeSchema)(bridge)
  return Exit.isSuccess(decoded)
    ? `${decoded.value.host}/${decoded.value.platform}`
    : "unknown-host"
}

const mountDesktopShell = (root: HTMLElement, host: string) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialDesktopShellState(host))
    const program = makeViewProgramFromState(state, desktopShellView)
    const bridge = readBridge()
    const localChat = {
      listThreads: async () => {
        const raw = await bridge?.listThreads?.()
        return Array.isArray(raw) ? raw as DesktopThread[] : []
      },
      newThread: async () => {
        const raw = await bridge?.newThread?.()
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      openThread: async (id: string) => {
        const raw = await bridge?.openThread?.({ id })
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      hydrateThread: async (id: string) => {
        const raw = await bridge?.hydrateThread?.({ id })
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      sendMessage: async (input: Readonly<{ id: string; message: string }>) => {
        const raw = await bridge?.sendMessage?.(input)
        if (typeof raw === "object" && raw !== null && typeof (raw as { ok?: unknown }).ok === "boolean") return raw as { ok: boolean; thread?: DesktopThread | null; error?: string }
        return { ok: false, error: "Desktop chat returned an invalid response." }
      },
    }
    // Fable local lane (#8712): narrow bridge over the preload surface. The
    // local-mode chat host routes "fable" through it and refuses "codex"
    // explicitly — never the legacy cloud gateway (no silent substitution).
    const fableLocalBridge: FableLocalRendererBridge | null =
      typeof bridge?.fableLocal?.start === "function" &&
      typeof bridge.fableLocal.availability === "function" &&
      typeof bridge.fableLocal.interrupt === "function" &&
      typeof bridge.fableLocal.onEvent === "function"
        ? {
            availability: bridge.fableLocal.availability,
            start: bridge.fableLocal.start,
            interrupt: bridge.fableLocal.interrupt,
            onEvent: bridge.fableLocal.onEvent as (
              listener: (envelope: FableLocalEventEnvelope) => void,
            ) => () => void,
          }
        : null
    let fableAvailability: FableLocalAvailability | null = null
    const localHarnessChat = makeLocalHarnessChatHost({
      base: localChat,
      fable: fableLocalBridge,
      fableAvailability: () => fableAvailability,
    })
    const selection = yield* Effect.promise(() => selectDesktopChatHostSelection({
      request: bridge?.runtimeRequest,
      subscribe: bridge?.runtimeSubscribe,
      local: localHarnessChat,
    }))
    const chat = selection.host
    // Evidence-gated composer lanes (#8712), resolved BEFORE first mount so
    // the chips never flash an unproven state.
    if (fableLocalBridge !== null && selection.mode === "local") {
      const rawAvailability = yield* Effect.promise(() =>
        fableLocalBridge.availability().catch(() => null))
      fableAvailability = decodeFableLocalAvailability(rawAvailability)
    }
    const harnessLanes: HarnessLanes = selection.mode === "runtime"
      ? {
          fable: { available: true, reason: null },
          codex: { available: true, reason: null },
        }
      : {
          fable: fableAvailability?.state === "available"
            ? { available: true, reason: null }
            : { available: false, reason: "Fable — unavailable: no linked Claude account" },
          codex: { available: false, reason: "Codex — requires OpenAgents session" },
        }
    yield* SubscriptionRef.update(state, current => withHarnessLanes(current, harnessLanes))
    let historyRequestSequence = 0
    const restoreHistory = (): { selectedThreadRef:string;offset:number;selectedItemRef:string|null;railCollapsed:boolean;expandedThreadRefs:ReadonlyArray<string> } | null => { try { const value=JSON.parse(localStorage.getItem("openagents.desktop.history.v1")??"null");return value&&typeof value.selectedThreadRef==="string"&&Number.isInteger(value.offset)&&value.offset>=0&&value.offset<=1_000_000&&typeof value.railCollapsed==="boolean"&&(value.selectedItemRef===null||typeof value.selectedItemRef==="string")&&Array.isArray(value.expandedThreadRefs)&&value.expandedThreadRefs.every((ref:unknown)=>typeof ref==="string")?value:null } catch{return null} }
    const historyHost = {
      catalog: async (): Promise<CodexHistoryCatalog | null> => {
        if (typeof bridge?.runtimeRequest !== "function") return null
        const response = await bridge.runtimeRequest({ kind: "query", requestId: `renderer-history-catalog-${++historyRequestSequence}`, query: { id: "codex.history.catalog" } })
        return response.kind === "codex_history_catalog" ? response.catalog : null
      },
      page: async (threadRef: string, offset: number, limit: number): Promise<CodexHistoryPage | null> => {
        if (typeof bridge?.runtimeRequest !== "function") return null
        const response = await bridge.runtimeRequest({ kind: "query", requestId: `renderer-history-page-${++historyRequestSequence}`, query: { id: "codex.history.page", threadRef, offset, limit } })
        return response.kind === "codex_history_page" ? response.page : null
      },
      save: (value: any): void => { try { localStorage.setItem("openagents.desktop.history.v1",JSON.stringify({...value,expandedThreadRefs:Array.isArray(value?.expandedThreadRefs)?value.expandedThreadRefs:[]})) } catch { /* restoration is best effort and contains refs only */ } },
    }
    const registry = yield* makeIntentRegistry(
      desktopShellIntents,
      makeDesktopShellHandlers(state, undefined, async (input) => {
        const bridge = (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop
        if (typeof bridge?.stageFleet !== "function") {
          return {
            state: "unavailable",
            message: "Local Pylon control is unavailable. No fleet work was dispatched.",
            intentStatus: null,
          }
        }
        const raw = await bridge.stageFleet(input)
        if (
          typeof raw === "object" && raw !== null &&
          (raw as { state?: unknown }).state !== undefined &&
          typeof (raw as { message?: unknown }).message === "string"
        ) {
          const value = raw as { state?: unknown; message: string; intentStatus?: unknown }
          if (value.state === "accepted" || value.state === "rejected" || value.state === "unavailable") {
            return {
              state: value.state,
              message: value.message,
              intentStatus: typeof value.intentStatus === "string" ? value.intentStatus : null,
            }
          }
        }
        return {
          state: "unavailable",
          message: "Local Pylon returned an invalid response. No fleet work was dispatched.",
          intentStatus: null,
        }
      }, chat, {
        summary: async () => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.workspaceSummary?.()
          return typeof raw === "object" && raw !== null && typeof (raw as { root?: unknown }).root === "string" ? raw as DesktopWorkspaceSnapshot : null
        },
        choose: async () => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.chooseWorkspace?.()
          return typeof raw === "object" && raw !== null && typeof (raw as { root?: unknown }).root === "string" ? raw as DesktopWorkspaceSnapshot : null
        },
        readFile: async (path) => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.readWorkspaceFile?.({ path })
          return typeof raw === "object" && raw !== null && typeof (raw as { content?: unknown }).content === "string" ? raw as DesktopWorkspaceFile : null
        },
        saveFile: async (input) => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.saveWorkspaceFile?.(input)
          if (typeof raw !== "object" || raw === null) {
            return { state: "unavailable", message: "Workspace save returned an invalid response." }
          }
          const value = raw as { state?: unknown; file?: unknown; message?: unknown }
          if (
            (value.state === "saved" || value.state === "conflict") &&
            typeof value.file === "object" && value.file !== null &&
            typeof (value.file as { content?: unknown }).content === "string" &&
            typeof (value.file as { revision?: unknown }).revision === "string"
          ) return value as DesktopWorkspaceSaveResult
          return {
            state: "unavailable",
            message: typeof value.message === "string" ? value.message : "Workspace save is unavailable.",
          }
        },
        gitStatus: async () => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.workspaceGitStatus?.()
          if (typeof raw !== "object" || raw === null) return { state: "unavailable" }
          const value = raw as { state?: unknown; changes?: unknown; truncated?: unknown }
          return value.state === "available" && Array.isArray(value.changes) && typeof value.truncated === "boolean"
            ? value as DesktopWorkspaceGitStatus
            : { state: "unavailable" }
        },
        gitDiff: async (path) => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.workspaceGitDiff?.({ path })
          if (typeof raw !== "object" || raw === null) return { state: "unavailable", message: "Git review is unavailable." }
          const value = raw as { state?: unknown; path?: unknown; content?: unknown; truncated?: unknown; message?: unknown }
          if (value.state === "available" && typeof value.path === "string" && typeof value.content === "string" && typeof value.truncated === "boolean") {
            return value as DesktopWorkspaceGitDiff
          }
          return { state: "unavailable", message: typeof value.message === "string" ? value.message : "Git review is unavailable." }
        },
      }, codexSettingsBridge, undefined, openAgentsSessionSettingsBridge, historyHost, fleetAccountsBridge, providerAccountsSettingsBridge),
    )
    // Session usage ledger push (#8712 Lane C): every ledger change re-pulls
    // the typed snapshot through the fleet handlers (schema-decoded there).
    if (typeof bridge?.usageLedger?.onEvent === "function") {
      const unsubscribeLedger = bridge.usageLedger.onEvent(() => {
        void Effect.runPromise(
          registry.dispatch(resolveIntentRef(IntentRef("FleetLedgerUpdated", StaticPayload(null)))),
        )
      })
      window.addEventListener("pagehide", () => unsubscribeLedger(), { once: true })
    }
    const historyCatalog = yield* Effect.promise(historyHost.catalog)
    if (historyCatalog !== null) {
      const restored=restoreHistory(); const selected=restorableHistoryThreadRef(historyCatalog,restored?.selectedThreadRef,historyCatalogPageSize)
      const firstPage = selected === null ? null : yield* Effect.promise(() => historyHost.page(selected, restored?.offset??0, 50))
      yield* SubscriptionRef.update(state, current => ({ ...current, history: { ...current.history, catalog: historyCatalog, page: firstPage, selectedItemRef: firstPage?.items.some(item=>item.itemRef===restored?.selectedItemRef)?restored!.selectedItemRef:null, railCollapsed:restored?.railCollapsed??false, expandedThreadRefs:restored?.expandedThreadRefs??firstPage?.agents.filter(agent=>agent.descendantCount>0).map(agent=>agent.threadRef)??[] } }))
    }
    const existing = yield* Effect.promise(chat.listThreads)
    const threads = Array.isArray(existing) ? existing.filter((item): item is DesktopThread => typeof item === "object" && item !== null && typeof (item as { id?: unknown }).id === "string") : []
    if (threads.length > 0) {
      const first = threads[0]!
      yield* SubscriptionRef.update(state, (current) => ({
        ...current,
        threads,
        activeThreadId: first.id,
        notes: [],
      }))
    }
    const focusComposer = (): void => {
      // Let the controlled TextField render its cleared/enabled state first.
      window.setTimeout(() => {
        root.querySelector<HTMLInputElement>('[data-en-key="shell-input"] input')?.focus()
      }, 0)
    }
    const report: IntentReporter = (ref, runtimeValue) => {
      const shouldFocus = ref.name === "DesktopNewChat" || ref.name === "DesktopNoteSubmitted"
      return registry.dispatch(resolveIntentRef(ref, runtimeValue ?? null)).pipe(
        Effect.ensuring(Effect.sync(() => {
          if (shouldFocus) focusComposer()
        })),
      )
    }
    const onCommandPaletteShortcut = (event: KeyboardEvent): void => {
      const target = event.target
      const editable = target instanceof HTMLElement &&
        target.closest("input, textarea, [contenteditable='true']") !== null
      if (
        event.defaultPrevented ||
        editable ||
        event.key.toLowerCase() !== "k" ||
        (!event.metaKey && !event.ctrlKey)
      ) return
      event.preventDefault()
      void Effect.runPromise(
        registry.dispatch(resolveIntentRef(IntentRef("DesktopCommandPaletteToggled", StaticPayload(null)))),
      )
    }
    let historyShortcutSteps=0
    let historyShortcutAbsoluteIndex:number|null=null
    let historyShortcutRunning=false
    let historySelectionTimer:number|null=null
    const settleFrame=():Promise<void>=>new Promise(resolve=>requestAnimationFrame(()=>resolve()))
    const scrollHistorySelectionIntoView=async(index:number,threadRef:string):Promise<void>=>{
      for(let attempt=0;attempt<4;attempt++){
        await settleFrame()
        const list=root.querySelector<HTMLElement>('[data-en-key="sidebar-history-list"]')
        if(list===null)return
        if(list.getAttribute("data-en-virtualized")==="true"){
          list.scrollTop=Math.max(0,index*28-Math.max(0,list.clientHeight-28)/2)
          list.dispatchEvent(new Event("scroll",{bubbles:true}))
          continue
        }
        const row=[...root.querySelectorAll<HTMLElement>('[data-en-key^="sidebar-thread-"]')].find(item=>item.getAttribute("data-en-key")===`sidebar-thread-${threadRef}`)
        const item=row?.closest<HTMLElement>('[data-en-role="item"]')??row
        if(row===undefined||item==null)continue
        const rows=Array.from(list.querySelectorAll<HTMLElement>('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]'))
        const rowIndex=rows.findIndex(candidate=>candidate.getAttribute("data-en-key")===`sidebar-thread-${threadRef}`)
        const measuredRowHeight=rows.map(candidate=>candidate.getBoundingClientRect().height).find(height=>height>0)??24
        const estimatedTop=rowIndex<0?item.offsetTop:rowIndex*measuredRowHeight
        const requestedScrollTop=Math.max(0,estimatedTop-Math.max(0,list.clientHeight-measuredRowHeight)/2)
        list.scrollTop=requestedScrollTop
        list.dispatchEvent(new Event("scroll",{bubbles:true}))
        await settleFrame()
        const rowRect=row.getBoundingClientRect();const listRect=list.getBoundingClientRect()
        if(rowRect.top>=listRect.top-1&&rowRect.bottom<=listRect.bottom+1)return
      }
    }
    const pumpHistoryConversationShortcut=async():Promise<void>=>{
      if(historyShortcutRunning)return
      historyShortcutRunning=true
      try{
        while(historyShortcutSteps!==0||historyShortcutAbsoluteIndex!==null){
          await new Promise(resolve=>window.setTimeout(resolve,35))
          const current=await Effect.runPromise(SubscriptionRef.get(state))
          if(current.workspace!=="chat"||current.history.catalog.roots.length===0){historyShortcutSteps=0;historyShortcutAbsoluteIndex=null;break}
          const roots=current.history.catalog.roots
          const activeRef=current.history.pendingThreadRef??current.history.page?.rootThreadRef
          const activeIndex=roots.findIndex(item=>item.threadRef===activeRef)
          const steps=historyShortcutSteps
          const absoluteIndex=historyShortcutAbsoluteIndex
          historyShortcutSteps=0
          historyShortcutAbsoluteIndex=null
          const baseIndex=activeIndex<0?(steps>0?-1:1):activeIndex
          const targetIndex=Math.max(0,Math.min(roots.length-1,absoluteIndex??baseIndex+steps))
          if(targetIndex===activeIndex)continue
          let visible=current.history.visibleRootCount
          while(targetIndex>=visible){
            await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryCatalogMoreRequested",StaticPayload(null)))))
            visible+=historyCatalogPageSize
          }
          const targetRef=roots[targetIndex]!.threadRef
          await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopHistoryConversationPreviewed",StaticPayload(targetRef)))))
          await scrollHistorySelectionIntoView(targetIndex,targetRef)
          if(historySelectionTimer!==null)window.clearTimeout(historySelectionTimer)
          historySelectionTimer=window.setTimeout(()=>{
            historySelectionTimer=null
            void Effect.runPromise(SubscriptionRef.get(state)).then(current=>{
              if(current.history.pendingThreadRef!==targetRef)return
              return Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryConversationSelected",StaticPayload(targetRef)))))
            }).then(()=>scrollHistorySelectionIntoView(targetIndex,targetRef))
          },110)
        }
      }finally{
        historyShortcutRunning=false
        if(historyShortcutSteps!==0||historyShortcutAbsoluteIndex!==null)void pumpHistoryConversationShortcut()
      }
    }
    const setHistoryShortcutHints=(visible:boolean):void=>{
      void Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopHistoryShortcutHintsChanged",StaticPayload(visible)))))
    }
    const onHistoryConversationShortcut = (event: KeyboardEvent): void => {
      const target=event.target
      const editable=target instanceof HTMLElement&&target.closest("input, textarea, [contenteditable='true']")!==null
      const platformModifier=bridge?.platform==="darwin"?event.metaKey&&!event.ctrlKey:event.ctrlKey&&!event.metaKey
      const digit=/^[1-9]$/.test(event.key)?Number(event.key)-1:null
      if(event.defaultPrevented||editable||!platformModifier||event.altKey||event.shiftKey||(digit===null&&event.key!=="ArrowUp"&&event.key!=="ArrowDown"))return
      event.preventDefault()
      if(digit!==null){historyShortcutSteps=0;historyShortcutAbsoluteIndex=digit}
      else {historyShortcutAbsoluteIndex=null;historyShortcutSteps+=event.key==="ArrowDown"?1:-1}
      void pumpHistoryConversationShortcut()
    }
    const onHistoryModifierDown=(event:KeyboardEvent):void=>{
      const platformModifier=bridge?.platform==="darwin"?event.metaKey:event.ctrlKey
      if(platformModifier)setHistoryShortcutHints(true)
    }
    const onHistoryModifierUp=(event:KeyboardEvent):void=>{
      if((bridge?.platform==="darwin"&&event.key==="Meta")||(bridge?.platform!=="darwin"&&event.key==="Control"))setHistoryShortcutHints(false)
    }
    const onHistoryWindowBlur=():void=>setHistoryShortcutHints(false)
    window.addEventListener("keydown", onCommandPaletteShortcut)
    window.addEventListener("keydown", onHistoryModifierDown)
    window.addEventListener("keydown", onHistoryConversationShortcut)
    window.addEventListener("keyup", onHistoryModifierUp)
    window.addEventListener("blur", onHistoryWindowBlur)
    window.addEventListener("pagehide", () => {
      window.removeEventListener("keydown", onCommandPaletteShortcut)
      window.removeEventListener("keydown", onHistoryModifierDown)
      window.removeEventListener("keydown", onHistoryConversationShortcut)
      window.removeEventListener("keyup", onHistoryModifierUp)
      window.removeEventListener("blur", onHistoryWindowBlur)
      if(historySelectionTimer!==null)window.clearTimeout(historySelectionTimer)
    }, { once: true })
    const renderer = makeDomRenderer({ theme: openagentsDesktopTheme })
    yield* renderer.mount(root, program.viewStream, report)
    // First paint must never wait on local rollout parsing. The sidebar gets
    // metadata immediately; the selected thread receives five recent messages
    // and then its bounded expanded tail after the DOM is already visible.
    if (threads.length > 0) {
      const id = threads[0]!.id
      window.setTimeout(() => {
        void (async () => {
          const detail = await chat.openThread(id)
          if (typeof detail === "object" && detail !== null && typeof (detail as { id?: unknown }).id === "string") {
            const selected = detail as DesktopThread
            await Effect.runPromise(SubscriptionRef.update(state, current => current.activeThreadId === id
              ? { ...current, threads: [selected, ...current.threads.filter(thread => thread.id !== id)], notes: selected.notes }
              : current))
          }
          if (chat.hydrateThread === undefined) return
          const hydrated = await chat.hydrateThread(id)
          if (typeof hydrated === "object" && hydrated !== null && typeof (hydrated as { id?: unknown }).id === "string") {
            const expanded = hydrated as DesktopThread
            await Effect.runPromise(SubscriptionRef.update(state, current => current.activeThreadId === id
              ? { ...current, threads: [expanded, ...current.threads.filter(thread => thread.id !== id)], notes: expanded.notes }
              : current))
          }
        })()
      }, 0)
    }
  })

const boot = (): void => {
  const root = document.getElementById("openagents-desktop-root")
  if (root === null) return
  const host = decodeBridgeHost(
    (globalThis as { openagentsDesktop?: unknown }).openagentsDesktop,
  )
  const scope = Effect.runSync(Scope.make())
  window.addEventListener(
    "pagehide",
    () => {
      void Effect.runPromise(Scope.close(scope, Exit.void))
    },
    { once: true },
  )
  void Effect.runPromise(Scope.provide(scope)(mountDesktopShell(root, host))).catch(
    (error) => {
      console.error("[openagents-desktop] shell mount failed", error)
    },
  )
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot)
} else {
  boot()
}
