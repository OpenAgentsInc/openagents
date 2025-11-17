import { readFile } from 'node:fs/promises'

export interface LoggingClientOptions {
  writeLog: (record: { event: string; payload: unknown }) => Promise<void>
  streamTextToStdout?: boolean
}

/**
 * Returns a minimal ACP client implementation that:
 * - logs every sessionUpdate as full JSON via writeLog (and console)
 * - implements readTextFile (read-only)
 * - auto-approves the first permission option (also logged)
 */
export function createLoggingClient(opts: LoggingClientOptions) {
  const { writeLog, streamTextToStdout = true } = opts

  const client = {
    async sessionUpdate(params: any) {
      console.log('\n[ACP sessionUpdate]')
      console.log(JSON.stringify(params, null, 2))
      await writeLog({ event: 'sessionUpdate', payload: params })

      const u = params.update
      if (streamTextToStdout && u?.sessionUpdate === 'agent_message_chunk' && u.content?.type === 'text') {
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

  return client
}

