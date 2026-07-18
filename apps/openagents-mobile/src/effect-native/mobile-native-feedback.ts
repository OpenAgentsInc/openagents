import { LayoutAnimation, Platform, UIManager } from "react-native"
import * as Haptics from "expo-haptics"
import { mobileIntentUsesRouteTransition, mobileNativeFeedbackKind } from "./mobile-native-feedback-policy"

export const enableMobileLayoutAnimation = (): void => {
  if (Platform.OS === "android" && UIManager.setLayoutAnimationEnabledExperimental !== undefined) {
    UIManager.setLayoutAnimationEnabledExperimental(true)
  }
}

export const prepareMobileNativeIntentFeedback = (
  intentName: string,
  reduceMotion: boolean,
): void => {
  if (!reduceMotion && mobileIntentUsesRouteTransition(intentName)) {
    LayoutAnimation.configureNext({
      duration: 180,
      create: { type: LayoutAnimation.Types.easeInEaseOut, property: LayoutAnimation.Properties.opacity },
      update: { type: LayoutAnimation.Types.easeInEaseOut },
      delete: { type: LayoutAnimation.Types.easeInEaseOut, property: LayoutAnimation.Properties.opacity },
    })
  }
  const kind = mobileNativeFeedbackKind(intentName)
  if (kind === "selection") void Haptics.selectionAsync().catch(() => undefined)
  else if (kind === "action") void Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => undefined)
  else if (kind === "warning") void Haptics.notificationAsync(Haptics.NotificationFeedbackType.Warning).catch(() => undefined)
}
