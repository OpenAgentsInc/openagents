import type { Meta, StoryObj } from '@storybook/react';
import { Canvas } from './Canvas';
import { Node } from '../Node'
import { demoStep } from '../Node/Node.stories';

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

// Put a node in the canvas
export const WithNode: Story = {
  args: {
    children: (
      <Node
        step={demoStep}
      />
    )
  }
}

export const Empty: Story = {}
