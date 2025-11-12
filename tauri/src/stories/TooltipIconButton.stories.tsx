import type { Meta, StoryObj } from '@storybook/react-vite';
import { TooltipIconButton } from '@/components/assistant-ui/tooltip-icon-button';
import { Plus, RefreshCw, Copy } from 'lucide-react';

const meta = {
  title: 'Assistant UI/TooltipIconButton',
  component: TooltipIconButton,
  argTypes: {
    side: { control: 'select', options: ['top', 'bottom', 'left', 'right'] },
    variant: {
      control: 'select',
      options: ['default', 'destructive', 'outline', 'secondary', 'ghost', 'link'],
    },
    size: { control: 'select', options: ['default', 'sm', 'lg', 'icon', 'icon-sm', 'icon-lg'] },
  },
  args: {
    tooltip: 'Action',
    side: 'bottom',
    variant: 'ghost',
    size: 'icon',
  },
} satisfies Meta<typeof TooltipIconButton>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Add: Story = {
  render: (args) => (
    <TooltipIconButton {...args}>
      <Plus />
    </TooltipIconButton>
  ),
};

export const Refresh: Story = {
  render: (args) => (
    <TooltipIconButton {...args} tooltip="Refresh">
      <RefreshCw />
    </TooltipIconButton>
  ),
};

export const CopyText: Story = {
  render: (args) => (
    <TooltipIconButton {...args} tooltip="Copy">
      <Copy />
    </TooltipIconButton>
  ),
};

