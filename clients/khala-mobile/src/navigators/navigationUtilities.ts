import { useEffect, useRef } from "react"
import { BackHandler, Platform } from "react-native"
import {
  createNavigationContainerRef,
  type NavigationState,
  type PartialState
} from "@react-navigation/native"

import type { AppDrawerParamList } from "./navigationTypes"

export const navigationRef = createNavigationContainerRef<AppDrawerParamList>()

export type KhalaBackAction = "exit_app" | "go_back" | "ignore"

export type KhalaNavigationPersistenceDecision = Readonly<{
  enabled: false
  reason: string
}>

export const KHALA_NAVIGATION_PERSISTENCE_DECISION: KhalaNavigationPersistenceDecision = {
  enabled: false,
  reason:
    "Navigation state is not persisted yet. Thread route params may contain private thread refs/titles, so persistence stays off until a route-name-only snapshot format is introduced.",
}

export const getActiveRouteName = (
  state: NavigationState | PartialState<NavigationState>,
): string => {
  const route = state.routes[state.index ?? 0]
  if (route === undefined) return ""
  if (route.state === undefined) return String(route.name)
  return getActiveRouteName(route.state as NavigationState)
}

export const routeNameSummary = (
  state: NavigationState | PartialState<NavigationState>,
): Readonly<{ activeRouteName: string }> => ({
  activeRouteName: getActiveRouteName(state),
})

export const decideBackAction = (input: {
  readonly canExitRoute: boolean
  readonly canGoBack: boolean
  readonly isAndroid: boolean
}): KhalaBackAction => {
  if (!input.isAndroid) return "ignore"
  if (input.canExitRoute) return "exit_app"
  if (input.canGoBack) return "go_back"
  return "ignore"
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
      const action = decideBackAction({
        canExitRoute: canExitRef.current(routeName),
        canGoBack: navigationRef.canGoBack(),
        isAndroid: Platform.OS === "android",
      })
      if (action === "exit_app") {
        BackHandler.exitApp()
        return true
      }
      if (action === "go_back") {
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
