import type { Meta, StoryObj } from '@storybook/react';
import { ChatPane } from '.';

const meta = {
  title: 'OpenAgents/AgentBuilder/ChatPane',
  component: ChatPane,
  // tags: ['autodocs'],
  argTypes: {},
  parameters: { layout: 'fullscreen' },
  decorators: [
    (Story) => (
      <div style={{ height: '100vh' }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof ChatPane>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
