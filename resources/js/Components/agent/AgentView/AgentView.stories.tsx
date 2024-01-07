import type { Meta, StoryObj } from '@storybook/react';
import { AgentView } from '.';
import { Navbar } from '@/Components/nav/Navbar';

const meta = {
  title: 'OpenAgents/AgentView',
  component: AgentView,
  parameters: { layout: 'fullscreen' },
  argTypes: {},
  decorators: [
    (Story) => (
      <>
        <Navbar />
        <div className="h-screen pt-16">
          <Story />
        </div>
      </>
    ),
  ],
} satisfies Meta<typeof AgentView>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
