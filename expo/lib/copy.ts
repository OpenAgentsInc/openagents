import * as Clipboard from 'expo-clipboard'
import * as Haptics from 'expo-haptics'
import { toast } from '@/lib/toast-store'

export async function copyToClipboard(text: string, opts?: { haptics?: boolean }) {
  try {
    await Clipboard.setStringAsync(String(text ?? ''))
    if (opts?.haptics && process.env.EXPO_OS === 'ios') {
      try { await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success) } catch {}
    }
    try { toast('Copied to clipboard', { type: 'success', duration: 1400 }) } catch {}
  } catch {}
}
