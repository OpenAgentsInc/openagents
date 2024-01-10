import type { Meta, StoryObj } from '@storybook/react';
import { demoAgent } from '../../../../../agentgraph/components/Node/Node.demodata';
import { AgentKnowledge } from '.';

const meta = {
  title: 'OpenAgents/AgentKnowledge',
  component: AgentKnowledge,
  parameters: { layout: 'fullscreen' },
  argTypes: {},
  decorators: [
    (Story) => (
      <>
        <div className="w-[375px] max-w-2xl mx-auto h-screen pt-16">
          <Story />
        </div>
      </>
    ),
  ],
} satisfies Meta<typeof AgentKnowledge>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: {
    agent: demoAgent,
    isOwner: true
  }
}
