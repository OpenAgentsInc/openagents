import type { Meta, StoryObj } from '@storybook/react';
import { AgentBuilder } from '.';

const meta = {
  title: 'OpenAgents/AgentBuilder/Page',
  component: AgentBuilder,
  // tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof AgentBuilder>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
