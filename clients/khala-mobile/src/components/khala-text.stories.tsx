import type { Meta, StoryObj } from "@storybook/react-native"
import { View } from "react-native"

import { KhalaText, type KhalaTextVariant } from "./khala-text"

const variants: ReadonlyArray<KhalaTextVariant> = [
  "heading",
  "body",
  "muted",
  "caption",
  "label",
  "mono",
  "success",
  "warning",
  "danger",
  "faint",
]

const meta = {
  title: "Khala/Primitives/Text",
  component: KhalaText,
  args: {
    text: "Khala Code",
    variant: "body",
  },
  argTypes: {
    variant: {
      control: "select",
      options: variants,
    },
  },
} satisfies Meta<typeof KhalaText>

export default meta

type Story = StoryObj<typeof meta>

export const Basic: Story = {}

export const Variants: Story = {
  render: () => (
    <View className="gap-3">
      {variants.map((variant) => (
        <View className="gap-1" key={variant}>
          <KhalaText text={variant} variant="faint" />
          <KhalaText text="The local coding runtime is ready." variant={variant} />
        </View>
      ))}
    </View>
  ),
}
