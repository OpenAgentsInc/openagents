import type { Meta, StoryObj } from '@storybook/react-vite';
import ModelToolbar from '@/components/assistant-ui/model-toolbar';
import { MyRuntimeProvider } from '@/runtime/MyRuntimeProvider';

const meta = {
  title: 'Assistant UI/ModelToolbar',
  component: ModelToolbar,
  decorators: [
    (Story) => (
      <MyRuntimeProvider>
        <div className="dark w-full max-w-2xl bg-background text-foreground border rounded-md overflow-hidden">
          <Story />
        </div>
      </MyRuntimeProvider>
    ),
  ],
} satisfies Meta<typeof ModelToolbar>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {};

