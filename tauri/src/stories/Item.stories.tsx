import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  ItemGroup,
  Item,
  ItemMedia,
  ItemContent,
  ItemTitle,
  ItemDescription,
  ItemActions,
  ItemSeparator,
} from "@openagentsinc/ui";
import { Button } from "@openagentsinc/ui";
import { Star, Download } from 'lucide-react';

const meta = {
  title: 'UI/Item',
  component: Item,
  argTypes: {
    variant: { control: 'select', options: ['default', 'outline', 'muted'] },
    size: { control: 'select', options: ['default', 'sm'] },
  },
  args: {
    variant: 'default',
    size: 'default',
  },
} satisfies Meta<typeof Item>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ variant, size }) => (
    <ItemGroup>
      <Item variant={variant as any} size={size as any}>
        <ItemMedia variant="icon"><Star /></ItemMedia>
        <ItemContent>
          <ItemTitle>Project Alpha</ItemTitle>
          <ItemDescription>A concise description of the project goes here.</ItemDescription>
        </ItemContent>
        <ItemActions>
          <Button size="sm" variant="outline">View</Button>
          <Button size="sm">Open</Button>
        </ItemActions>
      </Item>
      <ItemSeparator />
      <Item variant="outline" size="sm">
        <ItemMedia variant="image">
          <img alt="preview" src="https://picsum.photos/64/64" />
        </ItemMedia>
        <ItemContent>
          <ItemTitle>Report.pdf</ItemTitle>
          <ItemDescription>Exported 2 hours ago • 2.3 MB</ItemDescription>
        </ItemContent>
        <ItemActions>
          <Button size="sm" variant="ghost"><Download /></Button>
        </ItemActions>
      </Item>
    </ItemGroup>
  ),
};

