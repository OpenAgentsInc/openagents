import type { Meta, StoryObj } from '@storybook/react-vite';
import { ToolFallback } from '@/components/assistant-ui/tool-fallback';

const meta = {
  title: 'Assistant UI/ToolFallback',
  component: ToolFallback,
  argTypes: {
    toolName: { control: 'text' },
    argsText: { control: 'text' },
    result: { control: 'object' },
  },
  args: {
    toolName: 'calculator',
    argsText: '{"a": 2, "b": 3}',
    result: { sum: 5 },
  },
} satisfies Meta<typeof ToolFallback>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

export const WithStringResult: Story = {
  args: {
    result: 'Operation completed successfully',
  },
};

