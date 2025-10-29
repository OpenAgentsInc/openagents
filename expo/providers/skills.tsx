import React, { createContext, useContext, useEffect, useMemo, useState } from 'react'
import { hydrateSkills, listSkills, setAllSkills, type Skill } from '@/lib/skills-store'

type SkillsCtx = { skills: Skill[] }

const Ctx = createContext<SkillsCtx | undefined>(undefined)

export function SkillsProvider({ children }: { children: React.ReactNode }) {
  const [skills, setSkills] = useState<Skill[]>([])
  // Rehydrate from AsyncStorage immediately for instant UI
  useEffect(() => { (async () => { await hydrateSkills(); setSkills(listSkills()) })() }, [])
  // Convex removed; keep only local hydration for now

  const value = useMemo<SkillsCtx>(() => ({ skills }), [skills])
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useSkills() {
  const ctx = useContext(Ctx)
  if (!ctx) throw new Error('useSkills must be used within SkillsProvider')
  return ctx
}
