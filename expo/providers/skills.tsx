import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react'
import { useBridge } from '@/providers/ws'
import { hydrateSkills, listSkills, setAllSkills, type Skill } from '@/lib/skills-store'

type SkillsCtx = { skills: Skill[] }

const Ctx = createContext<SkillsCtx | undefined>(undefined)

export function SkillsProvider({ children }: { children: React.ReactNode }) {
  const [skills, setSkills] = useState<Skill[]>([])
  const ws = useBridge()

  useEffect(() => { (async () => { await hydrateSkills(); setSkills(listSkills()) })() }, [])

  // Seed from bridge; also listen for push updates
  useEffect(() => {
    let unsub: (() => void) | null = null
    ;(async () => {
      try {
        const items = await ws.requestSkills()
        if (Array.isArray(items)) {
          const arr: Skill[] = items.map((x: any) => ({
            id: String(x.id || x.name || ''),
            name: String(x.name || x.id || ''),
            description: String(x.description || ''),
            license: x.license ?? null,
            allowed_tools: Array.isArray(x.allowed_tools) ? x.allowed_tools : Array.isArray(x['allowed-tools']) ? x['allowed-tools'] : null,
            metadata: x.metadata ?? null,
          }))
          setAllSkills(arr)
          setSkills(listSkills())
        }
      } catch {}
      // Subscribe for live updates
      unsub = ws.addSubscriber((line) => {
        try {
          const s = String(line || '').trim(); if (!s.startsWith('{')) return
          const obj = JSON.parse(s)
          if (obj?.type === 'bridge.skills' && Array.isArray(obj.items)) {
            const arr: Skill[] = obj.items.map((x: any) => ({
              id: String(x.id || x.name || ''),
              name: String(x.name || x.id || ''),
              description: String(x.description || ''),
              license: x.license ?? null,
              allowed_tools: Array.isArray(x.allowed_tools) ? x.allowed_tools : Array.isArray(x['allowed-tools']) ? x['allowed-tools'] : null,
              metadata: x.metadata ?? null,
            }))
            setAllSkills(arr)
            setSkills(listSkills())
          }
        } catch {}
      })
    })()
    return () => { try { unsub?.() } catch {} }
  }, [ws])

  const value = useMemo<SkillsCtx>(() => ({ skills }), [skills])
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>
}

export function useSkills() {
  const ctx = useContext(Ctx)
  if (!ctx) throw new Error('useSkills must be used within SkillsProvider')
  return ctx
}

