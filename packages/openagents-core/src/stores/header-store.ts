import { create } from 'zustand'

type HeaderState = {
  title: string
  subtitle: string
  height: number
  setTitle: (v: string) => void
  setSubtitle: (v: string) => void
  setHeight: (v: number) => void
}

export const useHeaderStore = create<HeaderState>((set) => ({
  title: 'OpenAgents',
  subtitle: '',
  height: 0,
  setTitle: (title) => set({ title }),
  setSubtitle: (subtitle) => set({ subtitle }),
  setHeight: (height) => set({ height }),
}))

