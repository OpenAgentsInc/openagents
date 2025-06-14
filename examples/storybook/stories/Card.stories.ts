import type { Meta, StoryObj } from "@openagentsinc/storybook"
import type * as Fx from "@typed/fx/Fx"
import { button, div } from "@typed/ui/hyperscript"

type CardArgs = {
  title: string
  description: string
  content: string
  hasFooter: boolean
}

// Simple Typed Card component
const TypedCard = (args: CardArgs): Fx.Fx<any, never, any> => {
  const cardStyle =
    "font-family: 'Berkeley Mono', monospace; background-color: #000000; color: #ffffff; border: 1px solid #ffffff; padding: 24px; display: flex; flex-direction: column; gap: 24px; box-shadow: 0 1px 3px 0 rgba(255, 255, 255, 0.1)"

  const headerStyle = "display: grid; grid-template-rows: auto auto; align-items: start; gap: 6px"

  const titleStyle = "font-family: 'Berkeley Mono', monospace; font-weight: 600; line-height: 1; font-size: 16px"

  const descriptionStyle = "font-family: 'Berkeley Mono', monospace; color: #a1a1aa; font-size: 14px"

  const contentStyle = "font-family: 'Berkeley Mono', monospace"

  const footerStyle = "font-family: 'Berkeley Mono', monospace; display: flex; align-items: center; margin-top: 24px"

  const buttonStyle =
    "font-family: 'Berkeley Mono', monospace; background-color: #ffffff; color: #000000; border: 1px solid #ffffff; padding: 8px 16px; cursor: pointer; font-size: 14px"

  return div(
    { style: cardStyle },
    div(
      { style: headerStyle },
      div({ style: titleStyle }, args.title),
      div({ style: descriptionStyle }, args.description)
    ),
    div({ style: contentStyle }, args.content),
    ...(args.hasFooter ?
      [
        div(
          { style: footerStyle },
          button({ style: buttonStyle }, "Action")
        )
      ] :
      [])
  )
}

const meta = {
  title: "Components/Card",
  component: TypedCard,
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
  render: TypedCard,
  args: {
    title: "OpenAgents Card",
    description: "A beautiful card component with Berkeley Mono font",
    content:
      "This is the main content of the card. It demonstrates the OpenAgents aesthetic with black background, white text, and clean borders.",
    hasFooter: false
  }
}

export const WithFooter: Story = {
  render: TypedCard,
  args: {
    title: "Card with Footer",
    description: "This card includes a footer section",
    content: "Cards can optionally include footer sections for actions or additional information.",
    hasFooter: true
  }
}

export const LongContent: Story = {
  render: TypedCard,
  args: {
    title: "Long Content Example",
    description: "Demonstrating how cards handle longer text content",
    content:
      "This card contains much longer content to demonstrate how the component handles text wrapping and larger amounts of information. The Berkeley Mono font maintains readability even with longer passages of text, and the consistent spacing creates a pleasant reading experience.",
    hasFooter: true
  }
}

export const MinimalCard: Story = {
  render: TypedCard,
  args: {
    title: "Minimal",
    description: "Clean and simple",
    content: "Less is more.",
    hasFooter: false
  }
}
