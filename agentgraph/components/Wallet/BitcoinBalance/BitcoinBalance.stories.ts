import type { Meta, StoryObj } from '@storybook/react';
import { BitcoinBalance } from '.';

const meta = {
  title: 'AgentGraph/Wallet/BitcoinBalance',
  component: BitcoinBalance,
  tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof BitcoinBalance>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: {
    sats: 100000
  }
}
