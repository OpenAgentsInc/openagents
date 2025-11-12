import type { Meta, StoryObj } from '@storybook/react-vite';
import { useEffect, useRef } from 'react';
import { MyRuntimeProvider } from '@/runtime/MyRuntimeProvider';
import { Thread } from '@/components/assistant-ui/thread';
import { useAssistantApi } from '@openagentsinc/assistant-ui-runtime';

type Role = 'user' | 'assistant';

function SeedMessage({ role, text }: { role: Role; text: string }) {
  const api = useAssistantApi();
  const seeded = useRef(false);
  useEffect(() => {
    if (seeded.current) return;
    seeded.current = true;
    api.thread().append({ role, content: [{ type: 'text', text }] });
  }, [api, role, text]);
  return null;
}

function MessagePreview({ role, text }: { role: Role; text: string }) {
  return (
    <MyRuntimeProvider>
      <div className="dark w-full max-w-3xl h-[420px] bg-background text-foreground border rounded-md overflow-hidden">
        <SeedMessage role={role} text={text} />
        <Thread />
      </div>
    </MyRuntimeProvider>
  );
}

const meta = {
  title: 'Assistant UI/Message',
  component: MessagePreview,
  argTypes: {
    role: { control: 'select', options: ['user', 'assistant'] },
    text: { control: 'text' },
  },
  args: {
    role: 'assistant' as Role,
    text: 'Hello! I am your assistant. How can I help?',
  },
} satisfies Meta<typeof MessagePreview>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Assistant: Story = {
  args: { role: 'assistant', text: 'Hello there! I can help with UI tasks.' },
  render: (args) => <MessagePreview {...(args as any)} />,
};

export const User: Story = {
  args: { role: 'user', text: 'Can you summarize the latest changes?' },
  render: (args) => <MessagePreview {...(args as any)} />,
};
