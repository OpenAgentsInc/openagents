import type { Meta, StoryObj } from "@typed/storybook"
import { button, div } from "@typed/ui/hyperscript"
import type { Fx } from "@typed/fx/Fx"
import { RenderEvent } from "@typed/template/RenderEvent"

type CardArgs = {
  title: string
  description: string
  content: string
  hasFooter: boolean
}

// Card component using @typed/ui
const createCard = (args: CardArgs): Fx<RenderEvent, never, any> => {
  // Build the card structure
  const header = div(
    { className: "card-header" },
    div({ className: "card-title" }, args.title),
    div({ className: "card-description" }, args.description)
  )

  const content = div({ className: "card-content" }, args.content)

  // Conditionally add footer
  const children = args.hasFooter
    ? [
        header,
        content,
        div(
          { className: "card-footer" },
          button(
            {
              className: "btn btn-primary btn-default",
              onclick: () => console.log("Card action clicked")
            },
            "Action"
          )
        )
      ]
    : [header, content]

  return div({ className: "card" }, ...children)
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
      "This is the main content of the card. It demonstrates the OpenAgents aesthetic with black background, white text, and clean borders.",
    hasFooter: false
  }
}

export const WithFooter: Story = {
  render: createCard,
  args: {
    title: "Card with Footer",
    description: "This card includes a footer section",
    content: "Cards can optionally include footer sections for actions or additional information.",
    hasFooter: true
  }
}

export const LongContent: Story = {
  render: createCard,
  args: {
    title: "Long Content Example",
    description: "Demonstrating how cards handle longer text content",
    content:
      "This card contains much longer content to demonstrate how the component handles text wrapping and larger amounts of information. The Berkeley Mono font maintains readability even with longer passages of text, and the consistent spacing creates a pleasant reading experience.",
    hasFooter: true
  }
}

export const MinimalCard: Story = {
  render: createCard,
  args: {
    title: "Minimal",
    description: "Clean and simple",
    content: "Less is more.",
    hasFooter: false
  }
}