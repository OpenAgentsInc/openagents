import React from 'react'
import { useLocalSearchParams } from 'expo-router'

export default function ThreadRedirect() {
  const { id } = useLocalSearchParams<{ id: string }>()
  React.useEffect(() => {
    if (id) {
      try { (require('expo-router') as any).router.replace(`/convex/thread/${encodeURIComponent(String(id))}`) } catch {}
    }
  }, [id])
  return null
}
