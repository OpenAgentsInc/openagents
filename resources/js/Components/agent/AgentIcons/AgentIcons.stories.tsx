import type { Meta, StoryObj } from '@storybook/react';
import { AgentIcons } from '.';

const meta = {
  title: 'OpenAgents/AgentIcons',
  component: AgentIcons,
  parameters: { layout: 'fullscreen' },
  argTypes: {},
  decorators: [
    (Story) => (
      <>
        <div className="max-w-2xl mx-auto h-screen pt-16">
          <Story />
        </div>
      </>
    ),
  ],
} satisfies Meta<typeof AgentIcons>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
