import { AgentSideConnection, ClientSideConnection, ndJsonStream } from '@agentclientprotocol/sdk'
import { TransformStream as NodeTransformStream } from 'node:stream/web'

export function createInProcessNdjsonStreams() {
  // Prefer global Web Streams if available; fall back to Node's implementation
  const TS: any = (globalThis as any).TransformStream ?? NodeTransformStream
  const a = new TS()
  const b = new TS()
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
