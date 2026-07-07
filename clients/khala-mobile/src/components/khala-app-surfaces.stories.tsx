import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { KhalaAuthProvider } from "../auth/khala-auth-context"
import { SignInScreen } from "./sign-in-screen"

const meta = {
  title: "Khala/App Surfaces",
  component: View,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof View>

export default meta

type Story = StoryObj<typeof meta>

export const SignIn: Story = {
  render: () => (
    <KhalaAuthProvider>
      <SignInScreen />
    </KhalaAuthProvider>
  ),
}
