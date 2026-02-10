import { FC } from "react"

import { Screen } from "@/components/Screen"
import { DemoTabScreenProps } from "@/navigators/navigationTypes"
import { $styles } from "@/theme/styles"

/**
 * Feed tab - placeholder screen, first tab in the app.
 */
export const FeedScreen: FC<DemoTabScreenProps<"Feed">> = function FeedScreen() {
  return (
    <Screen preset="fixed" contentContainerStyle={$styles.container} safeAreaEdges={["top"]}>
      {/* Feed content goes here */}
    </Screen>
  )
}
