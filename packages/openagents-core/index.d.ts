declare module '@openagentsinc/core' {
  import type { ReactNode } from 'react'

  // Components
  export type ThreadListItemProps = {
    title: string
    meta?: ReactNode
    timestamp?: number | null
    count?: number | null
    onPress?: () => void
    onLongPress?: () => void
    testID?: string
  }
  export function ThreadListItem(props: ThreadListItemProps): JSX.Element

  export type ChatMessageBubbleProps = { role: 'assistant' | 'user'; text: string }
  export function ChatMessageBubble(props: ChatMessageBubbleProps): JSX.Element

  // Generic store hook type (Zustand-like)
  type UseStore<S> = {
    (): S
    <T>(selector: (s: S) => T): T
    getState: () => S
    persist?: { rehydrate?: () => Promise<void> }
  }

  // Header store
  export type HeaderStore = {
    title: string
    subtitle: string
    height: number
    setTitle: (v: string) => void
    setSubtitle: (v: string) => void
    setHeight: (v: number) => void
  }
  export const useHeaderStore: UseStore<HeaderStore>

  // Pairing
  export type PairingStore = { deeplinkPairing: boolean; setDeeplinkPairing: (v: boolean) => void }
  export const usePairingStore: UseStore<PairingStore>

  // Drawer
  export type DrawerStore = { open: boolean; setOpen: (v: boolean) => void }
  export const useDrawerStore: UseStore<DrawerStore>

  // Toasts
  export type ToastType = 'info' | 'success' | 'error'
  export type Toast = { id: string; text: string; type: ToastType; duration: number }
  export type ToastStore = {
    toasts: Toast[]
    enqueue: (t: Omit<Toast, 'id'> & { id?: string }) => string
    remove: (id: string) => void
    clear: () => void
  }
  export const useToastStore: UseStore<ToastStore>

  // App log
  export type LogLevel = 'debug' | 'info' | 'warn' | 'error'
  export type AppLogItem = { ts: number; level: LogLevel; event: string; details?: unknown }
  export type AppLogStore = { logs: AppLogItem[]; add: (level: LogLevel, event: string, details?: unknown) => void; clear: () => void }
  export const useAppLogStore: UseStore<AppLogStore>

  // Settings
  export type Approvals = 'never' | 'on-request' | 'on-failure'

  // Thread providers
  export type AgentProvider = 'codex' | 'claude_code'
  export type ThreadProviderState = { byThread: Record<string, AgentProvider>; setProvider: (threadId: string, provider: AgentProvider) => void }
  export const useThreadProviders: UseStore<ThreadProviderState>

  // Archive
  export type ArchiveStore = { archived: Record<string, true>; isArchived: (id: string) => boolean; archive: (id: string) => void; unarchive: (id: string) => void }
  export const useArchiveStore: UseStore<ArchiveStore>

  // Skills
  export type SkillId = string
  export type Skill = { id: SkillId; name: string; description: string; license?: string | null; allowed_tools?: string[] | null; metadata?: unknown; source?: 'user' | 'registry' | 'project'; projectId?: string | null }
  export type SkillsStore = { items: Record<SkillId, Skill>; list: () => Skill[]; get: (id: SkillId) => Skill | undefined; setAll: (arr: Skill[]) => void }
  export const useSkillsStore: UseStore<SkillsStore>

  // Projects
  export type ProjectId = string
  export type ProjectRepo = { provider?: 'github' | 'gitlab' | 'other'; remote?: string; url?: string; branch?: string }
  export type ProjectTodo = { text: string; completed: boolean }
  export type Project = { id: ProjectId; name: string; workingDir: string; repo?: ProjectRepo; agentFile?: string; instructions?: string; runningAgents?: number; attentionCount?: number; todos?: ProjectTodo[]; lastActivity?: number; createdAt: number; updatedAt: number; approvals?: Approvals; model?: string; sandbox?: 'danger-full-access' | 'workspace-write' | 'read-only' }
  export type ProjectsStore = { items: Record<ProjectId, Project>; activeId: ProjectId | null; list: () => Project[]; get: (id: ProjectId) => Project | undefined; getActive: () => Project | undefined; setActive: (id: ProjectId | null) => void; upsert: (p: Project) => void; remove: (id: ProjectId) => void; mergeTodos: (projectId: ProjectId, todos: ProjectTodo[]) => void; setAll: (arr: Project[]) => void }
  export const useProjectsStore: UseStore<ProjectsStore>

  // Threads
  export type ThreadItem = { ts: number; kind: 'message' | 'reason' | 'cmd'; role?: 'assistant' | 'user'; text: string }
  export type HistoryItem = { id: string; path: string; mtime: number; title: string; snippet: string; has_instructions?: boolean; tail?: ThreadItem[] }
  export type ThreadResponse = { title: string; items: ThreadItem[]; instructions?: string; resume_id?: string; partial?: boolean }
  export type ThreadsStore = {
    history: HistoryItem[]
    rehydrated: boolean
    thread: Record<string, ThreadResponse | undefined>
    threadProject: Record<string, string | undefined>
    setThreadProject: (threadId: string, projectId: string) => void
    primeThread: (id: string, preview: ThreadResponse) => void
    loadHistory: (requestHistory: (params?: { limit?: number; since_mtime?: number }) => Promise<HistoryItem[]>) => Promise<void>
    loadThread: (requestThread: (id: string, path?: string) => Promise<ThreadResponse | undefined>, id: string, path?: string) => Promise<ThreadResponse | undefined>
  }
  export const useThreads: UseStore<ThreadsStore>
  export function ensureThreadsRehydrated(): Promise<void>
}
