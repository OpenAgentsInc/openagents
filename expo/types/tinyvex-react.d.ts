declare module 'tinyvex/react' {
  import type React from 'react'
  import type { MessageRowTs } from '@/types/bridge/MessageRowTs'
  import type { ThreadSummaryTs } from '@/types/bridge/ThreadSummaryTs'
  
  // Provider from the tinyvex package
  export function TinyvexProvider(props: { config?: { url?: string; token?: string | null }; children: React.ReactNode }): JSX.Element

  // Threads list hook (subset of fields used by the app)
  export function useTinyvexThreads(limit?: number): {
    threads: Array<ThreadSummaryTs & {
      // Optional convenience fields as numbers where bigint may be present
      created_at?: number
      updated_at?: number
      last_message_ts?: number | null
      message_count?: number | null
      source?: string
      title?: string | null
    }>
  }

  // Single thread hook: history + live
  export function useTinyvexThread(args: { idOrAlias: string }): {
    status: 'idle' | 'connecting' | 'ready' | 'error'
    threadId?: string
    history: MessageRowTs[]
    live: { assistant: string; thought?: string }
    send: (text: string, opts?: { resumeId?: 'last'; provider?: string }) => unknown
    refresh: () => unknown
    debug?: unknown
  }
}

