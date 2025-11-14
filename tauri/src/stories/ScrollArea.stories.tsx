import type { Meta, StoryObj } from '@storybook/react-vite';
import { ScrollArea } from "@openagentsinc/ui";

const meta = {
  title: 'UI/ScrollArea',
  component: ScrollArea,
  argTypes: {
    width: { control: 'number' },
    height: { control: 'number' },
    items: { control: 'number' },
  },
  args: {
    width: 320,
    height: 240,
    items: 40,
  },
} satisfies Meta<typeof ScrollArea>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Vertical: Story = {
  render: ({ width, height, items }) => (
    <ScrollArea style={{ width: Number(width), height: Number(height), border: '1px solid var(--border)', borderRadius: 6, padding: 8 }}>
      <div style={{ display: 'grid', gap: 8 }}>
        {Array.from({ length: Math.max(1, Number(items)) }).map((_, i) => (
          <div key={i} style={{ padding: 8, border: '1px solid var(--border)', borderRadius: 6 }}>
            Row {i + 1}
          </div>
        ))}
      </div>
    </ScrollArea>
  ),
};

