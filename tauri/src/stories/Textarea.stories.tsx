import type { Meta, StoryObj } from '@storybook/react-vite';
import { Textarea } from '@/components/ui/textarea';

const meta = {
  title: 'UI/Textarea',
  component: Textarea,
  argTypes: {
    placeholder: { control: 'text' },
    rows: { control: 'number' },
    width: { control: 'number' },
    invalid: { control: 'boolean' },
    disabled: { control: 'boolean' },
  },
  args: {
    placeholder: 'Type your message here...',
    rows: 6,
    width: 420,
    invalid: false,
    disabled: false,
  },
} satisfies Meta<typeof Textarea>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ placeholder, rows, width, invalid, disabled }) => (
    <Textarea
      placeholder={placeholder as string}
      rows={Number(rows)}
      style={{ width: Number(width) }}
      aria-invalid={invalid ? true : undefined}
      disabled={!!disabled}
    />
  ),
};

