import { mkdir, appendFile } from 'node:fs/promises'
import path from 'node:path'

export type LogRecord = { event: string; payload: unknown } & Record<string, unknown>

export interface RunLogger {
  write: (record: LogRecord) => Promise<void>
  logFilePath: string
}

export async function createRunLogger(baseDir?: string): Promise<RunLogger> {
  const logsDir = path.resolve(baseDir ?? process.cwd(), 'logs')
  await mkdir(logsDir, { recursive: true })
  const runId = new Date().toISOString().replace(/[:.]/g, '-')
  const logFilePath = path.join(logsDir, `acp-${runId}.jsonl`)

  const write = async (record: LogRecord) => {
    try {
      const line = JSON.stringify({ ts: Date.now(), ...record }) + '\n'
      await appendFile(logFilePath, line)
    } catch (e) {
      // If we can't write to file, surface a concise error but don't crash the run
      console.error('[oa-bridge logger] write error:', e)
    }
  }

  return { write, logFilePath }
}

