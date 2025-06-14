import type { Meta, StoryObj } from "@storybook/html"

type ButtonArgs = {
  label: string
  variant: "primary" | "secondary" | "danger"
  size: "sm" | "default" | "lg"
  disabled: boolean
  onClick: () => void
}

const createButton = (args: ButtonArgs) => {
  const button = document.createElement("button")
  
  const baseClasses = "cursor-pointer inline-flex items-center justify-center whitespace-nowrap text-sm font-medium font-mono transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 rounded-none"
  
  const variantClasses = {
    primary: "bg-white text-black hover:bg-gray-200 border border-white",
    secondary: "bg-transparent text-white border border-white hover:bg-white hover:text-black",
    danger: "bg-red-600 text-white hover:bg-red-700 border border-red-600"
  }
  
  const sizeClasses = {
    sm: "h-8 px-3 text-xs",
    default: "h-9 px-4 py-2", 
    lg: "h-10 px-8"
  }
  
  button.className = [
    baseClasses,
    variantClasses[args.variant],
    sizeClasses[args.size]
  ].join(" ")

  button.textContent = args.label
  button.disabled = args.disabled
  button.onclick = args.onClick
  
  // Apply inline styles for OpenAgents aesthetic
  button.style.fontFamily = "'Berkeley Mono', monospace"
  button.style.backgroundColor = args.variant === "primary" ? "#ffffff" : args.variant === "danger" ? "#dc2626" : "transparent"
  button.style.color = args.variant === "primary" ? "#000000" : "#ffffff"
  button.style.border = `1px solid ${args.variant === "danger" ? "#dc2626" : "#ffffff"}`
  button.style.padding = sizeClasses[args.size].includes("px-3") ? "0.5rem 0.75rem" : sizeClasses[args.size].includes("px-8") ? "0.625rem 2rem" : "0.5rem 1rem"
  button.style.height = sizeClasses[args.size].includes("h-8") ? "2rem" : sizeClasses[args.size].includes("h-10") ? "2.5rem" : "2.25rem"
  
  return button
}

const meta: Meta<ButtonArgs> = {
  title: "Components/Button",
  render: createButton,
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

export const Small: Story = {
  args: {
    label: "Small Button",
    variant: "primary",
    size: "sm",
    disabled: false
  }
}

export const Large: Story = {
  args: {
    label: "Large Button",
    variant: "primary",
    size: "lg",
    disabled: false
  }
}

export const Disabled: Story = {
  args: {
    label: "Disabled Button",
    variant: "primary",
    size: "default",
    disabled: true
  }
}