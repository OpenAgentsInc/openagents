import React from 'react'
import { useLocalSearchParams } from 'expo-router'

export default function ThreadEntry() {
  const params = useLocalSearchParams<{ new?: string; focus?: string }>()
  React.useEffect(() => {
    const run = async () => {
      try {
        const { router } = require('expo-router') as any
        const create = (require('convex/react') as any).useMutation('threads:create') as (args?: { title?: string }) => Promise<string>
        const id = await create({ title: 'New Thread' })
        router.replace(`/convex/thread/${encodeURIComponent(String(id))}`)
      } catch {
        try { (require('expo-router') as any).router.replace('/convex') } catch {}
      }
    }
    run()
  }, [params?.new])
  return null
}
