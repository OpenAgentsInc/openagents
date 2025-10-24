import AsyncStorage from '@react-native-async-storage/async-storage'
import { create } from 'zustand'
import { persist, createJSONStorage } from 'zustand/middleware'

export type SkillId = string

export type Skill = {
  id: SkillId
  name: string
  description: string
  license?: string | null
  allowed_tools?: string[] | null
  metadata?: any
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
      storage: createJSONStorage(() => AsyncStorage),
    }
  )
)

export async function hydrateSkills(): Promise<void> {
  try { await useSkillsStore.persist.rehydrate?.() } catch {}
}

export function listSkills(): Skill[] { return useSkillsStore.getState().list() }

export function setAllSkills(arr: Skill[]): void { useSkillsStore.getState().setAll(arr) }

