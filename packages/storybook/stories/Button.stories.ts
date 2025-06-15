import type { Meta, StoryObj } from "@typed/storybook"
import { button } from "@typed/ui/hyperscript"
import type { Fx } from "@typed/fx/Fx"
import { RenderEvent } from "@typed/template/RenderEvent"

type ButtonArgs = {
  label: string
  variant: "primary" | "secondary" | "danger"
  size: "sm" | "default" | "lg"
  disabled: boolean
  onClick?: () => void
}

// HTML Button component using @typed/ui
const createButton = (args: ButtonArgs): Fx<RenderEvent, never, any> => {
  return button(
    {
      className: `btn btn-${args.variant} btn-${args.size} ${args.disabled ? 'btn-disabled' : ''}`,
      disabled: args.disabled,
      onclick: args.onClick
    },
    args.label
  )
}

const meta = {
  title: "Components/Button",
  component: createButton,
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
  render: createButton,
  args: {
    label: "Primary Button",
    variant: "primary",
    size: "default",
    disabled: false
  }
}

export const Secondary: Story = {
  render: createButton,
  args: {
    label: "Secondary Button",
    variant: "secondary",
    size: "default",
    disabled: false
  }
}

export const Danger: Story = {
  render: createButton,
  args: {
    label: "Danger Button",
    variant: "danger",
    size: "default",
    disabled: false
  }
}

export const Small: Story = {
  render: createButton,
  args: {
    label: "Small Button",
    variant: "primary",
    size: "sm",
    disabled: false
  }
}

export const Large: Story = {
  render: createButton,
  args: {
    label: "Large Button",
    variant: "primary",
    size: "lg",
    disabled: false
  }
}

export const Disabled: Story = {
  render: createButton,
  args: {
    label: "Disabled Button",
    variant: "primary",
    size: "default",
    disabled: true
  }
}