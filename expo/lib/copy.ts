import * as Clipboard from 'expo-clipboard'
import * as Haptics from 'expo-haptics'

export async function copyToClipboard(text: string, opts?: { haptics?: boolean }) {
  try {
    await Clipboard.setStringAsync(String(text ?? ''))
    if (opts?.haptics && process.env.EXPO_OS === 'ios') {
      try { await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success) } catch {}
    }
  } catch {}
}

