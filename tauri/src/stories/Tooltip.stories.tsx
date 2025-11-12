import type { Meta, StoryObj } from '@storybook/react-vite';
import { Tooltip, TooltipTrigger, TooltipContent } from '@/components/ui/tooltip';
import { Button } from '@/components/ui/button';

const meta = {
  title: 'UI/Tooltip',
  component: Tooltip,
  argTypes: {
    content: { control: 'text' },
    side: { control: 'select', options: ['top', 'right', 'bottom', 'left'] },
    label: { control: 'text' },
  },
  args: {
    content: 'Tooltip content',
    side: 'top',
    label: 'Hover me',
  },
} satisfies Meta<typeof Tooltip>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ content, side, label }) => (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="outline">{label as string}</Button>
      </TooltipTrigger>
      <TooltipContent side={side as any} sideOffset={8}>
        {content as string}
      </TooltipContent>
    </Tooltip>
  ),
};
