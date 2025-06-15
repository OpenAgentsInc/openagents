import type { Meta, StoryObj } from "@typed/storybook"
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter, Button } from "@openagentsinc/ui"
import type { Fx } from "@typed/fx/Fx"
import { RenderEvent } from "@typed/template/RenderEvent"

type CardArgs = {
  title: string
  description: string
  content: string
  hasFooter: boolean
  size?: "default" | "sm" | "lg"
}

// Card component using @openagentsinc/ui
const createCard = (args: CardArgs): Fx<RenderEvent, never, any> => {
  const cardContent = [
    CardHeader({
      children: [
        CardTitle({ children: args.title }),
        CardDescription({ children: args.description })
      ]
    }),
    CardContent({ children: args.content })
  ]

  if (args.hasFooter) {
    cardContent.push(
      CardFooter({
        children: Button({
          variant: "default",
          size: "default",
          children: "Action"
        })
      })
    )
  }

  return Card({
    size: args.size,
    children: cardContent
  })
}

const meta = {
  title: "Components/Card",
  component: createCard,
  parameters: {
    layout: "centered",
    backgrounds: {
      default: "dark"
    }
  },
  tags: ["autodocs"],
  argTypes: {
    hasFooter: {
      control: { type: "boolean" }
    },
    size: {
      control: { type: "select" },
      options: ["default", "sm", "lg"]
    }
  }
} satisfies Meta<CardArgs>

export default meta

type Story = StoryObj<CardArgs, typeof meta>

export const Default: Story = {
  render: createCard,
  args: {
    title: "OpenAgents Card",
    description: "A beautiful card component with Berkeley Mono font",
    content:
      "This is the main content of the card. It demonstrates the OpenAgents aesthetic with clean design and proper spacing.",
    hasFooter: false,
    size: "default"
  }
}

export const WithFooter: Story = {
  render: createCard,
  args: {
    title: "Card with Footer",
    description: "This card includes a footer section",
    content: "Cards can optionally include footer sections for actions or additional information.",
    hasFooter: true,
    size: "default"
  }
}

export const LongContent: Story = {
  render: createCard,
  args: {
    title: "Long Content Example",
    description: "Demonstrating how cards handle longer text content",
    content:
      "This card contains much longer content to demonstrate how the component handles text wrapping and larger amounts of information. The Berkeley Mono font maintains readability even with longer passages of text, and the consistent spacing creates a pleasant reading experience.",
    hasFooter: true,
    size: "default"
  }
}

export const Small: Story = {
  render: createCard,
  args: {
    title: "Small Card",
    description: "Compact size",
    content: "This is a small card with reduced padding.",
    hasFooter: false,
    size: "sm"
  }
}

export const Large: Story = {
  render: createCard,
  args: {
    title: "Large Card",
    description: "Spacious layout",
    content: "This is a large card with increased padding for more prominent display.",
    hasFooter: true,
    size: "lg"
  }
}