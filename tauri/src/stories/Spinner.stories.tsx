import type { Meta, StoryObj } from '@storybook/react-vite';
import { Spinner } from "@openagentsinc/ui";

const meta = {
  title: 'UI/Spinner',
  component: Spinner,
  argTypes: {
    size: { control: 'select', options: ['sm', 'md', 'lg'] },
  },
  args: {
    size: 'md',
  },
} satisfies Meta<typeof Spinner>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ size }) => {
    const sz = size === 'sm' ? 'size-4' : size === 'lg' ? 'size-8' : 'size-6';
    return <Spinner className={sz} />;
  },
};

