import type { Meta, StoryObj } from '@storybook/react';
import { Node } from '../../components/Node';
import { demoStep } from './Node.demodata';

const meta = {
  title: 'AgentGraph/Node',
  component: Node,
  parameters: { layout: 'centered' },
  tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof Node>;

export default meta;

type Story = StoryObj<typeof meta>;

export const StepNode: Story = {
  args: {
    step: demoStep
  }
}
