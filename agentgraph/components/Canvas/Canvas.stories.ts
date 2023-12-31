import type { Meta, StoryObj } from '@storybook/react';
import { Canvas } from './Canvas';

const meta = {
  title: 'AgentGraph/Canvas',
  component: Canvas,
  parameters: { layout: 'fullscreen' },
  tags: ['autodocs'],
  argTypes: {},
} satisfies Meta<typeof Canvas>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Empty: Story = {}
