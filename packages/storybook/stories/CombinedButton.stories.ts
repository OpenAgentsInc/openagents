import type { Meta, StoryObj } from "@storybook/html"
import { button as typedButton } from "@typed/ui/hyperscript"
import * as Runtime from "effect/Runtime"
import * as Effect from "effect/Effect"

type ButtonArgs = {
  label: string
  variant: "primary" | "secondary" | "danger"
  size: "sm" | "default" | "lg"
  disabled: boolean
  onClick: () => void
}

// Create combined component that shows both HTML and Typed versions
const createCombinedButton = (args: ButtonArgs) => {
  const container = document.createElement("div")
  container.style.display = "flex"
  container.style.flexDirection = "column"
  container.style.gap = "20px"
  container.style.alignItems = "center"
  container.style.padding = "20px"
  
  // HTML Version Label
  const htmlLabel = document.createElement("div")
  htmlLabel.style.fontFamily = "'Berkeley Mono', monospace"
  htmlLabel.style.color = "#666"
  htmlLabel.style.fontSize = "12px"
  htmlLabel.style.textTransform = "uppercase"
  htmlLabel.style.letterSpacing = "0.1em"
  htmlLabel.textContent = "HTML Version"
  
  // HTML Button
  const htmlButton = document.createElement("button")
  htmlButton.textContent = args.label
  htmlButton.disabled = args.disabled
  htmlButton.onclick = args.onClick
  
  // Apply styles to HTML button
  htmlButton.style.fontFamily = "'Berkeley Mono', monospace"
  htmlButton.style.backgroundColor = args.variant === "primary" ? "#ffffff" : args.variant === "danger" ? "#dc2626" : "transparent"
  htmlButton.style.color = args.variant === "primary" ? "#000000" : "#ffffff"
  htmlButton.style.border = `1px solid ${args.variant === "danger" ? "#dc2626" : "#ffffff"}`
  htmlButton.style.padding = args.size === "sm" ? "0.5rem 0.75rem" : args.size === "lg" ? "0.625rem 2rem" : "0.5rem 1rem"
  htmlButton.style.height = args.size === "sm" ? "2rem" : args.size === "lg" ? "2.5rem" : "2.25rem"
  htmlButton.style.cursor = args.disabled ? "not-allowed" : "pointer"
  htmlButton.style.opacity = args.disabled ? "0.5" : "1"
  htmlButton.style.fontSize = args.size === "sm" ? "12px" : args.size === "lg" ? "16px" : "14px"
  htmlButton.style.transition = "all 0.2s"
  
  // Typed Version Label
  const typedLabel = document.createElement("div")
  typedLabel.style.fontFamily = "'Berkeley Mono', monospace"
  typedLabel.style.color = "#666"
  typedLabel.style.fontSize = "12px"
  typedLabel.style.textTransform = "uppercase"
  typedLabel.style.letterSpacing = "0.1em"
  typedLabel.textContent = "Typed Version"
  
  // Typed Button Container
  const typedContainer = document.createElement("div")
  
  // Create and render Typed button
  const baseStyle = `
    cursor: ${args.disabled ? "not-allowed" : "pointer"};
    display: inline-flex;
    align-items: center;
    justify-content: center;
    white-space: nowrap;
    font-weight: 500;
    font-family: 'Berkeley Mono', monospace;
    transition: all 0.2s;
    border-radius: 0;
    outline: none;
    opacity: ${args.disabled ? "0.5" : "1"};
  `
  
  const variantStyles = {
    primary: `background-color: #ffffff; color: #000000; border: 1px solid #ffffff;`,
    secondary: `background-color: transparent; color: #ffffff; border: 1px solid #ffffff;`,
    danger: `background-color: #dc2626; color: #ffffff; border: 1px solid #dc2626;`
  }
  
  const sizeStyles = {
    sm: "height: 32px; padding: 0 12px; font-size: 12px;",
    default: "height: 36px; padding: 0 16px; font-size: 14px;",
    lg: "height: 40px; padding: 0 32px; font-size: 16px;"
  }
  
  const style = `${baseStyle} ${variantStyles[args.variant]} ${sizeStyles[args.size]}`
  
  // Create Typed button Fx
  const typedButtonFx = typedButton(
    {
      style,
      disabled: args.disabled,
      onclick: args.onClick
    },
    args.label
  )
  
  // Render Typed button synchronously for this demo
  const runtime = Runtime.defaultRuntime
  const renderEffect = Effect.gen(function* () {
    // In a real app, we'd use renderToLayer, but for this demo we'll just append directly
    const element = yield* typedButtonFx
    typedContainer.appendChild(element as any)
  })
  
  Runtime.runPromise(runtime)(renderEffect).catch(console.error)
  
  // Assemble the container
  container.appendChild(htmlLabel)
  container.appendChild(htmlButton)
  container.appendChild(typedLabel)
  container.appendChild(typedContainer)
  
  return container
}

const meta: Meta<ButtonArgs> = {
  title: "Combined/Button",
  render: createCombinedButton,
  parameters: {
    layout: "centered",
    backgrounds: {
      default: "dark"
    }
  },
  tags: ["autodocs"],
  argTypes: {
    variant: {
      control: { type: "select" },
      options: ["primary", "secondary", "danger"]
    },
    size: {
      control: { type: "select" },
      options: ["sm", "default", "lg"]
    },
    onClick: { action: "clicked" }
  }
}

export default meta

type Story = StoryObj<ButtonArgs>

export const Primary: Story = {
  args: {
    label: "Primary Button",
    variant: "primary",
    size: "default",
    disabled: false
  }
}

export const Secondary: Story = {
  args: {
    label: "Secondary Button",
    variant: "secondary",
    size: "default",
    disabled: false
  }
}

export const Danger: Story = {
  args: {
    label: "Danger Button",
    variant: "danger",
    size: "default",
    disabled: false
  }
}

export const Sizes: Story = {
  render: (args) => {
    const container = document.createElement("div")
    container.style.display = "flex"
    container.style.gap = "40px"
    container.style.alignItems = "center"
    
    const sizes: Array<"sm" | "default" | "lg"> = ["sm", "default", "lg"]
    
    sizes.forEach(size => {
      const button = createCombinedButton({ ...args, size, label: `${size} Button` })
      container.appendChild(button)
    })
    
    return container
  },
  args: {
    label: "Button",
    variant: "primary",
    size: "default",
    disabled: false
  }
}