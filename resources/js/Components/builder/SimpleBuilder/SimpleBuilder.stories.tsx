import type { Meta, StoryObj } from '@storybook/react';
import { SimpleBuilder } from '.';

const meta = {
  title: 'OpenAgents/AgentBuilder/SimpleBuilder',
  component: SimpleBuilder,
  parameters: { layout: 'fullscreen' },
  argTypes: {},
  decorators: [
    (Story) => (
      <div style={{ height: '100vh' }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof SimpleBuilder>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Primary: Story = {}
