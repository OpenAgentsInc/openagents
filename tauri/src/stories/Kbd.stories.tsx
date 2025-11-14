import type { Meta, StoryObj } from '@storybook/react-vite';
import { Kbd, KbdGroup } from "@openagentsinc/ui";

const meta = {
  title: 'UI/Kbd',
  component: Kbd,
  argTypes: {
    keys: { control: 'text' },
  },
  args: {
    keys: '⌘ + K',
  },
} satisfies Meta<typeof Kbd>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Single: Story = {
  render: ({ keys }) => <Kbd>{keys as string}</Kbd>,
};

export const Group: Story = {
  render: () => (
    <KbdGroup>
      <Kbd>⌘</Kbd>
      <span>+</span>
      <Kbd>K</Kbd>
    </KbdGroup>
  ),
};

