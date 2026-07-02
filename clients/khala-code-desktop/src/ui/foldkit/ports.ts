import { Schema as S } from "effect"
import { ts } from "foldkit/schema"

export const KhalaCodeFoldkitHostPortMessage = S.Union([
  ts("HostPing", {
    nonce: S.String,
  }),
  ts("HostSetLabel", {
    label: S.String,
  }),
])
export type KhalaCodeFoldkitHostPortMessage =
  typeof KhalaCodeFoldkitHostPortMessage.Type

export const KhalaCodeFoldkitProgramPortMessage = S.Union([
  ts("ProgramMounted", {
    mountId: S.String,
  }),
  ts("ProgramPong", {
    nonce: S.String,
    count: S.Number,
  }),
  ts("ProgramUnmounted", {
    mountId: S.String,
  }),
])
export type KhalaCodeFoldkitProgramPortMessage =
  typeof KhalaCodeFoldkitProgramPortMessage.Type

const decodeHostPortMessage = S.decodeUnknownSync(KhalaCodeFoldkitHostPortMessage)
const decodeProgramPortMessage = S.decodeUnknownSync(KhalaCodeFoldkitProgramPortMessage)

export type KhalaCodeFoldkitPortListener<Message> = (message: Message) => void

export type KhalaCodeFoldkitHostPort = Readonly<{
  send: (message: unknown) => KhalaCodeFoldkitHostPortMessage
  subscribe: (
    listener: KhalaCodeFoldkitPortListener<KhalaCodeFoldkitHostPortMessage>,
  ) => () => void
}>

export type KhalaCodeFoldkitProgramPort = Readonly<{
  emit: (message: unknown) => KhalaCodeFoldkitProgramPortMessage
  subscribe: (
    listener: KhalaCodeFoldkitPortListener<KhalaCodeFoldkitProgramPortMessage>,
  ) => () => void
}>

const makePort = <Message>(
  decode: (message: unknown) => Message,
): Readonly<{
  publish: (message: unknown) => Message
  subscribe: (listener: KhalaCodeFoldkitPortListener<Message>) => () => void
}> => {
  const listeners = new Set<KhalaCodeFoldkitPortListener<Message>>()
  return {
    publish: (message) => {
      const decoded = decode(message)
      for (const listener of listeners) listener(decoded)
      return decoded
    },
    subscribe: (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
  }
}

export type KhalaCodeFoldkitPorts = Readonly<{
  host: KhalaCodeFoldkitHostPort
  program: KhalaCodeFoldkitProgramPort
}>

export const makeKhalaCodeFoldkitPorts = (): KhalaCodeFoldkitPorts => {
  const host = makePort(decodeHostPortMessage)
  const program = makePort(decodeProgramPortMessage)
  return {
    host: {
      send: host.publish,
      subscribe: host.subscribe,
    },
    program: {
      emit: program.publish,
      subscribe: program.subscribe,
    },
  }
}
