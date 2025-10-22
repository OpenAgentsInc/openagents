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

    const interval = setInterval(handleCheckUpdate, 5000)
    void handleCheckUpdate()
    return () => clearInterval(interval)
  }, [])
}
