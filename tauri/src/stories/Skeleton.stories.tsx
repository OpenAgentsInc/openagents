import type { Meta, StoryObj } from '@storybook/react-vite';
import { Skeleton } from '@/components/ui/skeleton';

const meta = {
  title: 'UI/Skeleton',
  component: Skeleton,
  argTypes: {
    width: { control: 'number' },
  },
  args: {
    width: 320,
  },
} satisfies Meta<typeof Skeleton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Blocks: Story = {
  render: ({ width }) => (
    <div style={{ display: 'grid', gap: 12, width: Number(width) }}>
      <Skeleton className="h-6 w-2/3" />
      <Skeleton className="h-4 w-1/2" />
      <Skeleton className="h-4 w-full" />
      <div style={{ display: 'flex', gap: 12 }}>
        <Skeleton className="size-10 rounded-full" />
        <div style={{ flex: 1 }}>
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="mt-2 h-4 w-1/2" />
        </div>
      </div>
    </div>
  ),
};
