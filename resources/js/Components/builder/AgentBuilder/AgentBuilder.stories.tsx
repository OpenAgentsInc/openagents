import type { Meta, StoryObj } from '@storybook/react';
import { AgentBuilder } from '.';

const meta = {
  title: 'OpenAgents/AgentBuilder/Full',
  component: AgentBuilder,
  parameters: { layout: 'fullscreen' },
  // tags: ['autodocs'],
  argTypes: {},
  decorators: [
    (Story) => (
      <div style={{ height: '100vh' }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof AgentBuilder>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
