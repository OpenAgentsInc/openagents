import type { Meta, StoryObj } from '@storybook/react-vite';
import { Popover, PopoverTrigger, PopoverContent } from '@/components/ui/popover';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';

const meta = {
  title: 'UI/Popover',
  component: Popover,
  argTypes: {
    side: { control: 'select', options: ['top', 'right', 'bottom', 'left'] },
    align: { control: 'select', options: ['start', 'center', 'end'] },
    sideOffset: { control: 'number' },
    label: { control: 'text' },
    placeholder: { control: 'text' },
    width: { control: 'number' },
  },
  args: {
    side: 'bottom',
    align: 'center',
    sideOffset: 8,
    label: 'Open popover',
    placeholder: 'Type something...',
    width: 280,
  },
} satisfies Meta<typeof Popover>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ side, align, sideOffset, label, placeholder, width }) => (
    <Popover>
      <PopoverTrigger asChild>
        <Button variant="outline">{label as string}</Button>
      </PopoverTrigger>
      <PopoverContent side={side as any} align={align as any} sideOffset={Number(sideOffset)}>
        <div style={{ display: 'grid', gap: 8, width: Number(width) }}>
          <div className="text-sm font-medium">Quick input</div>
          <Input placeholder={placeholder as string} />
        </div>
      </PopoverContent>
    </Popover>
  ),
};

