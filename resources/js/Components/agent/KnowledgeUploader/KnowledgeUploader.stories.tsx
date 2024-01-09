import type { Meta, StoryObj } from '@storybook/react';
import { KnowledgeUploader } from '.';
import { demoAgent } from '../../../../../agentgraph/components/Node/Node.demodata';

const meta = {
  title: 'OpenAgents/KnowledgeUploader',
  component: KnowledgeUploader,
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
} satisfies Meta<typeof KnowledgeUploader>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {
  args: {
    agent: demoAgent,
    owner: 'DemoMan'
  }
}
