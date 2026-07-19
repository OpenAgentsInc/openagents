import { createHash, randomUUID } from "node:crypto"

import {
  DesktopCommandId,
  decodeDesktopDeferredCommandOrNull,
  desktopCanonicalCommandRegistry,
  type DesktopCommandDefinition,
  type DesktopDeferredCommand,
} from "./desktop-command-contract"
import { Schema } from "effect"

const requestRefFor = (value: string): string =>
  `command.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`

export const deferredDesktopCommand = (
  command: DesktopCommandDefinition,
  source: DesktopDeferredCommand["source"],
  requestRef = `command.${randomUUID()}`,
  commandArguments = command.defaultArguments,
): DesktopDeferredCommand => ({
  schema: "openagents.desktop.deferred_command.v1",
  requestRef,
  commandId: command.id,
  arguments: commandArguments,
  source,
  delivery: "dispatch",
})

export const parseDesktopCommandUrl = (
  raw: string,
  source: "deep_link" | "second_instance" | "restore" = "deep_link",
): DesktopDeferredCommand | null => {
  try {
    const url = new URL(raw)
    const segments = url.pathname.split("/").filter(Boolean)
    if (
      url.protocol !== "openagents:" ||
      url.hostname !== "command" ||
      segments.length !== 1 ||
      url.username !== "" ||
      url.password !== "" ||
      url.search !== "" ||
      url.hash !== ""
    ) return null
    const commandId = Schema.decodeUnknownSync(DesktopCommandId)(decodeURIComponent(segments[0]!))
    const command = desktopCanonicalCommandRegistry.find(value => value.id === commandId)
    // Finder delivery is admitted from Electron's main-process `open-file`
    // event only. A URL must never manufacture a file-open request using the
    // registry's harmless placeholder relative path.
    return command === undefined || command.id === "workspace.open_file"
      ? null
      : deferredDesktopCommand(command, source, requestRefFor(url.href))
  } catch {
    return null
  }
}

export const desktopCommandsFromArgv = (
  argv: ReadonlyArray<string>,
  source: "deep_link" | "second_instance" | "restore",
): ReadonlyArray<DesktopDeferredCommand> => argv.flatMap(value => {
  const command = parseDesktopCommandUrl(value, source)
  return command === null ? [] : [command]
})

export type DesktopCommandHost = Readonly<{
  enqueue: (value: unknown) => "accepted" | "duplicate" | "invalid"
  attach: (send: (command: DesktopDeferredCommand) => void) => void
  detach: () => void
  pendingCount: () => number
}>

export type NativeDesktopCommandEnvironment = Readonly<{
  hasOpenWindow: () => boolean
  openWindow: () => void
  enqueue: DesktopCommandHost["enqueue"]
}>

/**
 * Native menu accelerators outlive the last macOS BrowserWindow. A New chat
 * command therefore has to recreate the window before it is queued; the
 * command host will retain it until that window's renderer readiness
 * handshake attaches the next sink.
 */
export const dispatchNativeDesktopCommand = (
  command: DesktopCommandDefinition,
  environment: NativeDesktopCommandEnvironment,
): ReturnType<DesktopCommandHost["enqueue"]> => {
  if (command.id === "chat.new" && !environment.hasOpenWindow()) {
    environment.openWindow()
  }
  return environment.enqueue(deferredDesktopCommand(command, "native_menu"))
}

export const makeDesktopCommandHost = (maxPending = 32): DesktopCommandHost => {
  let sink: ((command: DesktopDeferredCommand) => void) | null = null
  let pending: DesktopDeferredCommand[] = []
  const admitted = new Set<string>()
  const flush = (): void => {
    if (sink === null) return
    const batch = pending
    pending = []
    for (const command of batch) sink(command)
  }
  return {
    enqueue: value => {
      const command = decodeDesktopDeferredCommandOrNull(value)
      if (command === null) return "invalid"
      if (admitted.has(command.requestRef)) {
        const duplicate: DesktopDeferredCommand = { ...command, delivery: "duplicate_rejected" }
        pending = [...pending, duplicate].slice(-Math.max(1, maxPending))
        flush()
        return "duplicate"
      }
      admitted.add(command.requestRef)
      if (admitted.size > 256) admitted.delete(admitted.values().next().value!)
      pending = [...pending, command].slice(-Math.max(1, maxPending))
      flush()
      return "accepted"
    },
    attach: send => {
      sink = send
      flush()
    },
    detach: () => { sink = null },
    pendingCount: () => pending.length,
  }
}
