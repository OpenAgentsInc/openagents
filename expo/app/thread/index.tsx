import React from 'react'
import { ActivityIndicator, View } from 'react-native'
import { useHeaderTitle } from '@/lib/header-store'
import { useMutation } from 'convex/react'
import { router } from 'expo-router'
import { Colors } from '@/constants/theme'

export default function ThreadEntry() {
  useHeaderTitle('New Thread')
  const createThread = (useMutation as any)('threads:create') as (args?: { title?: string; projectId?: string }) => Promise<string>
  const [creating, setCreating] = React.useState(false)
  React.useEffect(() => {
    let done = false
    ;(async () => {
      if (creating) return
      setCreating(true)
      try {
        const id = await createThread({ title: 'New Thread' })
        if (!done && id) {
          try { router.replace(`/convex/thread/${encodeURIComponent(String(id))}?new=1`) } catch {}
        }
      } catch {
        setCreating(false)
      }
    })()
    return () => { done = true }
  }, [creating, createThread])
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
      <ActivityIndicator color={Colors.secondary} />
    </View>
  )
}
