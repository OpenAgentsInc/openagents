import type { Meta, StoryObj } from "@storybook/html"

type CardArgs = {
  title: string
  description: string
  content: string
  hasFooter: boolean
}

// Create HTML Card component
const createCard = (args: CardArgs) => {
  const card = document.createElement("div")
  
  // Card container styles
  card.style.fontFamily = "'Berkeley Mono', monospace"
  card.style.backgroundColor = "#000000"
  card.style.color = "#ffffff"
  card.style.border = "1px solid #ffffff"
  card.style.padding = "24px"
  card.style.display = "flex"
  card.style.flexDirection = "column"
  card.style.gap = "24px"
  card.style.boxShadow = "0 1px 3px 0 rgba(255, 255, 255, 0.1)"
  card.style.width = "400px" // Fixed width for consistent display
  
  // Header section
  const header = document.createElement("div")
  header.style.display = "grid"
  header.style.gridTemplateRows = "auto auto"
  header.style.alignItems = "start"
  header.style.gap = "6px"
  
  // Title
  const title = document.createElement("div")
  title.style.fontFamily = "'Berkeley Mono', monospace"
  title.style.fontWeight = "600"
  title.style.lineHeight = "1"
  title.style.fontSize = "16px"
  title.textContent = args.title
  
  // Description
  const description = document.createElement("div")
  description.style.fontFamily = "'Berkeley Mono', monospace"
  description.style.color = "#a1a1aa"
  description.style.fontSize = "14px"
  description.textContent = args.description
  
  header.appendChild(title)
  header.appendChild(description)
  
  // Content
  const content = document.createElement("div")
  content.style.fontFamily = "'Berkeley Mono', monospace"
  content.textContent = args.content
  
  // Build card
  card.appendChild(header)
  card.appendChild(content)
  
  // Footer (optional)
  if (args.hasFooter) {
    const footer = document.createElement("div")
    footer.style.fontFamily = "'Berkeley Mono', monospace"
    footer.style.display = "flex"
    footer.style.alignItems = "center"
    footer.style.marginTop = "24px"
    
    const button = document.createElement("button")
    button.style.fontFamily = "'Berkeley Mono', monospace"
    button.style.backgroundColor = "#ffffff"
    button.style.color = "#000000"
    button.style.border = "1px solid #ffffff"
    button.style.padding = "8px 16px"
    button.style.cursor = "pointer"
    button.style.fontSize = "14px"
    button.textContent = "Action"
    
    // Add hover effect
    button.onmouseenter = () => {
      button.style.backgroundColor = "#e5e5e5"
    }
    button.onmouseleave = () => {
      button.style.backgroundColor = "#ffffff"
    }
    
    footer.appendChild(button)
    card.appendChild(footer)
  }
  
  return card
}

const meta: Meta<CardArgs> = {
  title: "Components/Card",
  render: createCard,
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
}

export default meta

type Story = StoryObj<CardArgs>

export const Default: Story = {
  args: {
    title: "OpenAgents Card",
    description: "A beautiful card component with Berkeley Mono font",
    content:
      "This is the main content of the card. It demonstrates the OpenAgents aesthetic with black background, white text, and clean borders.",
    hasFooter: false
  }
}

export const WithFooter: Story = {
  args: {
    title: "Card with Footer",
    description: "This card includes a footer section",
    content: "Cards can optionally include footer sections for actions or additional information.",
    hasFooter: true
  }
}

export const LongContent: Story = {
  args: {
    title: "Long Content Example",
    description: "Demonstrating how cards handle longer text content",
    content:
      "This card contains much longer content to demonstrate how the component handles text wrapping and larger amounts of information. The Berkeley Mono font maintains readability even with longer passages of text, and the consistent spacing creates a pleasant reading experience.",
    hasFooter: true
  }
}

export const MinimalCard: Story = {
  args: {
    title: "Minimal",
    description: "Clean and simple",
    content: "Less is more.",
    hasFooter: false
  }
}
