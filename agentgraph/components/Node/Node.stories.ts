import type { Meta, StoryObj } from '@storybook/react';
import { Node } from '../../components/Node';
import { Step } from '@/types/agents';

const demoStep: Step = {
  agent_id: 1,
  category: 'validation',
  created_at: '2021-08-31T15:00:00.000Z',
  description: "A demo step",
  entry_type: 'input',
  error_message: "Could not validate input",
  id: 1,
  name: "Validate Input",
  order: 1,
  success_action: "next_node",
  task_id: 1,
  updated_at: '2021-08-31T15:00:00.000Z',
}

const meta = {
  title: 'AgentGraph/Node',
  component: Node,
  parameters: {
    // Optional parameter to center the component in the Canvas. More info: https://storybook.js.org/docs/configure/story-layout
    layout: 'centered',
  },
  // This component will have an automatically generated Autodocs entry: https://storybook.js.org/docs/writing-docs/autodocs
  tags: ['autodocs'],
  // More on argTypes: https://storybook.js.org/docs/api/argtypes
  argTypes: {
    // backgroundColor: { control: 'color' },
  },
} satisfies Meta<typeof Node>;

export default meta;

type Story = StoryObj<typeof meta>;

// More on writing stories with args: https://storybook.js.org/docs/writing-stories/args
export const Primary: Story = {
  // args: {
  // primary: true,
  // label: 'Button',
  // },
};

export const StepNode: Story = {
  args: {
    step: demoStep
  }
}
