import type { Meta, StoryObj } from '@storybook/react-vite';
import { Button } from '@/components/ui/button';
import { ButtonGroup, ButtonGroupSeparator, ButtonGroupText } from '@/components/ui/button-group';
import { Copy, RefreshCw } from 'lucide-react';

const meta = {
  title: 'UI/ButtonGroup',
  component: ButtonGroup,
  argTypes: {
    orientation: { control: 'select', options: ['horizontal', 'vertical'] },
  },
  args: {
    orientation: 'horizontal',
  },
} satisfies Meta<typeof ButtonGroup>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ orientation }) => (
    <ButtonGroup orientation={orientation as any}>
      <Button variant="outline">Left</Button>
      <ButtonGroupSeparator />
      <Button variant="outline">Middle</Button>
      <ButtonGroupSeparator />
      <Button>Right</Button>
    </ButtonGroup>
  ),
};

export const WithText: Story = {
  render: ({ orientation }) => (
    <ButtonGroup orientation={orientation as any}>
      <ButtonGroupText><Copy /> Copy</ButtonGroupText>
      <Button variant="outline">Duplicate</Button>
      <Button><RefreshCw /> Refresh</Button>
    </ButtonGroup>
  ),
};

