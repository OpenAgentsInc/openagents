import type { Meta, StoryObj } from '@storybook/react';
import { ChatBar } from '.';

const meta = {
  title: 'OpenAgents/AgentBuilder/ChatBar',
  component: ChatBar,
  // tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof ChatBar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
