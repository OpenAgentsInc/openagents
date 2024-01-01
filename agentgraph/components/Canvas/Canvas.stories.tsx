import type { Meta, StoryObj } from '@storybook/react';
import { Canvas } from './Canvas';
import { Node } from '../Node'
import { demoStep } from '../Node/Node.demodata';

const meta = {
  title: 'AgentGraph/Canvas',
  component: Canvas,
  parameters: { layout: 'fullscreen' },
  tags: ['autodocs'],
  argTypes: {},
  decorators: [
    (Story) => (
      <div style={{ height: '100vh' }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof Canvas>;

export default meta;

type Story = StoryObj<typeof meta>;

export const WithNode: Story = {
  args: {
    children: (
      <Node
        position={{ x: 100, y: 100 }}
        step={demoStep}
      />
    )
  }
}

export const Empty: Story = {}
