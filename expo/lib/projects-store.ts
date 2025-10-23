import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

export type ProjectId = string

export type ProjectRepo = {
  provider?: 'github' | 'gitlab' | 'other'
  remote?: string // "owner/name"
  url?: string // e.g. https://github.com/owner/name
  branch?: string
}

export type ProjectTodo = { text: string; completed: boolean }

export type Project = {
  id: ProjectId
  name: string
  voiceAliases: string[]
  workingDir: string
  repo?: ProjectRepo
  agentFile?: string
  instructions?: string

  runningAgents?: number
  attentionCount?: number
  todos?: ProjectTodo[]

  lastActivity?: number
  createdAt: number
  updatedAt: number

  approvals?: 'never' | 'on-request' | 'on-failure'
  model?: string
  sandbox?: 'danger-full-access' | 'workspace-write' | 'read-only'
}

type ProjectsState = {
  items: Record<ProjectId, Project>
  activeId: ProjectId | null
  list: () => Project[]
  get: (id: ProjectId) => Project | undefined
  getActive: () => Project | undefined
  setActive: (id: ProjectId | null) => void
  upsert: (p: Project) => void
  remove: (id: ProjectId) => void
  mergeTodos: (projectId: ProjectId, todos: ProjectTodo[]) => void
}

export const useProjectsStore = create<ProjectsState>()(
  persist(
    (set, get) => ({
      items: {},
      activeId: null,
      list: () => Object.values(get().items).sort((a, b) => a.name.localeCompare(b.name)),
      get: (id) => get().items[id],
      getActive: () => {
        const id = get().activeId
        return id ? get().items[id] : undefined
      },
      setActive: (id) => set({ activeId: id }),
      upsert: (p) => set(({ items }) => ({ items: { ...items, [p.id]: { ...p, updatedAt: Date.now(), createdAt: p.createdAt ?? Date.now() } } })),
      remove: (id) => set(({ items, activeId }) => {
        const next = { ...items }
        delete next[id]
        return { items: next, activeId: activeId === id ? null : activeId }
      }),
      mergeTodos: (projectId, todos) => set(({ items }) => {
        const p = items[projectId]
        if (!p) return { items }
        const next: Project = { ...p, todos, attentionCount: todos.filter(t => !t.completed).length, updatedAt: Date.now() }
        return { items: { ...items, [projectId]: next } }
      }),
    }),
    {
      name: '@openagents/projects-v2',
      version: 1,
      storage: createJSONStorage(() => AsyncStorage),
      migrate: (persisted: any) => {
        // Accept legacy shape: a record of projects in plain object and optional activeId
        try {
          if (persisted && typeof persisted === 'object' && !('items' in persisted)) {
            // Try KEY format
            return { items: persisted as Record<ProjectId, Project>, activeId: null }
          }
        } catch {}
        return persisted
      },
    }
  )
)

// Back-compat procedural API used by providers/projects and others
export async function hydrateProjects(): Promise<void> {
  try { await useProjectsStore.persist.rehydrate?.() } catch {}
  // Legacy migration path: import from old AsyncStorage keys if current store is empty
  try {
    const cur = useProjectsStore.getState()
    if (Object.keys(cur.items || {}).length === 0) {
      const raw = await AsyncStorage.getItem('@openagents/projects-v1')
      if (raw) {
        const obj = JSON.parse(raw) as Record<ProjectId, Project>
        const active = await AsyncStorage.getItem('@openagents/projects-active-v1')
        if (obj && typeof obj === 'object') {
          useProjectsStore.setState({ items: obj, activeId: active as ProjectId | null })
        }
      }
    }
  } catch {}
}

export function listProjects(): Project[] {
  return useProjectsStore.getState().list()
}

export function getProject(id: ProjectId): Project | undefined {
  return useProjectsStore.getState().get(id)
}

export function getActiveProject(): Project | undefined {
  return useProjectsStore.getState().getActive()
}

export async function setActiveProject(id: ProjectId | null): Promise<void> {
  useProjectsStore.getState().setActive(id)
}

export async function upsertProject(p: Project): Promise<void> {
  useProjectsStore.getState().upsert(p)
}

export async function removeProject(id: ProjectId): Promise<void> {
  useProjectsStore.getState().remove(id)
}

export async function mergeProjectTodos(projectId: ProjectId, todos: ProjectTodo[]): Promise<void> {
  useProjectsStore.getState().mergeTodos(projectId, todos)
}
