import type { Meta, StoryObj } from "@storybook/react-native"

import { MobileTestingLabScreen } from "./mobile-testing-lab-screen"

const meta = {
  title: "Khala/Generated Screens/MobileTestingLab",
  component: MobileTestingLabScreen,
  parameters: { layout: "fullscreen" },
} satisfies Meta<typeof MobileTestingLabScreen>

export default meta

type Story = StoryObj<typeof meta>

export const Loading: Story = { args: { state: "loading" } }
export const Error: Story = { args: { state: "error" } }
export const Empty: Story = { args: { state: "empty" } }
export const Populated: Story = { args: { state: "populated" } }
