import type { Meta, StoryObj } from "@openagentsinc/storybook"
import { button } from "@typed/ui"
import type { Fx } from "@typed/fx/Fx"

type ButtonArgs = {
  label: string
  variant: "primary" | "secondary" | "danger"
  size: "sm" | "default" | "lg"
  disabled: boolean
  onClick: () => void
}

// Typed Button component using @typed/ui
const TypedButton = (args: ButtonArgs): Fx<HTMLButtonElement, never, any> => {
  // Base styles
  const baseStyle = `
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    white-space: nowrap;
    font-size: 14px;
    font-weight: 500;
    font-family: 'Berkeley Mono', monospace;
    transition: all 0.2s;
    border-radius: 0;
    outline: none;
  `

  // Variant styles
  const variantStyles = {
    primary: `
      background-color: #ffffff;
      color: #000000;
      border: 1px solid #ffffff;
    `,
    secondary: `
      background-color: transparent;
      color: #ffffff;
      border: 1px solid #ffffff;
    `,
    danger: `
      background-color: #dc2626;
      color: #ffffff;
      border: 1px solid #dc2626;
    `
  }

  // Size styles
  const sizeStyles = {
    sm: "height: 32px; padding: 0 12px; font-size: 12px;",
    default: "height: 36px; padding: 0 16px; font-size: 14px;",
    lg: "height: 40px; padding: 0 32px; font-size: 16px;"
  }

  // Disabled styles
  const disabledStyle = args.disabled ? "opacity: 0.5; cursor: not-allowed;" : ""

  // Combine all styles
  const style = `${baseStyle} ${variantStyles[args.variant]} ${sizeStyles[args.size]} ${disabledStyle}`

  return button(
    {
      style,
      disabled: args.disabled,
      onclick: args.onClick
    },
    args.label
  )
}

const meta: Meta<ButtonArgs> = {
  title: "Typed/Button",
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
}

export default meta

type Story = StoryObj<ButtonArgs>

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