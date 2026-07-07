import type { Meta, StoryObj } from "@storybook/react-native"
import { Image, Pressable, StyleSheet, Text as RNText, View } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { tx } from "../i18n/copy"
import { KhalaText } from "./khala-text"

const meta = {
  title: "Khala/App Surfaces",
  component: View,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

export const SignIn: Story = {
  render: () => (
    <View className="flex-1 bg-bg">
      <Image resizeMode="cover" source={require("../../assets/images/home-hero.jpg")} style={StyleSheet.absoluteFill} />
      <View pointerEvents="none" style={styles.scrim} />
      <SafeAreaView className="flex-1" edges={["top", "bottom"]}>
        <View className="flex-1 justify-between px-6 pb-6">
          <View />
          <KhalaText style={styles.title} variant="heading">
            {tx("app.title")}
          </KhalaText>
          <Pressable accessibilityRole="button" className="min-h-[54px] items-center justify-center rounded-xl bg-accent px-5">
            <RNText style={styles.loginButtonText}>{tx("signIn.github.primary")}</RNText>
          </Pressable>
        </View>
      </SafeAreaView>
    </View>
  ),
}

const styles = StyleSheet.create({
  loginButtonText: {
    color: "#02060d",
    fontSize: 17,
    fontWeight: "700",
    textAlign: "center",
  },
  scrim: {
    ...StyleSheet.absoluteFill,
    backgroundColor: "rgba(2, 10, 22, 0.9)",
  },
  title: {
    color: "white",
    fontFamily: "Protomolecule",
    fontSize: 40,
    letterSpacing: 2,
    lineHeight: 48,
    textAlign: "center",
    textShadowColor: "#4fd0ff",
    textShadowOffset: { height: 0, width: 0 },
    textShadowRadius: 12,
  },
})
