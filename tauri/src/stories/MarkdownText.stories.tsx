import type { Meta, StoryObj } from '@storybook/react-vite';
// MarkdownText from assistant-ui requires a message context (MessagePrimitive.Parts)
// which is only available inside a Thread. For Storybook, render a static preview
// using the same wrapper container and a minimal markdown sample to avoid context errors.
import { MarkdownText } from '@/components/assistant-ui/markdown-text';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';

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
    <div className="dark max-w-2xl bg-background text-foreground p-4 rounded-md border space-y-3">
      <div className="text-sm text-muted-foreground">
        MarkdownText is designed to be used inside a Thread message context. This
        preview renders a static markdown sample in Storybook to avoid context errors.
      </div>
      <ReactMarkdown remarkPlugins={[remarkGfm]}>
        {`# Markdown Preview\n\n- Supports **bold**, _italic_, and code: \`const x = 1\`\n- Tables:\n\n| Col A | Col B |\n| --- | --- |\n| 1 | 2 |\n\n> Blockquote sample\n\n\`\`\`ts\nfunction greet(name: string){\n  console.log('Hello ' + name)\n}\n\`\`\``}
      </ReactMarkdown>
    </div>
  ),
  parameters: {
    controls: { disable: true },
  },
};
