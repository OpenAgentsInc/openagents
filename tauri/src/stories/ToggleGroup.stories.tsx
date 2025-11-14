import type { Meta, StoryObj } from '@storybook/react-vite';
import { useEffect, useState } from 'react';
import { ToggleGroup, ToggleGroupItem } from "@openagentsinc/ui";
import { Bold, Italic, Underline } from 'lucide-react';

const meta = {
  title: 'UI/ToggleGroup',
  component: ToggleGroup,
  argTypes: {
    type: { control: 'select', options: ['single', 'multiple'] },
    spacing: { control: 'number' },
    variant: { control: 'select', options: ['default', 'outline'] },
    size: { control: 'select', options: ['sm', 'default', 'lg'] },
  },
  args: {
    type: 'single',
    spacing: 0,
    variant: 'outline',
    size: 'default',
  },
} satisfies Meta<typeof ToggleGroup>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Single: Story = {
  render: ({ type, spacing, variant, size }) => {
    const [value, setValue] = useState<string>('bold');
    useEffect(() => setValue('bold'), [type]);
    return (
      <ToggleGroup type={type as any} spacing={Number(spacing)} variant={variant as any} size={size as any} value={value} onValueChange={setValue}>
        <ToggleGroupItem value="bold" aria-label="Toggle bold"><Bold /></ToggleGroupItem>
        <ToggleGroupItem value="italic" aria-label="Toggle italic"><Italic /></ToggleGroupItem>
        <ToggleGroupItem value="underline" aria-label="Toggle underline"><Underline /></ToggleGroupItem>
      </ToggleGroup>
    );
  },
};

export const Multiple: Story = {
  args: { type: 'multiple' },
  render: ({ type, spacing, variant, size }) => {
    const [value, setValue] = useState<string[]>(['bold']);
    useEffect(() => setValue(['bold']), [type]);
    return (
      <ToggleGroup type={type as any} spacing={Number(spacing)} variant={variant as any} size={size as any} value={value} onValueChange={setValue}>
        <ToggleGroupItem value="bold" aria-label="Toggle bold"><Bold /></ToggleGroupItem>
        <ToggleGroupItem value="italic" aria-label="Toggle italic"><Italic /></ToggleGroupItem>
        <ToggleGroupItem value="underline" aria-label="Toggle underline"><Underline /></ToggleGroupItem>
      </ToggleGroup>
    );
  },
};

