import type { Meta, StoryObj } from '@storybook/react-vite';
import { Thread } from '@/components/assistant-ui/thread';
import { MyRuntimeProvider } from '@/runtime/MyRuntimeProvider';

const meta = {
  title: 'Assistant UI/Thread',
  component: Thread,
  decorators: [
    (Story) => (
      <MyRuntimeProvider>
        <div className="dark w-full max-w-5xl h-[640px] mx-auto bg-background text-foreground border rounded-md overflow-hidden">
          <Story />
        </div>
      </MyRuntimeProvider>
    ),
  ],
} satisfies Meta<typeof Thread>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Empty: Story = {};

