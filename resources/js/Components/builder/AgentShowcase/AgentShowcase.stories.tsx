import type { Meta, StoryObj } from '@storybook/react';
import { AgentShowcase } from '.';

const meta = {
  title: 'OpenAgents/AgentShowcase/Page',
  component: AgentShowcase,
  parameters: { layout: 'fullscreen' },
  // tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof AgentShowcase>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
