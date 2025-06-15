import type { Meta, StoryObj } from "@typed/storybook"
import { Badge } from "@openagentsinc/ui"
import { Fx } from "@typed/fx"
import { RenderEvent } from "@typed/dom/RenderEvent"

type BadgeArgs = {
  children: string
  variant?: "foreground0" | "foreground1" | "foreground2" | "background0" | "background1" | "background2" | "background3"
  capStart?: "round" | "triangle" | "slant-top" | "slant-bottom" | "ribbon"
  capEnd?: "round" | "triangle" | "slant-top" | "slant-bottom" | "ribbon"
}

const meta: Meta<BadgeArgs> = {
  title: "WebTUI/Badge",
  argTypes: {
    children: {
      control: "text"
    },
    variant: {
      control: "select",
      options: ["foreground0", "foreground1", "foreground2", "background0", "background1", "background2", "background3"]
    },
    capStart: {
      control: "select",
      options: [undefined, "round", "triangle", "slant-top", "slant-bottom", "ribbon"]
    },
    capEnd: {
      control: "select", 
      options: [undefined, "round", "triangle", "slant-top", "slant-bottom", "ribbon"]
    }
  }
}

export default meta
type Story = StoryObj<BadgeArgs>

const createBadge = (args: BadgeArgs): Fx<RenderEvent, never, any> => {
  return Badge({
    ...(args.variant && { variant: args.variant }),
    ...(args.capStart && { capStart: args.capStart }),
    ...(args.capEnd && { capEnd: args.capEnd }),
    children: args.children
  })
}

export const Default: Story = {
  args: {
    children: "Badge"
  },
  render: createBadge
}

export const Variants: Story = {
  render: () => {
    return [
      Badge({ children: "Default", variant: "foreground0" }),
      " ",
      Badge({ children: "Foreground1", variant: "foreground1" }),
      " ",
      Badge({ children: "Foreground2", variant: "foreground2" }),
      " ",
      Badge({ children: "Background0", variant: "background0" }),
      " ",
      Badge({ children: "Background1", variant: "background1" }),
      " ",
      Badge({ children: "Background2", variant: "background2" }),
      " ",
      Badge({ children: "Background3", variant: "background3" })
    ]
  }
}

export const WithCaps: Story = {
  render: () => {
    return [
      Badge({ children: "Round", capStart: "round", capEnd: "round" }),
      " ",
      Badge({ children: "Triangle", capStart: "triangle", capEnd: "triangle" }),
      " ", 
      Badge({ children: "Slant Top", capStart: "slant-top", capEnd: "slant-top" }),
      " ",
      Badge({ children: "Slant Bottom", capStart: "slant-bottom", capEnd: "slant-bottom" }),
      " ",
      Badge({ children: "Ribbon", capStart: "ribbon", capEnd: "ribbon" })
    ]
  }
}

export const Interactive: Story = {
  args: {
    children: "Interactive Badge",
    variant: "background1",
    capStart: "round",
    capEnd: "round"
  },
  render: createBadge
}