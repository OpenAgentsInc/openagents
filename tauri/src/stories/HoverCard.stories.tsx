import type { Meta, StoryObj } from '@storybook/react-vite';
import { HoverCard, HoverCardTrigger, HoverCardContent } from '@/components/ui/hover-card';
import { Button } from '@/components/ui/button';

const meta = {
  title: 'UI/HoverCard',
  component: HoverCard,
  argTypes: {
    side: { control: 'select', options: ['top', 'right', 'bottom', 'left'] },
    align: { control: 'select', options: ['start', 'center', 'end'] },
    sideOffset: { control: 'number' },
    label: { control: 'text' },
    content: { control: 'text' },
  },
  args: {
    side: 'top',
    align: 'center',
    sideOffset: 8,
    label: 'Hover me',
    content:
      'Hover cards show rich previews or supplemental information when hovering an element.',
  },
} satisfies Meta<typeof HoverCard>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ side, align, sideOffset, label, content }) => (
    <HoverCard>
      <HoverCardTrigger asChild>
        <Button variant="outline">{label as string}</Button>
      </HoverCardTrigger>
      <HoverCardContent side={side as any} align={align as any} sideOffset={Number(sideOffset)}>
        {content as string}
      </HoverCardContent>
    </HoverCard>
  ),
};

