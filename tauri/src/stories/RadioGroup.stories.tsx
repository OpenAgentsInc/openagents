import type { Meta, StoryObj } from '@storybook/react-vite';
import { useEffect, useState } from 'react';
import { RadioGroup, RadioGroupItem } from "@openagentsinc/ui";
import { Label } from "@openagentsinc/ui";

const meta = {
  title: 'UI/RadioGroup',
  component: RadioGroup,
  argTypes: {
    value: { control: 'select', options: ['apple', 'banana', 'orange'] },
    disabled: { control: 'boolean' },
  },
  args: {
    value: 'apple',
    disabled: false,
  },
} satisfies Meta<typeof RadioGroup>;

export default meta;
type Story = StoryObj<typeof meta>;

const OPTIONS = [
  { value: 'apple', label: 'Apple' },
  { value: 'banana', label: 'Banana' },
  { value: 'orange', label: 'Orange' },
];

export const Basic: Story = {
  render: ({ value, disabled }) => {
    const [current, setCurrent] = useState<string>(String(value));
    useEffect(() => setCurrent(String(value)), [value]);
    return (
      <RadioGroup value={current} onValueChange={setCurrent} className="max-w-xs">
        {OPTIONS.map((o) => (
          <div key={o.value} className="flex items-center gap-2">
            <RadioGroupItem id={`rg-${o.value}`} value={o.value} disabled={!!disabled} />
            <Label htmlFor={`rg-${o.value}`}>{o.label}</Label>
          </div>
        ))}
      </RadioGroup>
    );
  },
};

