import type { Meta, StoryObj } from '@storybook/react-vite';
import { MarkdownText } from '@/components/assistant-ui/markdown-text';

const meta = {
  title: 'Assistant UI/MarkdownText',
  component: MarkdownText,
} satisfies Meta<typeof MarkdownText>;

export default meta;
type Story = StoryObj<typeof meta>;

// Note: MarkdownText is typically rendered within a Message/Thread context.
// This story demonstrates the component surface within a simple container.
export const Preview: Story = {
  render: () => (
    <div className="dark max-w-2xl bg-background text-foreground p-4 rounded-md border">
      <MarkdownText />
    </div>
  ),
};

