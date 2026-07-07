import type { ReactNode } from "react"
import { ScrollView, StyleSheet, View } from "react-native"

import { Text } from "./components/Text"
import { useAppTheme } from "./theme/context"

export const IgniteStoryScreen = ({ children }: { children: ReactNode }) => {
  const { theme } = useAppTheme()

  return (
    <ScrollView
      contentContainerStyle={[styles.screenContent, { backgroundColor: theme.colors.background }]}
      style={[styles.screen, { backgroundColor: theme.colors.background }]}
    >
      {children}
    </ScrollView>
  )
}

export const IgniteStory = ({ children }: { children: ReactNode }) => (
  <View style={styles.story}>{children}</View>
)

export const IgniteUseCase = ({
  children,
  noPad = false,
  text,
  usage,
}: {
  children: ReactNode
  noPad?: boolean
  text: string
  usage?: string
}) => {
  const { theme } = useAppTheme()

  return (
    <View style={[styles.useCase, { borderColor: theme.colors.separator }]}>
      <View style={styles.useCaseHeader}>
        <Text preset="subheading" text={text} />
        {usage === undefined ? null : <Text preset="formHelper" text={usage} />}
      </View>
      <View style={[styles.useCaseBody, noPad && styles.noPad]}>{children}</View>
    </View>
  )
}

const styles = StyleSheet.create({
  noPad: { padding: 0 },
  screen: { flex: 1 },
  screenContent: { gap: 18, padding: 18 },
  story: { gap: 18 },
  useCase: { borderTopWidth: 1, gap: 10, paddingTop: 14 },
  useCaseBody: { gap: 10, padding: 4 },
  useCaseHeader: { gap: 3 },
})
