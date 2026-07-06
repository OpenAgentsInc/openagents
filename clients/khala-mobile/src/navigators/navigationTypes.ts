import type { DrawerScreenProps } from "@react-navigation/drawer"
import type { CompositeScreenProps, NavigatorScreenParams } from "@react-navigation/native"
import type { NativeStackScreenProps } from "@react-navigation/native-stack"

export type AppDrawerParamList = {
  Threads: undefined
  Settings: undefined
}

export type AppStackParamList = {
  Home: NavigatorScreenParams<AppDrawerParamList> | undefined
  CreditsHistory: undefined
  RepoPicker: {
    threadId: string
  }
  ThreadMessages: {
    threadId: string
    title?: string
  }
}

export type AppStackScreenProps<T extends keyof AppStackParamList> =
  NativeStackScreenProps<AppStackParamList, T>

export type AppDrawerScreenProps<T extends keyof AppDrawerParamList> =
  CompositeScreenProps<
    DrawerScreenProps<AppDrawerParamList, T>,
    AppStackScreenProps<keyof AppStackParamList>
  >
