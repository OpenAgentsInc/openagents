import React from 'react'
import { ActivityIndicator, View, Text, Pressable } from 'react-native'
import { useHeaderTitle } from '@/lib/header-store'
import { useMutation, useQuery } from 'convex/react'
import { router } from 'expo-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export default function ThreadEntry() {
  useHeaderTitle('New Thread')
  const createThread = (useMutation as any)('threads:create') as (args?: { title?: string; projectId?: string }) => Promise<string>
  const [creating, setCreating] = React.useState(false)
  const [error, setError] = React.useState<string | null>(null)
  // Use a lightweight readiness probe: if threads:list is undefined ⇒ loading; null ⇒ functions missing/unreachable; array ⇒ ready
  const probe = (useQuery as any)('threads:list', {}) as any[] | undefined | null
  const ready = Array.isArray(probe)
  const triedRef = React.useRef(false)
  const kickCreate = React.useCallback(async () => {
    setError(null)
    setCreating(true)
    try {
      const id = await createThread({ title: 'New Thread' })
      if (id) {
        try { router.replace(`/convex/thread/${encodeURIComponent(String(id))}?new=1`) } catch {}
        return
      }
      setError('Failed to create thread.')
    } catch (e: any) {
      setError(String(e?.message || 'Failed to create thread.'))
    } finally {
      setCreating(false)
    }
  }, [createThread])
  React.useEffect(() => {
    if (triedRef.current) return
    if (!ready) return
    triedRef.current = true
    void kickCreate()
  }, [ready, kickCreate])
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center', padding: 24 }}>
      {ready ? (
        creating ? (
          <ActivityIndicator color={Colors.secondary} />
        ) : (
          <>
            {error ? (<Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginBottom: 10 }}>{error}</Text>) : (
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginBottom: 10 }}>Create a new thread in Convex.</Text>
            )}
            <Pressable onPress={() => { try { void kickCreate() } catch {} }} accessibilityRole="button" style={{ backgroundColor: Colors.foreground, paddingHorizontal: 14, paddingVertical: 8, borderRadius: 8 }}>
              <Text style={{ color: Colors.black, fontFamily: Typography.bold }}>Create Thread</Text>
            </Pressable>
          </>
        )
      ) : probe === undefined ? (
        <ActivityIndicator color={Colors.secondary} />
      ) : (
        <>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginBottom: 10 }}>Connecting to Convex…</Text>
          <ActivityIndicator color={Colors.secondary} />
        </>
      )}
    </View>
  )
}
