import React from 'react'
import { useLocalSearchParams } from 'expo-router'

export default function ThreadEntry() {
  // Only attempt thread creation once to avoid navigation loops when Convex isn't ready
  const attemptedRef = React.useRef(false)
  const params = useLocalSearchParams<{ new?: string; focus?: string }>()
  React.useEffect(() => {
    if (attemptedRef.current) return
    attemptedRef.current = true
    const run = async () => {
      try {
        const { router } = require('expo-router') as any
        const create = (require('convex/react') as any).useMutation('threads:create') as (args?: { title?: string }) => Promise<string>
        const id = await create({ title: 'New Thread' })
        router.replace(`/convex/thread/${encodeURIComponent(String(id))}?new=1`)
      } catch {
        // Stay on this screen with empty state; user can retry New Chat later
      }
    }
    run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])
  return null
}
