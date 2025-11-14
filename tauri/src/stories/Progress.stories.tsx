import type { Meta, StoryObj } from '@storybook/react-vite';
import { Progress } from "@openagentsinc/ui";

const meta = {
  title: 'UI/Progress',
  component: Progress,
  argTypes: {
    value: { control: { type: 'range', min: 0, max: 100, step: 1 } },
    width: { control: 'number' },
  },
  args: {
    value: 33,
    width: 320,
  },
} satisfies Meta<typeof Progress>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ value, width }) => (
    <div style={{ width: Number(width) }}>
      <Progress value={Number(value)} />
    </div>
  ),
};

