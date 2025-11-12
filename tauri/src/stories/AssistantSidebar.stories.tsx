import type { Meta, StoryObj } from '@storybook/react-vite';
import { AssistantSidebar } from '@/components/assistant-ui/assistant-sidebar';
import { MyRuntimeProvider } from '@/runtime/MyRuntimeProvider';

const meta = {
  title: 'Assistant UI/AssistantSidebar',
  component: AssistantSidebar,
  decorators: [
    (Story) => (
      <MyRuntimeProvider>
        <div className="dark w-full min-h-[80vh] bg-background text-foreground">
          <Story />
        </div>
      </MyRuntimeProvider>
    ),
  ],
  parameters: {
    layout: 'fullscreen',
  },
} satisfies Meta<typeof AssistantSidebar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

