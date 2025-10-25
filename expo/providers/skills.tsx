import React, { createContext, useContext, useEffect, useMemo, useState } from 'react'
import { useQuery } from 'convex/react'
import { hydrateSkills, listSkills, setAllSkills, type Skill } from '@/lib/skills-store'

type SkillsCtx = { skills: Skill[] }

const Ctx = createContext<SkillsCtx | undefined>(undefined)

export function SkillsProvider({ children }: { children: React.ReactNode }) {
  const [skills, setSkills] = useState<Skill[]>([])
  // Rehydrate from AsyncStorage immediately for instant UI
  useEffect(() => { (async () => { await hydrateSkills(); setSkills(listSkills()) })() }, [])
  // Live list from Convex; undefined while connecting, null if function/schema missing
  const convexSkills = (useQuery as any)('skills:listAll', {}) as any[] | undefined | null
  useEffect(() => {
    if (Array.isArray(convexSkills)) {
      const arr: Skill[] = convexSkills.map((x: any) => ({
        id: String(x.skillId || x.id || x.name || ''),
        name: String(x.name || x.skillId || x.id || ''),
        description: String(x.description || ''),
        license: x.license ?? null,
        allowed_tools: Array.isArray(x.allowed_tools) ? x.allowed_tools : Array.isArray(x['allowed-tools']) ? x['allowed-tools'] : null,
        metadata: x.metadata ?? null,
        source: ((): any => { const s = String(x.source || ''); return s === 'project' || s === 'user' || s === 'registry' ? s : undefined })(),
        projectId: typeof x.projectId === 'string' ? x.projectId : null,
      }))
      setAllSkills(arr)
      setSkills(listSkills())
    }
  }, [convexSkills])

  const value = useMemo<SkillsCtx>(() => ({ skills }), [skills])
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useSkills() {
  const ctx = useContext(Ctx)
  if (!ctx) throw new Error('useSkills must be used within SkillsProvider')
  return ctx
}
