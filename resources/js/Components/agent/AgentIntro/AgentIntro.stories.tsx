import type { Meta, StoryObj } from '@storybook/react';
import { AgentIntro } from '.';
import { demoAgent } from '../../../../../agentgraph/components/Node/Node.demodata';

const meta = {
  title: 'OpenAgents/AgentIntro',
  component: AgentIntro,
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
} satisfies Meta<typeof AgentIntro>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: {
    agent: demoAgent,
    conversation: {
      id: 1
    }
  }
}
