import type { Meta, StoryObj } from '@storybook/react-vite';
import { ResizablePanelGroup, ResizablePanel, ResizableHandle } from '@/components/ui/resizable';

const meta = {
  title: 'UI/Resizable',
  component: ResizablePanelGroup,
  argTypes: {
    direction: { control: 'select', options: ['horizontal', 'vertical'] },
    withHandle: { control: 'boolean' },
    width: { control: 'number' },
    height: { control: 'number' },
  },
  args: {
    direction: 'horizontal',
    withHandle: true,
    width: 560,
    height: 220,
  },
} satisfies Meta<typeof ResizablePanelGroup>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ direction, withHandle, width, height }) => (
    <div style={{ width: Number(width), height: Number(height) }}>
      <ResizablePanelGroup direction={direction as any}>
        <ResizablePanel defaultSize={50} className="grid place-content-center border">
          Left panel
        </ResizablePanel>
        <ResizableHandle withHandle={!!withHandle} />
        <ResizablePanel defaultSize={50} className="grid place-content-center border">
          Right panel
        </ResizablePanel>
      </ResizablePanelGroup>
    </div>
  ),
};

