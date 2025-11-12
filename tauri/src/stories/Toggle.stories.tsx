import type { Meta, StoryObj } from '@storybook/react-vite';
import { useEffect, useState } from 'react';
import { Toggle } from '@/components/ui/toggle';
import { Bold } from 'lucide-react';

const meta = {
  title: 'UI/Toggle',
  component: Toggle,
  argTypes: {
    variant: { control: 'select', options: ['default', 'outline'] },
    size: { control: 'select', options: ['sm', 'default', 'lg'] },
    pressed: { control: 'boolean' },
    label: { control: 'text' },
  },
  args: {
    variant: 'default',
    size: 'default',
    pressed: false,
    label: 'Bold',
  },
} satisfies Meta<typeof Toggle>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ variant, size, pressed, label }) => {
    const [on, setOn] = useState<boolean>(!!pressed);
    useEffect(() => setOn(!!pressed), [pressed]);
    return (
      <Toggle variant={variant as any} size={size as any} pressed={on} onPressedChange={setOn} aria-label={String(label)}>
        <Bold /> {label as string}
      </Toggle>
    );
  },
};

export const Outline: Story = { args: { variant: 'outline' } };
export const Small: Story = { args: { size: 'sm' } };
export const Large: Story = { args: { size: 'lg' } };

