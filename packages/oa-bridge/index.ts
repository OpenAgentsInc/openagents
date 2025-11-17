import { ClaudeAcpAgent } from '@zed-industries/claude-code-acp'
import { AgentSideConnection, ClientSideConnection, ndJsonStream, PROTOCOL_VERSION } from '@agentclientprotocol/sdk'

async function main() {
  console.log("Hello via Bun!");

  // Cross-wire two NDJSON streams to connect client ⇄ agent in-process
  // Use global Web Streams (from lib.dom) to match ACP SDK typings
  const a = new TransformStream<Uint8Array, Uint8Array>()
  const b = new TransformStream<Uint8Array, Uint8Array>()

  const agentStream = ndJsonStream(a.writable, b.readable)
  const clientStream = ndJsonStream(b.writable, a.readable)

  // Create the agent side
  // The agent uses Claude Code SDK under the hood, so you must be logged in
  // via `claude /login` (or have ~/.claude.json present)
  new AgentSideConnection((client) => new ClaudeAcpAgent(client), agentStream)

}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
