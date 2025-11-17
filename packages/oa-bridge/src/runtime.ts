import { AgentSideConnection, ClientSideConnection, ndJsonStream } from '@agentclientprotocol/sdk'

export function createInProcessNdjsonStreams() {
  // Use global Web Streams (from lib.dom) to match ACP SDK typings
  const a = new TransformStream<Uint8Array, Uint8Array>()
  const b = new TransformStream<Uint8Array, Uint8Array>()
  const agentStream = ndJsonStream(a.writable, b.readable)
  const clientStream = ndJsonStream(b.writable, a.readable)
  return { agentStream, clientStream }
}

export function connectAgent(createAgent: (client: any) => unknown, agentStream: ReturnType<typeof ndJsonStream>) {
  return new AgentSideConnection((client) => (createAgent as any)(client), agentStream as any)
}

export function createClientConnection(createClient: () => any, clientStream: ReturnType<typeof ndJsonStream>) {
  return new ClientSideConnection(createClient as any, clientStream as any)
}
