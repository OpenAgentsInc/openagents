import type { Meta, StoryObj } from "@typed/storybook"
import { WebTUIButton } from "@openagentsinc/ui"
import { Fx } from "@typed/fx"
import { RenderEvent } from "@typed/dom/RenderEvent"

type WebTUIButtonArgs = {
  children: string
  variant?: "foreground0" | "foreground1" | "foreground2" | "background0" | "background1" | "background2" | "background3"
  size?: "small" | "default" | "large"
  box?: "square" | "round" | "double"
  shear?: "top" | "bottom" | "both"
  disabled?: boolean
  onClick?: any
}

const meta: Meta<WebTUIButtonArgs> = {
  title: "WebTUI/Button",
  argTypes: {
    children: {
      control: "text"
    },
    variant: {
      control: "select",
      options: ["foreground0", "foreground1", "foreground2", "background0", "background1", "background2", "background3"]
    },
    size: {
      control: "select",
      options: ["small", "default", "large"]
    },
    box: {
      control: "select",
      options: [undefined, "square", "round", "double"]
    },
    shear: {
      control: "select",
      options: [undefined, "top", "bottom", "both"]
    },
    disabled: {
      control: "boolean"
    }
  }
}

export default meta
type Story = StoryObj<WebTUIButtonArgs>

const createButton = (args: WebTUIButtonArgs): Fx<RenderEvent, never, any> => {
  return WebTUIButton({
    ...(args.variant && { variant: args.variant }),
    ...(args.size && { size: args.size }),
    ...(args.box && { box: args.box }),
    ...(args.shear && { shear: args.shear }),
    ...(args.disabled !== undefined && { disabled: args.disabled }),
    ...(args.onClick && { onClick: args.onClick }),
    children: args.children
  })
}

export const Default: Story = {
  args: {
    children: "Click Me"
  },
  render: createButton
}

export const Variants: Story = {
  render: () => {
    return [
      WebTUIButton({ children: "Foreground0", variant: "foreground0" }),
      " ",
      WebTUIButton({ children: "Foreground1", variant: "foreground1" }),
      " ",
      WebTUIButton({ children: "Foreground2", variant: "foreground2" }),
      " ",
      WebTUIButton({ children: "Background0", variant: "background0" }),
      " ",
      WebTUIButton({ children: "Background1", variant: "background1" }),
      " ",
      WebTUIButton({ children: "Background2", variant: "background2" }),
      " ",
      WebTUIButton({ children: "Background3", variant: "background3" })
    ]
  }
}

export const Sizes: Story = {
  render: () => {
    return [
      WebTUIButton({ children: "Small", size: "small" }),
      " ",
      WebTUIButton({ children: "Default", size: "default" }),
      " ",
      WebTUIButton({ children: "Large", size: "large" })
    ]
  }
}

export const WithBoxBorders: Story = {
  render: () => {
    return [
      WebTUIButton({ children: "Square", box: "square" }),
      " ",
      WebTUIButton({ children: "Round", box: "round" }),
      " ",
      WebTUIButton({ children: "Double", box: "double" })
    ]
  }
}

export const WithShear: Story = {
  render: () => {
    return [
      WebTUIButton({ children: "Top Shear", box: "square", shear: "top" }),
      " ",
      WebTUIButton({ children: "Bottom Shear", box: "square", shear: "bottom" }),
      " ",
      WebTUIButton({ children: "Both Shear", box: "square", shear: "both" })
    ]
  }
}

export const Disabled: Story = {
  args: {
    children: "Disabled Button",
    disabled: true
  },
  render: createButton
}

export const Interactive: Story = {
  args: {
    children: "Interactive Button",
    variant: "background1",
    box: "round",
    onClick: () => alert("Button clicked!")
  },
  render: createButton
}