import type { ComponentProps, ReactNode } from "react"
import {
  KeyboardAvoidingView,
  Platform,
  ScrollView,
  View,
  type ScrollViewProps,
} from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

type SafeAreaEdges = ComponentProps<typeof SafeAreaView>["edges"]
type KhalaScrollViewProps = ScrollViewProps & {
  readonly className?: string
  readonly contentContainerClassName?: string
}

export type KhalaScreenPreset = "fixed" | "keyboardAware" | "scroll"

export type KhalaScreenProps = Readonly<{
  children?: ReactNode
  className?: string
  contentClassName?: string
  edges?: SafeAreaEdges
  keyboardShouldPersistTaps?: ScrollViewProps["keyboardShouldPersistTaps"]
  keyboardVerticalOffset?: number
  preset?: KhalaScreenPreset
  scrollViewProps?: KhalaScrollViewProps
}>

const defaultEdges: SafeAreaEdges = ["top", "bottom", "left", "right"]

export const KhalaScreen = ({
  children,
  className = "",
  contentClassName = "",
  edges = defaultEdges,
  keyboardShouldPersistTaps = "handled",
  keyboardVerticalOffset = 0,
  preset = "fixed",
  scrollViewProps,
}: KhalaScreenProps) => {
  const content =
    preset === "fixed" ? (
      <View className={`flex-1 ${contentClassName}`.trim()}>{children}</View>
    ) : (
      <ScrollView
        {...scrollViewProps}
        className={`flex-1 ${scrollViewProps?.className ?? ""}`.trim()}
        contentContainerClassName={`${
          preset === "keyboardAware" ? "grow" : ""
        } ${contentClassName} ${scrollViewProps?.contentContainerClassName ?? ""}`.trim()}
        keyboardShouldPersistTaps={keyboardShouldPersistTaps}
      >
        {children}
      </ScrollView>
    )

  return (
    <SafeAreaView className={`flex-1 bg-bg ${className}`.trim()} edges={edges}>
      {preset === "keyboardAware" ? (
        <KeyboardAvoidingView
          behavior={Platform.OS === "ios" ? "padding" : "height"}
          className="flex-1"
          keyboardVerticalOffset={keyboardVerticalOffset}
        >
          {content}
        </KeyboardAvoidingView>
      ) : (
        content
      )}
    </SafeAreaView>
  )
}
