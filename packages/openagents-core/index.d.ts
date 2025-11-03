import type { ComponentType, ReactNode } from 'react'

export type ThreadListItemProps = {
  title: string
  meta?: ReactNode
  timestamp?: number | null
  count?: number | null
  onPress?: () => void
  onLongPress?: () => void
  testID?: string
}
export const ThreadListItem: ComponentType<ThreadListItemProps>

export type ChatMessageBubbleProps = { role: 'assistant' | 'user'; text: string }
export const ChatMessageBubble: ComponentType<ChatMessageBubbleProps>

export const useHeaderStore: typeof import('./src/stores/header-store').useHeaderStore

export const useSettings: typeof import('./src/stores/settings-store').useSettings
export type Approvals = import('./src/stores/settings-store').Approvals

export const useThreadProviders: typeof import('./src/stores/thread-provider-store').useThreadProviders
export type AgentProvider = import('./src/stores/thread-provider-store').AgentProvider

export const useArchiveStore: typeof import('./src/stores/archive-store').useArchiveStore
export const useToastStore: typeof import('./src/stores/toast-store').useToastStore
export const usePairingStore: typeof import('./src/stores/pairing-store').usePairingStore
export const useAppLogStore: typeof import('./src/stores/app-log').useAppLogStore
export type LogLevel = import('./src/stores/app-log').LogLevel
export const useDrawerStore: typeof import('./src/stores/drawer-store').useDrawerStore

export const useSkillsStore: typeof import('./src/stores/skills-store').useSkillsStore
export type Skill = import('./src/stores/skills-store').Skill
export type SkillId = import('./src/stores/skills-store').SkillId

export const useProjectsStore: typeof import('./src/stores/projects-store').useProjectsStore
export type Project = import('./src/stores/projects-store').Project
export type ProjectId = import('./src/stores/projects-store').ProjectId
export type ProjectRepo = import('./src/stores/projects-store').ProjectRepo
export type ProjectTodo = import('./src/stores/projects-store').ProjectTodo

export const useThreads: typeof import('./src/stores/threads-store').useThreads
export const ensureThreadsRehydrated: typeof import('./src/stores/threads-store').ensureThreadsRehydrated
export type ThreadItem = import('./src/stores/threads-store').ThreadItem
export type HistoryItem = import('./src/stores/threads-store').HistoryItem
export type ThreadResponse = import('./src/stores/threads-store').ThreadResponse
