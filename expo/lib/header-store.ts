import React from 'react'
import { create } from 'zustand'

type HeaderState = {
  title: string
  setTitle: (t: string) => void
  height: number
  setHeight: (h: number) => void
}

export const useHeaderStore = create<HeaderState>((set) => ({
  title: '',
  setTitle: (t) => set({ title: t }),
  height: 0,
  setHeight: (h) => set({ height: h }),
}))

export function useHeaderTitle(title: string) {
  const setTitle = useHeaderStore((s) => s.setTitle)
  React.useEffect(() => { setTitle(title) }, [setTitle, title])
}
