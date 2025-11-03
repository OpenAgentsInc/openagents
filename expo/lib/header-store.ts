import React from 'react'
import { useFocusEffect } from 'expo-router'
import { useHeaderStore } from '@openagentsinc/core'

export { useHeaderStore };

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
