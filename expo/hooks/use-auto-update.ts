import * as Updates from 'expo-updates'
import { useEffect } from 'react'
import { useUpdateStore } from '@/lib/update-store'
import { useSettings } from '@/lib/settings-store'
import { usePathname } from 'expo-router'

export const useAutoUpdate = () => {
  const setUpdating = useUpdateStore((s) => s.setUpdating)
  const autoPoll = useSettings((s) => s.updatesAutoPoll)
  const setLastRoute = useSettings((s) => s.setLastRoute)
  const pathname = usePathname()
  const handleCheckUpdate = async () => {
    if (__DEV__) return

    try {
      const result = await Updates.checkForUpdateAsync()
      console.log('Update check result:', result)

      if (result.isAvailable) {
        // Immediately flip UI into updating screen
        try { setUpdating(true) } catch {}
        try { if (pathname) setLastRoute(String(pathname)) } catch {}
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
    // Initial once-after-boot
    const t = setTimeout(() => { void handleCheckUpdate() }, 3000)
    let interval: any = null
    if (autoPoll) {
      // Poll every 5s when user enabled repeated syncing in settings
      interval = setInterval(() => { void handleCheckUpdate() }, 5000)
    }
    return () => { clearTimeout(t); if (interval) clearInterval(interval) }
  }, [autoPoll])
}
