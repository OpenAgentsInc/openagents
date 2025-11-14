import type { Meta, StoryObj } from '@storybook/react-vite';
import { Label } from "@openagentsinc/ui";
import { Input } from "@openagentsinc/ui";
import { Checkbox } from "@openagentsinc/ui";

const meta = {
  title: 'UI/Label',
  component: Label,
  argTypes: {
    text: { control: 'text' },
  },
  args: {
    text: 'Email',
  },
} satisfies Meta<typeof Label>;

export default meta;
type Story = StoryObj<typeof meta>;

export const WithInput: Story = {
  render: ({ text }) => (
    <div style={{ display: 'grid', gap: 8, width: 320 }}>
      <Label htmlFor="email">{text as string}</Label>
      <Input id="email" type="email" placeholder="you@example.com" />
    </div>
  ),
};

export const WithCheckbox: Story = {
  render: () => (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
      <Checkbox id="agree" />
      <Label htmlFor="agree">I agree to the terms</Label>
    </div>
  ),
};

