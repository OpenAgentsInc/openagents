import type { Meta, StoryObj } from '@storybook/react';
import { Node } from '../../components/Node';
import { demoAgent, demoBrain, demoStep } from './Node.demodata';

const meta = {
  title: 'AgentGraph/Node',
  component: Node,
  tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof Node>;

export default meta;

type Story = StoryObj<typeof meta>;

export const AgentNode: Story = {
  args: {
    agent: demoAgent,
  }
}

export const BrainNode: Story = {
  args: {
    brain: demoBrain
  }
}

export const StepNode: Story = {
  args: {
    step: demoStep
  }
}
