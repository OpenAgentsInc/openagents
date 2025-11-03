import { create } from "zustand"
import { persist } from "zustand/middleware"
import { universalJSONStorage } from "./persist"

export type ProjectId = string
export type ProjectRepo = { provider?: 'github' | 'gitlab' | 'other'; remote?: string; url?: string; branch?: string }
export type ProjectTodo = { text: string; completed: boolean }
export type Project = { id: ProjectId; name: string; workingDir: string; repo?: ProjectRepo; agentFile?: string; instructions?: string; runningAgents?: number; attentionCount?: number; todos?: ProjectTodo[]; lastActivity?: number; createdAt: number; updatedAt: number; approvals?: 'never' | 'on-request' | 'on-failure'; model?: string; sandbox?: 'danger-full-access' | 'workspace-write' | 'read-only' }

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
  setAll: (arr: Project[]) => void
}

export const useProjectsStore = create<ProjectsState>()(
  persist(
    (set, get) => ({
      items: {},
      activeId: null,
      list: () => Object.values(get().items).sort((a, b) => a.name.localeCompare(b.name)),
      get: (id) => get().items[id],
      getActive: () => { const id = get().activeId; return id ? get().items[id] : undefined },
      setActive: (id) => set({ activeId: id }),
      upsert: (p) => set(({ items }) => ({ items: { ...items, [p.id]: { ...p, updatedAt: Date.now(), createdAt: p.createdAt ?? Date.now() } } })),
      remove: (id) => set(({ items, activeId }) => { const next = { ...items }; delete next[id]; return { items: next, activeId: activeId === id ? null : activeId } }),
      mergeTodos: (projectId, todos) => set(({ items }) => { const p = items[projectId]; if (!p) return { items }; const next: Project = { ...p, todos, attentionCount: todos.filter(t => !t.completed).length, updatedAt: Date.now() }; return { items: { ...items, [projectId]: next } } }),
      setAll: (arr) => set(() => ({ items: Object.fromEntries(arr.map(p => [p.id, p])) })),
    }),
    { name: '@openagents/projects-v2', version: 1, storage: universalJSONStorage() }
  )
)
