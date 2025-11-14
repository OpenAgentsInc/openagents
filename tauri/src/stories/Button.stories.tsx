import type { Meta, StoryObj } from '@storybook/react-vite';
import { Button } from "@openagentsinc/ui";
import { Plus } from 'lucide-react';

const meta = {
  title: 'UI/Button',
  component: Button,
  parameters: {},
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'destructive', 'outline', 'secondary', 'ghost', 'link'],
    },
    size: {
      control: 'select',
      options: ['default', 'sm', 'lg', 'icon', 'icon-sm', 'icon-lg'],
    },
  },
  args: {
    children: 'Button',
  },
} satisfies Meta<typeof Button>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Default: Story = {
  args: {
    variant: 'default',
    size: 'default',
  },
};

export const Destructive: Story = {
  args: {
    variant: 'destructive',
  },
};

export const Outline: Story = {
  args: {
    variant: 'outline',
  },
};

export const Secondary: Story = {
  args: {
    variant: 'secondary',
  },
};

export const Ghost: Story = {
  args: {
    variant: 'ghost',
  },
};

export const Link: Story = {
  args: {
    variant: 'link',
  },
};

export const Small: Story = {
  args: {
    size: 'sm',
  },
};

export const Large: Story = {
  args: {
    size: 'lg',
  },
};

export const Icon: Story = {
  args: {
    size: 'icon',
    children: <Plus />,
    'aria-label': 'Add',
  },
};

export const IconSmall: Story = {
  args: {
    size: 'icon-sm',
    children: <Plus />,
    'aria-label': 'Add',
  },
};

export const IconLarge: Story = {
  args: {
    size: 'icon-lg',
    children: <Plus />,
    'aria-label': 'Add',
  },
};
