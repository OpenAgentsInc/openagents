import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  NavigationMenu,
  NavigationMenuList,
  NavigationMenuItem,
  NavigationMenuTrigger,
  NavigationMenuContent,
  NavigationMenuLink,
} from "@openagentsinc/ui";

const meta = {
  title: 'UI/NavigationMenu',
  component: NavigationMenu,
  argTypes: {
    viewport: { control: 'boolean' },
    width: { control: 'number' },
  },
  args: {
    viewport: true,
    width: 720,
  },
} satisfies Meta<typeof NavigationMenu>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ viewport, width }) => (
    <div style={{ width: Number(width) }}>
      <NavigationMenu viewport={!!viewport}>
        <NavigationMenuList>
          <NavigationMenuItem>
            <NavigationMenuTrigger>Products</NavigationMenuTrigger>
            <NavigationMenuContent>
              <div style={{ display: 'grid', gridTemplateColumns: 'repeat(2, minmax(0,1fr))', gap: 8, width: 560 }}>
                <NavigationMenuLink href="#">Chat</NavigationMenuLink>
                <NavigationMenuLink href="#">Agents</NavigationMenuLink>
                <NavigationMenuLink href="#">Tools</NavigationMenuLink>
                <NavigationMenuLink href="#">Integrations</NavigationMenuLink>
              </div>
            </NavigationMenuContent>
          </NavigationMenuItem>
          <NavigationMenuItem>
            <NavigationMenuTrigger>Docs</NavigationMenuTrigger>
            <NavigationMenuContent>
              <div style={{ display: 'grid', gap: 8, width: 400 }}>
                <NavigationMenuLink href="#">Getting Started</NavigationMenuLink>
                <NavigationMenuLink href="#">Components</NavigationMenuLink>
                <NavigationMenuLink href="#">API</NavigationMenuLink>
              </div>
            </NavigationMenuContent>
          </NavigationMenuItem>
        </NavigationMenuList>
      </NavigationMenu>
    </div>
  ),
};

