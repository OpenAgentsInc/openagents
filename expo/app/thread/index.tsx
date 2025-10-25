import React from 'react'
import { View, KeyboardAvoidingView, Platform } from 'react-native'
import { useLocalSearchParams, router } from 'expo-router'
import { useMutation } from 'convex/react'
import { Colors } from '@/constants/theme'
import { useHeaderTitle, useHeaderStore } from '@/lib/header-store'
import { Composer } from '@/components/composer'
import { useSafeAreaInsets } from 'react-native-safe-area-context'

export default function ThreadEntry() {
  const params = useLocalSearchParams<{ new?: string; focus?: string }>()
  const insets = useSafeAreaInsets()
  const headerHeight = useHeaderStore((s) => s.height)
  const createThread = (useMutation as any)('threads:create') as (args?: { title?: string }) => Promise<string>
  const inputRef = React.useRef<any>(null)
  useHeaderTitle('New Thread')

  // Try to create a thread once on mount; if it fails, keep fallback screen visible
  const attemptedRef = React.useRef(false)
  React.useEffect(() => {
    if (attemptedRef.current) return
    attemptedRef.current = true
    ;(async () => {
      try {
        const id = await createThread({ title: 'New Thread' })
        router.replace(`/convex/thread/${encodeURIComponent(String(id))}?new=1`)
      } catch {}
    })()
  }, [createThread])

  React.useEffect(() => {
    const t = setTimeout(() => { try { inputRef.current?.focus?.() } catch {} }, 150)
    return () => clearTimeout(t)
  }, [])

  return (
    <KeyboardAvoidingView behavior={Platform.OS === 'ios' ? 'padding' : 'height'} keyboardVerticalOffset={headerHeight + 4} style={{ flex: 1 }}>
      <View style={{ flex: 1, backgroundColor: Colors.background, justifyContent: 'flex-end', paddingBottom: Math.max(insets.bottom, 8), paddingHorizontal: 8 }}>
        <Composer
          onSend={async (txt) => {
            const base = String(txt || '').trim()
            if (!base) return
            try { const id = await createThread({ title: 'New Thread' }); router.replace(`/convex/thread/${encodeURIComponent(String(id))}?new=1&send=${encodeURIComponent(base)}`) } catch {}
          }}
          connected={true}
          isRunning={false}
          onQueue={() => {}}
          onInterrupt={() => {}}
          queuedMessages={[]}
          prefill={null}
          onDraftChange={() => {}}
          inputRef={inputRef}
        />
      </View>
    </KeyboardAvoidingView>
  )
}
