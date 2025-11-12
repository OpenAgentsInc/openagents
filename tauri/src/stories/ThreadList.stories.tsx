import type { Meta, StoryObj } from '@storybook/react-vite';
import { ThreadList } from '@/components/assistant-ui/thread-list';
import { MyRuntimeProvider } from '@/runtime/MyRuntimeProvider';

const meta = {
  title: 'Assistant UI/ThreadList',
  component: ThreadList,
  decorators: [
    (Story) => (
      <MyRuntimeProvider>
        <div className="dark w-72 h-[600px] bg-background text-foreground border rounded-md overflow-hidden p-2">
          <Story />
        </div>
      </MyRuntimeProvider>
    ),
  ],
} satisfies Meta<typeof ThreadList>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

