import { Component, type ErrorInfo, type ReactNode } from "react"
import { View, type TextStyle, type ViewStyle } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import {
  buildKhalaCrashReport,
  reportKhalaMobileCrash,
  type KhalaCrashReporter,
} from "../diagnostics/crash-reporting"
import { tx } from "../i18n/copy"
import { Button, Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"

type KhalaErrorBoundaryProps = Readonly<{
  crashReporter?: KhalaCrashReporter
  children: ReactNode
}>

type KhalaErrorBoundaryState = Readonly<{
  error: Error | null
}>

const KhalaErrorFallback = ({ onReset }: { onReset: () => void }) => {
  const { themed } = useAppTheme()
  return (
    <SafeAreaView style={themed($safeArea)}>
      <View style={themed($content)}>
        <Text preset="heading" style={$center} text={tx("app.title")} />
        <Text style={[$center, themed($dim)]} text={tx("errorBoundary.body")} />
        <Text size="xs" style={[$center, themed($faint)]} text={tx("errorBoundary.help")} />
        <Button preset="reversed" style={$stretch} onPress={onReset} text={tx("errorBoundary.retry")} />
      </View>
    </SafeAreaView>
  )
}

const $safeArea: ThemedStyle<ViewStyle> = ({ colors }) => ({ flex: 1, backgroundColor: colors.background })
const $content: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flex: 1,
  alignItems: "center",
  justifyContent: "center",
  paddingHorizontal: spacing.lg,
  gap: spacing.md,
})
const $center: TextStyle = { textAlign: "center" }
const $stretch: ViewStyle = { alignSelf: "stretch", marginTop: 16 }
const $dim: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })
const $faint: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })

export class KhalaErrorBoundary extends Component<
  KhalaErrorBoundaryProps,
  KhalaErrorBoundaryState
> {
  state: KhalaErrorBoundaryState = { error: null }

  static getDerivedStateFromError(error: Error): KhalaErrorBoundaryState {
    return { error }
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    void reportKhalaMobileCrash(
      buildKhalaCrashReport(error, errorInfo),
      this.props.crashReporter,
    )
  }

  reset = () => {
    this.setState({ error: null })
  }

  render() {
    if (this.state.error !== null) return <KhalaErrorFallback onReset={this.reset} />
    return this.props.children
  }
}
