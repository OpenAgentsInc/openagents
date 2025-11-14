import type { Meta, StoryObj } from '@storybook/react-vite';
import { Separator } from "@openagentsinc/ui";

const meta = {
  title: 'UI/Separator',
  component: Separator,
  argTypes: {
    orientation: { control: 'select', options: ['horizontal', 'vertical'] },
    width: { control: 'number' },
    height: { control: 'number' },
  },
  args: {
    orientation: 'horizontal',
    width: 420,
    height: 120,
  },
} satisfies Meta<typeof Separator>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ orientation, width, height }) => (
    <div
      style={{
        width: Number(width),
        height: Number(height),
        display: 'flex',
        flexDirection: orientation === 'horizontal' ? 'column' : 'row',
        alignItems: 'center',
        justifyContent: 'center',
        gap: 12,
      }}
    >
      <span>Left</span>
      <Separator orientation={orientation as any} />
      <span>Right</span>
    </div>
  ),
};

