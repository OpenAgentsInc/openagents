import { PROTOCOL_VERSION } from '@agentclientprotocol/sdk'
import path from 'node:path'
import { createRunLogger } from './src/logger'
import { createLoggingClient } from './src/acpClient'
import { createInProcessNdjsonStreams, connectAgent, createClientConnection } from './src/runtime'
import { getAgentFactory } from './src/agents'
import { buildReadOnlyPrompt } from './src/prompt'

async function main() {
  // Prepare file logging (logs/ is gitignored)
  const { write: writeLog, logFilePath } = await createRunLogger()
  console.log(`[oa-bridge] Logging to ${logFilePath}`)

  // Cross-wire two NDJSON streams to connect client ⇄ agent in-process
  const { agentStream, clientStream } = createInProcessNdjsonStreams()

  // Create the agent side (default: claude-code). We'll expand registry for more agents.
  const agentFactory = getAgentFactory('claude-code')
  connectAgent(agentFactory, agentStream)

  // Create a minimal logging client
  const client = createLoggingClient({ writeLog, streamTextToStdout: true })
  const clientConn = createClientConnection(() => client as any, clientStream)

  // Initialize
  const init = await clientConn.initialize({
    protocolVersion: PROTOCOL_VERSION,
    clientCapabilities: { fs: { readTextFile: true } },
  })
  console.log('\n[ACP initialize.result]')
  console.log(JSON.stringify(init, null, 2))
  await writeLog({ event: 'initialize.result', payload: init })

  // Create a session
  const { sessionId, models, modes } = await clientConn.newSession({ cwd: process.cwd(), mcpServers: [] })
  console.log('\n[ACP newSession.result]')
  console.log(JSON.stringify({ sessionId, models, modes }, null, 2))
  await writeLog({ event: 'newSession.result', payload: { sessionId, models, modes } })

  // Prepare a simple prompt encouraging read-only exploration of a few files
  // (Agent will invoke readTextFile with absolute paths.)
  const absA = path.resolve('package.json')
  const absB = path.resolve('convex/schema.ts')
  const absC = path.resolve('README.md')
  const promptSegments = buildReadOnlyPrompt([absA, absB, absC], { maxToolCalls: 8 }) as any

  const res = await clientConn.prompt({
    sessionId,
    prompt: promptSegments,
  })

  console.log('\n\n[ACP prompt.result] Stop reason:', res.stopReason)
  await writeLog({ event: 'prompt.result', payload: res })
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
