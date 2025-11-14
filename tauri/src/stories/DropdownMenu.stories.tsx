import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuCheckboxItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSub,
  DropdownMenuSubTrigger,
  DropdownMenuSubContent,
  DropdownMenuShortcut,
} from "@openagentsinc/ui";
import { Button } from "@openagentsinc/ui";

const meta = {
  title: 'UI/DropdownMenu',
  component: DropdownMenu,
  argTypes: {
    label: { control: 'text' },
    showCheckboxes: { control: 'boolean' },
    showSubmenu: { control: 'boolean' },
  },
  args: {
    label: 'Open menu',
    showCheckboxes: true,
    showSubmenu: true,
  },
} satisfies Meta<typeof DropdownMenu>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ label, showCheckboxes, showSubmenu }) => (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button variant="outline">{label as string}</Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuLabel>My Account</DropdownMenuLabel>
        <DropdownMenuSeparator />
        <DropdownMenuGroup>
          <DropdownMenuItem>
            Profile <DropdownMenuShortcut>⌘P</DropdownMenuShortcut>
          </DropdownMenuItem>
          <DropdownMenuItem>Settings</DropdownMenuItem>
        </DropdownMenuGroup>
        {showCheckboxes && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuCheckboxItem checked>Show status</DropdownMenuCheckboxItem>
            <DropdownMenuCheckboxItem>Enable notifications</DropdownMenuCheckboxItem>
          </>
        )}
        {showSubmenu && (
          <>
            <DropdownMenuSeparator />
            <DropdownMenuSub>
              <DropdownMenuSubTrigger>More</DropdownMenuSubTrigger>
              <DropdownMenuSubContent>
                <DropdownMenuItem>About</DropdownMenuItem>
                <DropdownMenuItem>Help</DropdownMenuItem>
              </DropdownMenuSubContent>
            </DropdownMenuSub>
          </>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuRadioGroup value="en">
          <DropdownMenuLabel inset>Language</DropdownMenuLabel>
          <DropdownMenuRadioItem value="en">English</DropdownMenuRadioItem>
          <DropdownMenuRadioItem value="es">Spanish</DropdownMenuRadioItem>
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  ),
};

