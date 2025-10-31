import * as Updates from 'expo-updates'
import { useEffect } from 'react'
import { useUpdateStore } from '@/lib/update-store'

export const useAutoUpdate = () => {
  const setUpdating = useUpdateStore((s) => s.setUpdating)
  const handleCheckUpdate = async () => {
    if (__DEV__) return

    try {
      const result = await Updates.checkForUpdateAsync()
      console.log('Update check result:', result)

      if (result.isAvailable) {
        // Immediately flip UI into updating screen
        try { setUpdating(true) } catch {}
        console.log('Update available, downloading...')
        const downloadResult = await Updates.fetchUpdateAsync()
        console.log('Update download result:', downloadResult)
        if (downloadResult) {
          console.log('Reloading with new update...')
          await Updates.reloadAsync()
        }
      }
    } catch (error) {
      console.error('Error checking/downloading update:', error)
    } finally {
      // In normal flows, reloadAsync ends the process; this is just a fallback
      try { setUpdating(false) } catch {}
    }
  }

  useEffect(() => {
    if (__DEV__) return
    // Check once shortly after startup. Excessive polling can stress expo-updates DB on iOS.
    const t = setTimeout(() => { void handleCheckUpdate() }, 3000)
    return () => clearTimeout(t)
  }, [])
}
