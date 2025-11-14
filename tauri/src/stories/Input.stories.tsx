import type { ComponentProps } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";

import { Input } from "@openagentsinc/ui";

type InputStoryProps = ComponentProps<typeof Input> & {
  width?: number;
  invalid?: boolean;
};

const InputStoryDemo = ({
  width = 320,
  invalid,
  style,
  ...rest
}: InputStoryProps) => {
  const { ["aria-invalid"]: ariaInvalidProp, ...inputProps } = rest;
  return (
    <Input
      {...inputProps}
      aria-invalid={invalid ?? ariaInvalidProp}
      style={{ width, ...(style ?? {}) }}
    />
  );
};

const meta = {
  title: "UI/Input",
  component: InputStoryDemo,
  argTypes: {
    type: {
      control: "select",
      options: ["text", "password", "email", "file", "number", "search", "url"],
    },
    placeholder: { control: "text" },
    width: { control: "number" },
    invalid: { control: "boolean" },
    disabled: { control: "boolean" },
  },
  args: {
    type: "text",
    placeholder: "Enter text",
    width: 320,
    invalid: false,
    disabled: false,
  },
} satisfies Meta<typeof InputStoryDemo>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {};

export const Password: Story = {
  args: { type: "password", placeholder: "Enter password" },
};

export const File: Story = {
  args: { type: 'file', placeholder: undefined },
};
