import type { Meta, StoryObj } from '@storybook/react';
import { AgentSidebar } from '.';
import { demoAgent } from '../../../../../agentgraph/components/Node/Node.demodata';
import { Navbar } from '@/Components/nav/Navbar';

const meta = {
  title: 'OpenAgents/AgentSidebar',
  component: AgentSidebar,
  parameters: { layout: 'fullscreen' },
  argTypes: {},
  decorators: [
    (Story) => (
      <>
        <Navbar />
        <div className="w-full mx-auto h-screen">
          <Story />
        </div>
      </>
    ),
  ],
} satisfies Meta<typeof AgentSidebar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: {
    agent: demoAgent,
    owner: 'DemoMan'
  }
}
