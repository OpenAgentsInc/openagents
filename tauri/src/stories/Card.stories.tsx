import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardAction,
  CardContent,
  CardFooter,
} from '@/components/ui/card';
import { Button } from '@/components/ui/button';

const meta = {
  title: 'UI/Card',
  component: Card,
  argTypes: {
    heading: { control: 'text' },
    description: { control: 'text' },
    content: { control: 'text' },
    showFooter: { control: 'boolean' },
    width: { control: 'number' },
  },
  args: {
    heading: 'Card title',
    description: 'This is the card description area.',
    content:
      'Cards group related content and actions. Use header, content, and footer slots to compose rich layouts.',
    showFooter: true,
    width: 420,
  },
} satisfies Meta<typeof Card>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ heading, description, content, showFooter, width }) => (
    <Card style={{ width: Number(width) }}>
      <CardHeader>
        <CardTitle>{heading as string}</CardTitle>
        <CardDescription>{description as string}</CardDescription>
        <CardAction>
          <Button size="sm" variant="outline">
            Action
          </Button>
        </CardAction>
      </CardHeader>
      <CardContent>
        <p>{content as string}</p>
      </CardContent>
      {showFooter && (
        <CardFooter>
          <Button variant="secondary">Secondary</Button>
          <div style={{ flex: 1 }} />
          <Button>Primary</Button>
        </CardFooter>
      )}
    </Card>
  ),
};

