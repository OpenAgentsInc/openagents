import type { Preview } from "@storybook/react"
import { View } from "react-native"
import { GestureHandlerRootView } from "react-native-gesture-handler"
import { SafeAreaProvider } from "react-native-safe-area-context"

import "../global.css"
import { KhalaThemeProvider } from "../src/theme/khala-theme-provider"

const preview: Preview = {
  decorators: [
    (Story) => (
      <GestureHandlerRootView className="flex-1 bg-bg">
        <SafeAreaProvider>
          <KhalaThemeProvider>
            <View className="flex-1 bg-bg p-5">
              <Story />
            </View>
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
