import type { Meta, StoryObj } from "@openagentsinc/storybook"
import * as Fx from "@typed/fx/Fx"
import { button, span } from "@typed/ui/hyperscript"
import type { Renderable } from "@typed/template/Renderable"

type ButtonArgs = {
  label: string
  variant: "primary" | "secondary" | "danger"
  size: "sm" | "default" | "lg"
  disabled: boolean
  onClick: () => void
}

// Simple Typed Button component
const TypedButton = (args: ButtonArgs): Fx.Fx<any, never, any> => {
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
  
  const className = [
    baseClasses,
    variantClasses[args.variant],
    sizeClasses[args.size]
  ].join(" ")

  return button(
    {
      className,
      disabled: args.disabled,
      onclick: args.onClick,
      style: "font-family: 'Berkeley Mono', monospace; background-color: " + 
             (args.variant === "primary" ? "#ffffff" : args.variant === "danger" ? "#dc2626" : "transparent") + 
             "; color: " + (args.variant === "primary" ? "#000000" : "#ffffff") + 
             "; border: 1px solid " + (args.variant === "danger" ? "#dc2626" : "#ffffff")
    },
    args.label
  )
}

const meta = {
  title: "Components/Button",
  component: TypedButton,
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
} satisfies Meta<ButtonArgs>

export default meta

type Story = StoryObj<ButtonArgs, typeof meta>

export const Primary: Story = {
  render: TypedButton,
  args: {
    label: "Primary Button",
    variant: "primary",
    size: "default",
    disabled: false
  }
}

export const Secondary: Story = {
  render: TypedButton,
  args: {
    label: "Secondary Button", 
    variant: "secondary",
    size: "default",
    disabled: false
  }
}

export const Danger: Story = {
  render: TypedButton,
  args: {
    label: "Danger Button",
    variant: "danger",
    size: "default",
    disabled: false
  }
}

export const Small: Story = {
  render: TypedButton,
  args: {
    label: "Small Button",
    variant: "primary",
    size: "sm",
    disabled: false
  }
}

export const Large: Story = {
  render: TypedButton,
  args: {
    label: "Large Button",
    variant: "primary", 
    size: "lg",
    disabled: false
  }
}

export const Disabled: Story = {
  render: TypedButton,
  args: {
    label: "Disabled Button",
    variant: "primary",
    size: "default",
    disabled: true
  }
}