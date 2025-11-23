#!/usr/bin/env node
import { PROTOCOL_VERSION } from '@agentclientprotocol/sdk'
import { createInProcessNdjsonStreams, connectAgent, createClientConnection } from './src/runtime.js'
import { getAgentFactory } from './src/agents.js'
import { ConvexHttpClient } from 'convex/browser'
import { makeFunctionReference } from 'convex/server'
import { config as loadEnv } from 'dotenv'

type CliArgs = {
  token?: string
  convexUrl?: string
  prod?: boolean
}

function parseArgs(argv: string[]): CliArgs {
  const args: CliArgs = {}
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (!a) continue
    if (a === '--token' || a === '-t') {
      if (i + 1 < argv.length) {
        args.token = argv[i + 1]
        i++
      }
      continue
    }
    if (a.startsWith('--token=')) {
      args.token = a.slice('--token='.length)
      continue
    }
    if (a === '--convex' || a === '--url') {
      if (i + 1 < argv.length) {
        args.convexUrl = argv[i + 1]
        i++
      }
      continue
    }
    if (a.startsWith('--convex=') || a.startsWith('--url=')) {
      const [k, v] = a.split('=')
      if (v) args.convexUrl = v
      continue
    }
    if (a === '--prod') {
      args.prod = true
      continue
    }
  }
  return args
}

async function main() {
  const args = parseArgs(process.argv.slice(2))

  // Load env from local files to discover Convex URL when run inside a project
  if (args.prod) {
    loadEnv({ path: '.env.production.local', override: true })
    loadEnv({ path: '.env.production', override: true })
  } else {
    loadEnv({ path: '.env.local' })
    loadEnv()
  }

  const convexUrl = args.convexUrl || process.env.NEXT_PUBLIC_CONVEX_URL
  if (!convexUrl) {
    console.error('[oa-bridge] Missing Convex URL. Provide --convex <url> or set NEXT_PUBLIC_CONVEX_URL in .env.local')
    process.exit(2)
  }
  console.log(`[oa-bridge] Using Convex: ${convexUrl}`)

  const convex = new ConvexHttpClient(convexUrl)
  if (process.env.CONVEX_DEPLOY_KEY && (convex as any).setAdminAuth) {
    ;(convex as any).setAdminAuth(process.env.CONVEX_DEPLOY_KEY)
  }

  // Cross-wire two NDJSON streams to connect client ⇄ agent in-process
  const { agentStream, clientStream } = createInProcessNdjsonStreams()

  // Create the agent side (default: claude-code)
  const agentFactory = getAgentFactory('claude-code')
  connectAgent(agentFactory, agentStream)

  // Convex-backed client to persist events
  const cliToken = args.token
  let ordinal = 0

  const client = {
    async sessionUpdate(params: any) {
      const u = params.update
      // Stream text to stdout for message chunks
      if (u?.sessionUpdate === 'agent_message_chunk' && u.content?.type === 'text') {
        process.stdout.write(u.content.text)
      }
      // Persist every update as an event
      try {
        await convex.mutation(makeFunctionReference<'mutation'>('acp:appendEvent'), {
          sessionId: params.sessionId,
          eventType: u?.sessionUpdate ?? 'unknown',
          update: u,
          toolCallId: u?.toolCallId,
          claudeMeta: u?._meta?.claudeCode,
          ordinal: ++ordinal,
          createdAtTs: Date.now(),
          ...(cliToken ? { cliToken } : {}),
        })
      } catch (e) {
        console.error('[oa-bridge] appendEvent failed:', e)
      }

      // Maintain tool call snapshot
      if (u?.sessionUpdate === 'tool_call' || u?.sessionUpdate === 'tool_call_update') {
        const patch: any = {
          title: u.title,
          kind: u.kind,
          status: u.status,
          content: (u as any).content,
          locations: u.locations,
          rawInput: u.rawInput,
          rawOutput: u.rawOutput,
          claudeMeta: u?._meta?.claudeCode,
        }
        if (u?.toolCallId) {
          try {
            await convex.mutation(makeFunctionReference<'mutation'>('acp:upsertToolCall'), {
              sessionId: params.sessionId,
              toolCallId: u.toolCallId,
              patch,
              nowTs: Date.now(),
              ...(cliToken ? { cliToken } : {}),
            })
          } catch (e) {
            console.error('[oa-bridge] upsertToolCall failed:', e)
          }
        }
      }
    },

    async readTextFile(params: { sessionId: string; path: string; line?: number | null; limit?: number | null }) {
      const { readFile } = await import('node:fs/promises')
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
      try {
        await convex.mutation(makeFunctionReference<'mutation'>('acp:recordPermissionRequest'), {
          sessionId: params.sessionId,
          toolCallId: params.toolCall.toolCallId,
          request: params,
          createdAtTs: Date.now(),
          ...(cliToken ? { cliToken } : {}),
        })
      } catch (e) {
        console.error('[oa-bridge] recordPermissionRequest failed:', e)
      }
      const first = params.options?.[0]
      const response = first
        ? { outcome: { outcome: 'selected', optionId: first.optionId } }
        : { outcome: { outcome: 'cancelled' as const } }
      try {
        await convex.mutation(makeFunctionReference<'mutation'>('acp:recordPermissionResponse'), {
          sessionId: params.sessionId,
          toolCallId: params.toolCall.toolCallId,
          response,
          respondedAtTs: Date.now(),
          ...(cliToken ? { cliToken } : {}),
        })
      } catch (e) {
        console.error('[oa-bridge] recordPermissionResponse failed:', e)
      }
      return response
    },
  }

  const clientConn = createClientConnection(() => client as any, clientStream)

  // Initialize
  const init = await clientConn.initialize({
    protocolVersion: PROTOCOL_VERSION,
    clientCapabilities: { fs: { readTextFile: true } },
  })
  console.log('\n[ACP initialize.result]')
  console.log(JSON.stringify(init, null, 2))

  // Create a session
  const { sessionId, models, modes } = await clientConn.newSession({ cwd: process.cwd(), mcpServers: [] })
  console.log('\n[ACP newSession.result]')
  console.log(JSON.stringify({ sessionId, models, modes }, null, 2))

  // Upsert session metadata and claim token → set userId
  try {
    await convex.mutation(makeFunctionReference<'mutation'>('acp:upsertSession'), {
      sessionId,
      nowTs: Date.now(),
      patch: {
        currentModeId: modes?.currentModeId,
        models,
        agentInfo: init.agentInfo,
      },
      ...(cliToken ? { cliToken } : {}),
    })
    console.log('\n[oa-bridge] Session paired! Visit the web UI to see your session.\n')
    console.log('Claude Code will continue running. Press Ctrl+C to stop.\n')
  } catch (e) {
    console.error('[oa-bridge] upsertSession failed:', e)
  }

  // Keep the process alive
  await new Promise(() => {})
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
