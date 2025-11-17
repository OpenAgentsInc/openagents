import { ClaudeAcpAgent } from '@zed-industries/claude-code-acp'
import {
  AgentSideConnection,
  ClientSideConnection,
  ndJsonStream,
  PROTOCOL_VERSION,
} from '@agentclientprotocol/sdk'
import { readFile, mkdir, appendFile } from 'node:fs/promises'
import path from 'node:path'

async function main() {
  // Prepare file logging (logs/ is gitignored)
  const logsDir = path.resolve(process.cwd(), 'logs')
  await mkdir(logsDir, { recursive: true })
  const runId = new Date().toISOString().replace(/[:.]/g, '-')
  const logFile = path.join(logsDir, `acp-${runId}.jsonl`)
  const writeLog = async (record: any) => {
    try {
      const line = JSON.stringify({ ts: Date.now(), ...record }) + '\n'
      await appendFile(logFile, line)
    } catch (e) {
      console.error('[log write error]', e)
    }
  }

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
  // Minimal client that logs ALL ACP sessionUpdate events as full JSON,
  // implements a basic readTextFile, and auto-approves first permission.
  const client = {
    async sessionUpdate(params: any) {
      // Log the entire sessionUpdate payload for full visibility
      console.log('\n[ACP sessionUpdate]')
      console.log(JSON.stringify(params, null, 2))
      await writeLog({ event: 'sessionUpdate', payload: params })

      // In addition, stream text chunks to stdout for readability
      const u = params.update
      if (u?.sessionUpdate === 'agent_message_chunk' && u.content?.type === 'text') {
        process.stdout.write(u.content.text)
      }
    },
    async readTextFile(params: { sessionId: string; path: string; line?: number | null; limit?: number | null }) {
      try {
        const raw = await readFile(params.path, 'utf8')
        if (params?.limit != null || params?.line != null) {
          const lines = raw.split(/\r?\n/)
          const start = Math.max(0, ((params.line ?? 1) as number) - 1)
          const end = params.limit != null ? start + (params.limit as number) : lines.length
          const content = lines.slice(start, end).join('\n')
          return { content }
        }
        return { content: raw }
      } catch (e: any) {
        return { content: `ERROR reading ${params.path}: ${e?.message ?? e}` }
      }
    },
    async requestPermission(params: any) {
      // Log permission request; auto-select the first option if present
      console.log('\n[ACP requestPermission]')
      console.log(JSON.stringify(params, null, 2))
      await writeLog({ event: 'requestPermission', payload: params })
      const first = params.options?.[0]
      const response = first
        ? { outcome: { outcome: 'selected', optionId: first.optionId } }
        : { outcome: { outcome: 'cancelled' as const } }
      console.log('\n[ACP requestPermission.response]')
      console.log(JSON.stringify(response, null, 2))
      await writeLog({ event: 'requestPermission.response', payload: response })
      return response
    },
  }

  const clientConn = new ClientSideConnection(() => client as any, clientStream)

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

  const promptSegments = [
    {
      type: 'text' as const,
      text:
        `Explore the project using read-only tools only. Read these absolute paths:\n` +
        `1) ${absA}\n2) ${absB}\n3) ${absC}\n\n` +
        `Rules:\n` +
        `- Use only the Read file tool.\n` +
        `- Do not run commands or write files.\n` +
        `- After reading, summarize the project structure and key configuration in 6-10 bullet points.`,
    },
  ]

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
