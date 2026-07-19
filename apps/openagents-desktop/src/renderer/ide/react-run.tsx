import type { ReactElement } from "react"
import { useCallback, useEffect, useMemo, useState } from "react"

import { Activity, CircleStop, FileText, Play, RefreshCw, RotateCcw, Trash2 } from "lucide-react"

import { Button } from "../../components/ui/button.tsx"
import {
  IdeRunSnapshotSchema,
  decodeIdeRunCommandResult,
  decodeIdeRunEvent,
  decodeIdeRunSnapshot,
  type IdeOutputChannel,
  type IdeRunActor,
  type IdeRunCommand,
  type IdeRunEvent,
  type IdeRunSnapshot,
} from "../../ide/run-contract.ts"

type IdeRunBridge = Readonly<{
  snapshot: () => Promise<unknown>
  command: (value: unknown) => Promise<unknown>
  onEvent: (listener: (event: IdeRunEvent) => void) => () => void
}>

const method = (value: unknown, name: string): ((...args: ReadonlyArray<unknown>) => unknown) | null => {
  if (typeof value !== "object" || value === null) return null
  const candidate = Reflect.get(value, name)
  return typeof candidate === "function" ? candidate.bind(value) : null
}

const readBridge = (): IdeRunBridge | null => {
  const desktop = Reflect.get(globalThis, "openagentsDesktop")
  if (typeof desktop !== "object" || desktop === null) return null
  const value = Reflect.get(desktop, "ideRun")
  const snapshot = method(value, "snapshot")
  const command = method(value, "command")
  const onEvent = method(value, "onEvent")
  if (snapshot === null || command === null || onEvent === null) return null
  return {
    snapshot: () => Promise.resolve(snapshot()),
    command: (input) => Promise.resolve(command(input)),
    onEvent: (listener) => {
      const unsubscribe = onEvent(listener)
      return typeof unsubscribe === "function"
        ? () => { Reflect.apply(unsubscribe, value, []) }
        : () => undefined
    },
  }
}

const ownerActor: IdeRunActor = { _tag: "Human", actorRef: "owner.desktop" }

const commandFor = (value: IdeRunCommand): IdeRunCommand => value

const outcomeLabel = (value: Readonly<{ _tag: string }>): string =>
  value._tag.replace(/([a-z])([A-Z])/gu, "$1 $2").toLocaleLowerCase()

const outputText = (channel: IdeOutputChannel | null): string =>
  channel?.chunks.map((chunk) => chunk.text).join("") ?? ""

const withOutput = (snapshot: IdeRunSnapshot, event: Extract<IdeRunEvent, { readonly _tag: "Output" }>): IdeRunSnapshot =>
  IdeRunSnapshotSchema.make({
    ...snapshot,
    outputChannels: snapshot.outputChannels.map((channel) => {
      if (channel.channelRef !== event.chunk.channelRef || channel.disposed) return channel
      const chunks = [...channel.chunks, event.chunk].slice(-2_048)
      const retainedBytes = chunks.reduce((total, chunk) => total + chunk.byteLength, 0)
      return {
        ...channel,
        firstSequence: chunks[0]?.sequence ?? event.chunk.sequence,
        lastSequence: event.chunk.sequence,
        chunks,
        retainedBytes,
        gap: channel.gap || event.chunk.gapBefore,
        redactionCount: channel.redactionCount + (event.chunk.redacted ? 1 : 0),
      }
    }),
  })

export type IdeRunPanelMode = "tasks" | "tests" | "output"

export const ReactIdeRunPanel = ({ mode }: { readonly mode: IdeRunPanelMode }): ReactElement => {
  const [snapshot, setSnapshot] = useState<IdeRunSnapshot | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [selectedChannelRef, setSelectedChannelRef] = useState<string | null>(null)
  const bridge = useMemo(readBridge, [])

  const refresh = useCallback(async (): Promise<void> => {
    if (bridge === null) {
      setNotice("Run services are unavailable in this Desktop host.")
      return
    }
    const decoded = decodeIdeRunSnapshot(await bridge.snapshot().catch(() => null))
    if (decoded === null) {
      setNotice("Choose a project before using terminal tasks and tests.")
      return
    }
    setSnapshot(decoded)
    setSelectedChannelRef((current) => decoded.outputChannels.some((channel) => channel.channelRef === current)
      ? current
      : decoded.outputChannels.at(-1)?.channelRef ?? null)
    setNotice(null)
  }, [bridge])

  useEffect(() => {
    void refresh()
    if (bridge === null) return
    return bridge.onEvent((raw) => {
      const event = decodeIdeRunEvent(raw)
      if (event === null) return
      if (event._tag === "Snapshot") {
        setSnapshot(event.snapshot)
      } else {
        setSnapshot((current) => current === null ? current : withOutput(current, event))
      }
    })
  }, [bridge, refresh])

  const run = useCallback(async (command: IdeRunCommand): Promise<void> => {
    if (bridge === null) return
    const decoded = decodeIdeRunCommandResult(await bridge.command(command).catch(() => null))
    if (decoded === null) {
      setNotice("The run command returned an invalid response.")
      return
    }
    setSnapshot(decoded.snapshot)
    setNotice(decoded._tag === "Refused" ? decoded.message : null)
    if (decoded._tag === "Succeeded") {
      setSelectedChannelRef(decoded.snapshot.outputChannels.at(-1)?.channelRef ?? selectedChannelRef)
    }
  }, [bridge, selectedChannelRef])

  useEffect(() => {
    if (snapshot !== null && snapshot.taskDefinitions.length === 0 && snapshot.testControllers.length === 0) {
      void run(commandFor({ _tag: "Discover" }))
    }
  }, [run, snapshot])

  if (snapshot === null) {
    return <div className="oa-react-run-empty" role="status"><Activity aria-hidden="true" /><p>{notice ?? "Loading project run capabilities…"}</p><Button size="sm" variant="ghost" onClick={() => void refresh()}><RefreshCw aria-hidden="true" />Refresh</Button></div>
  }

  const selectedChannel = snapshot.outputChannels.find((channel) => channel.channelRef === selectedChannelRef)
    ?? snapshot.outputChannels.at(-1)
    ?? null

  return <div className="oa-react-run-panel" data-run-mode={mode}>
    <div className="oa-react-run-summary" aria-label="Project run authority">
      <span>{snapshot.binding.cwdLabel}</span>
      <small>attachment {snapshot.binding.attachmentGeneration} · placement {snapshot.binding.placementGeneration} · Effect authority</small>
      <Button size="icon-sm" variant="ghost" aria-label="Discover tasks and tests" onClick={() => void run(commandFor({ _tag: "Discover" }))}><RefreshCw aria-hidden="true" /></Button>
    </div>
    {notice === null ? null : <p className="oa-react-terminal-notice" role="alert">{notice}</p>}

    {mode === "tasks" ? <div className="oa-react-run-list" aria-label="Declared tasks">
      {snapshot.taskDefinitions.length === 0 ? <p>No admitted package tasks were discovered.</p> : snapshot.taskDefinitions.map((definition) => {
        const latest = snapshot.taskRuns.filter((run) => run.definitionRef === definition.definitionRef).at(-1) ?? null
        const running = latest?.outcome._tag === "Running" || latest?.outcome._tag === "Ready"
        return <article key={definition.definitionRef}>
          <div><strong>{definition.label}</strong><small>{definition.group} · {definition.exactRerunLabel}</small></div>
          <span data-status={latest?.outcome._tag ?? "idle"}>{latest === null ? "idle" : outcomeLabel(latest.outcome)}</span>
          {running && latest !== null
            ? <Button size="sm" variant="ghost" onClick={() => void run(commandFor({ _tag: "CancelTask", runRef: latest.runRef, actor: ownerActor }))}><CircleStop aria-hidden="true" />Cancel</Button>
            : <Button size="sm" disabled={!definition.executable.admitted} onClick={() => void run(commandFor({ _tag: "StartTask", definitionRef: definition.definitionRef, actor: ownerActor }))}><Play aria-hidden="true" />Run</Button>}
        </article>
      })}
    </div> : null}

    {mode === "tests" ? <div className="oa-react-test-tree" aria-label="Test explorer">
      {snapshot.testControllers.map((controller) => <section key={controller.controllerRef}>
        <header><div><strong>{controller.label}</strong><small>generation {controller.discoveryGeneration} · {controller.items.length - 1} files</small></div><Button size="sm" disabled={!controller.discoveryComplete || controller.items.length <= 1} onClick={() => void run(commandFor({ _tag: "RunTests", controllerRef: controller.controllerRef, itemRefs: [], profile: "run", actor: ownerActor, retryOf: null }))}><Play aria-hidden="true" />Run all</Button></header>
        <ol>{controller.items.filter((item) => item.kind === "file").slice(0, 500).map((item) => {
          const latest = snapshot.testRuns.filter((candidate) => candidate.requestedItemRefs.includes(item.itemRef)).at(-1)
          const result = latest?.results.find((candidate) => candidate.itemRef === item.itemRef)
          return <li key={item.itemRef}><FileText aria-hidden="true" /><button disabled={!item.runnable} onClick={() => void run(commandFor({ _tag: "RunTests", controllerRef: controller.controllerRef, itemRefs: [item.itemRef], profile: "run", actor: ownerActor, retryOf: null }))}>{item.label}</button><span data-status={result?.status ?? "idle"}>{result?.status ?? "idle"}</span></li>
        })}</ol>
      </section>)}
      {snapshot.testRuns.filter((candidate) => candidate.outcome._tag === "Running").map((candidate) => <Button key={candidate.runRef} size="sm" variant="ghost" onClick={() => void run(commandFor({ _tag: "CancelTests", runRef: candidate.runRef, actor: ownerActor }))}><CircleStop aria-hidden="true" />Cancel active test run</Button>)}
    </div> : null}

    {mode === "output" ? <div className="oa-react-output-workbench">
      <div className="oa-react-output-channels" role="tablist" aria-label="Output channels">
        {snapshot.outputChannels.map((channel) => <button aria-selected={channel.channelRef === selectedChannel?.channelRef} key={channel.channelRef} role="tab" type="button" onClick={() => setSelectedChannelRef(channel.channelRef)}><span>{channel.label}</span>{channel.gap ? <small>gap</small> : null}</button>)}
      </div>
      {selectedChannel === null ? <p>No Output channel exists yet. Run a task or test.</p> : <>
        <div className="oa-react-output-toolbar"><span>{selectedChannel.chunks.length} chunks · {selectedChannel.retainedBytes} bytes{selectedChannel.droppedBytes > 0 ? ` · ${selectedChannel.droppedBytes} dropped` : ""}{selectedChannel.redactionCount > 0 ? ` · ${selectedChannel.redactionCount} redacted` : ""}</span><Button size="sm" variant="ghost" onClick={() => void run(commandFor({ _tag: "ExportOutput", channelRef: selectedChannel.channelRef, actor: ownerActor }))}><FileText aria-hidden="true" />Export</Button><Button size="sm" variant="ghost" onClick={() => void run(commandFor({ _tag: "ClearOutput", channelRef: selectedChannel.channelRef }))}><Trash2 aria-hidden="true" />Clear</Button></div>
        {selectedChannel.gap ? <p className="oa-react-output-gap" role="status">Output is incomplete. A bounded retention or reconnect gap precedes the retained sequence.</p> : null}
        <pre aria-label={`Output channel ${selectedChannel.label}`} tabIndex={0}>{outputText(selectedChannel)}</pre>
      </>}
    </div> : null}
  </div>
}
