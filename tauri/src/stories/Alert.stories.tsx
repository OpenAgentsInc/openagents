import type { Meta, StoryObj } from '@storybook/react-vite';
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert';

const meta = {
  title: 'UI/Alert',
  component: Alert,
  argTypes: {
    variant: { control: 'select', options: ['default', 'destructive'] },
    title: { control: 'text' },
    description: { control: 'text' },
  },
  args: {
    variant: 'default',
    title: 'Heads up!'
      ,
    description: 'This is a simple alert component with two variants.',
  },
} satisfies Meta<typeof Alert>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ variant, title, description }) => (
    <Alert variant={variant as any}>
      <AlertTitle>{title as string}</AlertTitle>
      <AlertDescription>{description as string}</AlertDescription>
    </Alert>
  ),
};

export const Destructive: Story = {
  args: { variant: 'destructive' },
  render: ({ variant, title, description }) => (
    <Alert variant={variant as any}>
      <AlertTitle>{title as string}</AlertTitle>
      <AlertDescription>{description as string}</AlertDescription>
    </Alert>
  ),
};

