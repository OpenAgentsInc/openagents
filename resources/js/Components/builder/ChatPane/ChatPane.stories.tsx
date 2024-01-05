import type { Meta, StoryObj } from '@storybook/react';
import { ChatPane } from '.';

const meta = {
  title: 'OpenAgents/AgentBuilder/ChatPane',
  component: ChatPane,
  // tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof ChatPane>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
