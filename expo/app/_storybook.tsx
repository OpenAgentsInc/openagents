import React from 'react'
import { Platform } from 'react-native'

export default function StorybookRoot() {
  const Comp = React.useMemo<React.ComponentType | null>(() => {
    if (Platform.OS === 'web') return null
    try {
      // Avoid static analysis resolving RN Storybook on web by using eval(require)
      // eslint-disable-next-line no-eval
      const req = eval('require') as (id: string) => any
      const mod = req('../.rnstorybook')
      return (mod && mod.default) ? (mod.default as React.ComponentType) : null
    } catch {
      return null
    }
  }, [])
  if (!Comp) return null
  return <Comp />
}
