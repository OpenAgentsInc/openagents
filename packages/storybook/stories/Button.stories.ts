import type { Meta, StoryObj } from "@typed/storybook"
import { Button } from "@openagentsinc/ui"
import type { Fx } from "@typed/fx/Fx"
import { RenderEvent } from "@typed/template/RenderEvent"

type ButtonArgs = {
  children: string
  variant?: "default" | "destructive" | "outline" | "secondary" | "ghost" | "link" | undefined
  size?: "default" | "sm" | "lg" | "icon" | undefined
  disabled?: boolean | undefined
  onClick?: (() => void) | undefined
}

// Button component using @openagentsinc/ui
const createButton = (args: ButtonArgs): Fx<RenderEvent, never, any> => {
  return Button({
    ...(args.variant && { variant: args.variant }),
    ...(args.size && { size: args.size }),
    ...(args.disabled !== undefined && { disabled: args.disabled }),
    ...(args.onClick && { onClick: args.onClick }),
    children: args.children
  })
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
      options: ["default", "destructive", "outline", "secondary", "ghost", "link"]
    },
    size: {
      control: { type: "select" },
      options: ["default", "sm", "lg", "icon"]
    },
    onClick: { action: "clicked" }
  }
} satisfies Meta<ButtonArgs>

export default meta

type Story = StoryObj<ButtonArgs, typeof meta>

export const Default: Story = {
  render: createButton,
  args: {
    children: "Default Button",
    variant: "default",
    size: "default",
    disabled: false
  }
}

export const Destructive: Story = {
  render: createButton,
  args: {
    children: "Destructive Button",
    variant: "destructive",
    size: "default",
    disabled: false
  }
}

export const Outline: Story = {
  render: createButton,
  args: {
    children: "Outline Button",
    variant: "outline",
    size: "default",
    disabled: false
  }
}

export const Secondary: Story = {
  render: createButton,
  args: {
    children: "Secondary Button",
    variant: "secondary",
    size: "default",
    disabled: false
  }
}

export const Ghost: Story = {
  render: createButton,
  args: {
    children: "Ghost Button",
    variant: "ghost",
    size: "default",
    disabled: false
  }
}

export const Link: Story = {
  render: createButton,
  args: {
    children: "Link Button",
    variant: "link",
    size: "default",
    disabled: false
  }
}

export const Small: Story = {
  render: createButton,
  args: {
    children: "Small Button",
    variant: "default",
    size: "sm",
    disabled: false
  }
}

export const Large: Story = {
  render: createButton,
  args: {
    children: "Large Button",
    variant: "default",
    size: "lg",
    disabled: false
  }
}

export const Disabled: Story = {
  render: createButton,
  args: {
    children: "Disabled Button",
    variant: "default",
    size: "default",
    disabled: true
  }
}