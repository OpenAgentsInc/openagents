import * as Updates from 'expo-updates'
import { useEffect } from 'react'

export const useAutoUpdate = () => {
  const handleCheckUpdate = async () => {
    if (__DEV__) return

    try {
      const result = await Updates.checkForUpdateAsync()
      console.log('Update check result:', result)

      if (result.isAvailable) {
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
    }
  }

  useEffect(() => {
    if (__DEV__) return
    // Check once shortly after startup. Excessive polling can stress expo-updates DB on iOS.
    const t = setTimeout(() => { void handleCheckUpdate() }, 3000)
    return () => clearTimeout(t)
  }, [])
}
