import type { Preview } from "@storybook/react"
import { useFonts } from "expo-font"
import type { ReactNode } from "react"
import { View } from "react-native"
import { GestureHandlerRootView } from "react-native-gesture-handler"
import { SafeAreaProvider } from "react-native-safe-area-context"

import "../global.css"
import { khalaMobileFontsToLoad } from "../src/theme/typography"
import { KhalaThemeProvider } from "../src/theme/khala-theme-provider"

const FontGate = ({ children }: { children: ReactNode }) => {
  const [fontsLoaded, fontError] = useFonts(khalaMobileFontsToLoad)

  if (!fontsLoaded && fontError === null) return <View className="flex-1 bg-bg" />

  return <>{children}</>
}

const preview: Preview = {
  decorators: [
    (Story) => (
      <GestureHandlerRootView className="flex-1 bg-bg">
        <SafeAreaProvider>
          <KhalaThemeProvider>
            <FontGate>
              <View className="flex-1 bg-bg">
                <Story />
              </View>
            </FontGate>
          </KhalaThemeProvider>
        </SafeAreaProvider>
      </GestureHandlerRootView>
    ),
  ],
  parameters: {
    backgrounds: {
      default: "Khala dark",
      values: [
        { name: "Khala dark", value: "#02060d" },
        { name: "Raised surface", value: "#07111f" },
      ],
    },
    controls: {
      matchers: {
        color: /(background|color)$/i,
        date: /Date$/i,
      },
    },
  },
}

export default preview
