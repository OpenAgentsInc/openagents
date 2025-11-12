import type { Meta, StoryObj } from '@storybook/react-vite';
import { Input } from '@/components/ui/input';

const meta = {
  title: 'UI/Input',
  component: Input,
  argTypes: {
    type: {
      control: 'select',
      options: ['text', 'password', 'email', 'file', 'number', 'search', 'url'],
    },
    placeholder: { control: 'text' },
    width: { control: 'number' },
    invalid: { control: 'boolean' },
    disabled: { control: 'boolean' },
  },
  args: {
    type: 'text',
    placeholder: 'Enter text',
    width: 320,
    invalid: false,
    disabled: false,
  },
} satisfies Meta<typeof Input>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ type, placeholder, width, invalid, disabled }) => (
    <Input
      type={type as string}
      placeholder={placeholder as string}
      style={{ width: Number(width) }}
      aria-invalid={invalid ? true : undefined}
      disabled={!!disabled}
    />
  ),
};

export const Password: Story = {
  args: { type: 'password', placeholder: 'Enter password' },
};

export const File: Story = {
  args: { type: 'file', placeholder: undefined },
};

