import type { Meta, StoryObj } from '@storybook/react';
import { Canvas } from './Canvas';
import { Node } from '../Node'
import { demoAgent, demoSteps } from '../Node/Node.demodata';

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
        step={demoSteps[0]}
      />
    )
  }
}

export const With6Nodes: Story = {
  args: {
    children: (
      <>
        <Node
          position={{ x: 260, y: 50 }}
          agent={demoAgent}
        />
        <Node
          position={{ x: 800, y: 110 }}
          brain={demoAgent.brains[0]}
        />
        <Node
          position={{ x: 100, y: 200 }}
          step={demoSteps[0]}
        />
        <Node
          position={{ x: 450, y: 200 }}
          step={demoSteps[1]}
        />
        <Node
          position={{ x: 100, y: 450 }}
          step={demoSteps[2]}
        />
        <Node
          position={{ x: 450, y: 450 }}
          step={demoSteps[3]}
        />
      </>
    )
  }
}

export const Empty: Story = {}
