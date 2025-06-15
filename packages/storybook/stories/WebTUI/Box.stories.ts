import type { Meta, StoryObj } from "@typed/storybook"
import { Box, SquareBox, RoundBox, DoubleBox } from "@openagentsinc/ui"
import { Fx } from "@typed/fx"
import { RenderEvent } from "@typed/dom/RenderEvent"

type BoxArgs = {
  children: string
  box?: "square" | "round" | "double"
  shear?: "top" | "bottom" | "both"
}

const meta: Meta<BoxArgs> = {
  title: "WebTUI/Box",
  argTypes: {
    children: {
      control: "text"
    },
    box: {
      control: "select",
      options: ["square", "round", "double"]
    },
    shear: {
      control: "select",
      options: [undefined, "top", "bottom", "both"]
    }
  }
}

export default meta
type Story = StoryObj<BoxArgs>

const createBox = (args: BoxArgs): Fx<RenderEvent, never, any> => {
  return Box({
    ...(args.box && { box: args.box }),
    ...(args.shear && { shear: args.shear }),
    children: args.children
  })
}

export const Default: Story = {
  args: {
    children: "Box Content",
    box: "square"
  },
  render: createBox
}

export const BoxTypes: Story = {
  render: () => {
    return [
      SquareBox({ children: "Square Box" }),
      " ",
      RoundBox({ children: "Round Box" }),
      " ",
      DoubleBox({ children: "Double Box" })
    ]
  }
}

export const WithShear: Story = {
  render: () => {
    return [
      SquareBox({ children: "Top Shear", shear: "top" }),
      " ",
      SquareBox({ children: "Bottom Shear", shear: "bottom" }),
      " ",
      SquareBox({ children: "Both Shear", shear: "both" })
    ]
  }
}

export const NestedBoxes: Story = {
  render: () => {
    return DoubleBox({
      children: [
        "Outer Double Box",
        RoundBox({
          children: [
            "Inner Round Box",
            SquareBox({ children: "Nested Square Box" })
          ]
        })
      ]
    })
  }
}

export const Interactive: Story = {
  args: {
    children: "Interactive Box",
    box: "round",
    shear: "top"
  },
  render: createBox
}