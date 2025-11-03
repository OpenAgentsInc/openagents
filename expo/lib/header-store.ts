import React from 'react'
import { useFocusEffect } from 'expo-router'
import { create } from 'zustand'

type HeaderState = {
  title: string
  setTitle: (t: string) => void
  subtitle: string
  setSubtitle: (t: string) => void
  height: number
  setHeight: (h: number) => void
}

export const useHeaderStore = create<HeaderState>((set) => ({
  title: '',
  setTitle: (t) => set({ title: t }),
  subtitle: '',
  setSubtitle: (t) => set({ subtitle: t }),
  height: 0,
  setHeight: (h) => set({ height: h }),
}))

export function useHeaderTitle(title: string) {
  const setTitle = useHeaderStore((s) => s.setTitle)
  // Set on mount and whenever the screen regains focus (e.g., after back navigation)
  React.useEffect(() => { setTitle(title) }, [setTitle, title])
  useFocusEffect(React.useCallback(() => { setTitle(title); return () => {} }, [setTitle, title]))
}

export function useHeaderSubtitle(subtitle: string) {
  const setSubtitle = useHeaderStore((s) => s.setSubtitle)
  React.useEffect(() => { setSubtitle(subtitle) }, [setSubtitle, subtitle])
  useFocusEffect(React.useCallback(() => { setSubtitle(subtitle); return () => {} }, [setSubtitle, subtitle]))
}
