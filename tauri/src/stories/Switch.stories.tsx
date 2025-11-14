import type { Meta, StoryObj } from '@storybook/react-vite';
import { useEffect, useState } from 'react';
import { Switch } from "@openagentsinc/ui";
import { Label } from "@openagentsinc/ui";

const meta = {
  title: 'UI/Switch',
  component: Switch,
  argTypes: {
    checked: { control: 'boolean' },
    disabled: { control: 'boolean' },
    label: { control: 'text' },
  },
  args: {
    checked: false,
    disabled: false,
    label: 'Dark mode',
  },
} satisfies Meta<typeof Switch>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ checked, disabled, label }) => {
    const [value, setValue] = useState<boolean>(!!checked);
    useEffect(() => setValue(!!checked), [checked]);
    return (
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <Switch id="switch-basic" checked={value} disabled={!!disabled} onCheckedChange={(v) => setValue(!!v)} />
        <Label htmlFor="switch-basic">{label as string}</Label>
      </div>
    );
  },
};

export const Disabled: Story = {
  args: { disabled: true },
};

