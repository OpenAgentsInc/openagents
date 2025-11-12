import type { Meta, StoryObj } from '@storybook/react-vite';
import { AppHeader } from '@/components/assistant-ui/app-header';

const meta = {
  title: 'Assistant UI/AppHeader',
  component: AppHeader,
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof AppHeader>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  render: () => (
    <div className="dark w-full bg-background text-foreground">
      <AppHeader />
    </div>
  ),
};

