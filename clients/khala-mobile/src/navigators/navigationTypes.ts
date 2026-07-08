import type { DrawerScreenProps } from "@react-navigation/drawer"
import type { CompositeScreenProps, NavigatorScreenParams } from "@react-navigation/native"
import type { NativeStackScreenProps } from "@react-navigation/native-stack"

/**
 * Navigation hierarchy (MM chat-header drawer rework, 2026-07-07): the Drawer
 * is now the ROOT navigator so the flyout menu overlays every in-app screen —
 * including the thread/chat view — and the chat header's hamburger can open it
 * via `navigation.getParent()?.openDrawer()`. The threads area (list + thread
 * view + repo picker + credit history) is a native stack hosted inside the
 * drawer's "Main" screen; Settings is a sibling drawer screen.
 */
export type AppStackParamList = {
  Threads: undefined
  CreditsHistory: undefined
  RepoPicker: {
    threadId: string
  }
  ThreadMessages: {
    createdLocally?: boolean
    threadId: string
    title?: string
  }
}

export type AppDrawerParamList = {
  Main: NavigatorScreenParams<AppStackParamList> | undefined
  FleetPeek: undefined
  Settings: undefined
}

/** A screen inside the threads native stack. `getParent()` reaches the root
 * Drawer (so the chat header's hamburger can `openDrawer()`). */
export type AppStackScreenProps<T extends keyof AppStackParamList> =
  CompositeScreenProps<
    NativeStackScreenProps<AppStackParamList, T>,
    DrawerScreenProps<AppDrawerParamList>
  >

/** A screen mounted directly on the root Drawer (e.g. Settings). */
export type AppDrawerScreenProps<T extends keyof AppDrawerParamList> =
  DrawerScreenProps<AppDrawerParamList, T>
