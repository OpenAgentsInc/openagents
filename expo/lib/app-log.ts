import { useAppLogStore } from '@openagentsinc/core'
import type { LogLevel } from '@openagentsinc/core'

export function appLog(event: string, details?: any, level: LogLevel = 'info') {
  try { useAppLogStore.getState().add(level, event, details) } catch {}
}
