import { create } from 'zustand/traditional'
import { persist } from 'zustand/traditional/middleware'
import { persistStorage } from './persist-storage'

export type SkillId = string

export type Skill = {
  id: SkillId
  name: string
  description: string
  license?: string | null
  allowed_tools?: string[] | null
  metadata?: any
  source?: 'user' | 'registry' | 'project'
  projectId?: string | null
}

type SkillsState = {
  items: Record<SkillId, Skill>
  list: () => Skill[]
  get: (id: SkillId) => Skill | undefined
  setAll: (arr: Skill[]) => void
}

export const useSkillsStore = create<SkillsState>()(
  persist(
    (set, get) => ({
      items: {},
      list: () => Object.values(get().items).sort((a, b) => a.name.localeCompare(b.name)),
      get: (id) => get().items[id],
      setAll: (arr) => set(() => ({ items: Object.fromEntries(arr.map(s => [s.id, s])) })),
    }),
    {
      name: '@openagents/skills-v1',
      version: 1,
      storage: persistStorage(),
    }
  )
)

export async function hydrateSkills(): Promise<void> {
  try { await useSkillsStore.persist.rehydrate?.() } catch {}
}

export function listSkills(): Skill[] { return useSkillsStore.getState().list() }

export function setAllSkills(arr: Skill[]): void { useSkillsStore.getState().setAll(arr) }
