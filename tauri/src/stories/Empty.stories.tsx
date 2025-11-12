import type { Meta, StoryObj } from '@storybook/react-vite';
import { Empty, EmptyHeader, EmptyTitle, EmptyDescription, EmptyContent, EmptyMedia } from '@/components/ui/empty';
import { Button } from '@/components/ui/button';
import { FolderPlus } from 'lucide-react';

const meta = {
  title: 'UI/Empty',
  component: Empty,
  argTypes: {
    title: { control: 'text' },
    description: { control: 'text' },
    showIcon: { control: 'boolean' },
    bordered: { control: 'boolean' },
    width: { control: 'number' },
    height: { control: 'number' },
  },
  args: {
    title: 'No files yet',
    description: 'Get started by creating a new file or folder.',
    showIcon: true,
    bordered: true,
    width: 520,
    height: 240,
  },
} satisfies Meta<typeof Empty>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ title, description, showIcon, bordered, width, height }) => (
    <div style={{ width: Number(width), height: Number(height), border: bordered ? '1px dashed var(--border)' : undefined, borderRadius: 8, display: 'grid' }}>
      <Empty>
        <EmptyHeader>
          {showIcon && (
            <EmptyMedia variant="icon">
              <FolderPlus />
            </EmptyMedia>
          )}
          <EmptyTitle>{title as string}</EmptyTitle>
          <EmptyDescription>{description as string}</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <div className="flex items-center gap-2">
            <Button variant="secondary">Create Folder</Button>
            <Button>New File</Button>
          </div>
        </EmptyContent>
      </Empty>
    </div>
  ),
};

