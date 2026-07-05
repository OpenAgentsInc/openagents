import { useEffect, useRef } from "react"
import { BackHandler, Platform } from "react-native"
import {
  createNavigationContainerRef,
  type NavigationState,
  type PartialState
} from "@react-navigation/native"

import type { AppStackParamList } from "./navigationTypes"

export const navigationRef = createNavigationContainerRef<AppStackParamList>()

export const getActiveRouteName = (
  state: NavigationState | PartialState<NavigationState>,
): string => {
  const route = state.routes[state.index ?? 0]
  if (route === undefined) return ""
  if (route.state === undefined) return String(route.name)
  return getActiveRouteName(route.state as NavigationState)
}

const iosExit = () => false

export const useBackButtonHandler = (canExit: (routeName: string) => boolean) => {
  const canExitRef = useRef(Platform.OS === "android" ? canExit : iosExit)

  useEffect(() => {
    canExitRef.current = Platform.OS === "android" ? canExit : iosExit
  }, [canExit])

  useEffect(() => {
    const onBackPress = () => {
      if (!navigationRef.isReady()) return false
      const routeName = getActiveRouteName(navigationRef.getRootState())
      if (canExitRef.current(routeName)) {
        BackHandler.exitApp()
        return true
      }
      if (navigationRef.canGoBack()) {
        navigationRef.goBack()
        return true
      }
      return false
    }

    const subscription = BackHandler.addEventListener("hardwareBackPress", onBackPress)
    return () => subscription.remove()
  }, [])
}

export const goBack = () => {
  if (navigationRef.isReady() && navigationRef.canGoBack()) {
    navigationRef.goBack()
  }
}

export const resetRoot = (
  state: Parameters<typeof navigationRef.resetRoot>[0] = { index: 0, routes: [] },
) => {
  if (navigationRef.isReady()) {
    navigationRef.resetRoot(state)
  }
}
