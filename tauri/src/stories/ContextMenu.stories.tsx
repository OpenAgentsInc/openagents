import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  ContextMenu,
  ContextMenuTrigger,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuCheckboxItem,
  ContextMenuSeparator,
  ContextMenuLabel,
  ContextMenuSub,
  ContextMenuSubTrigger,
  ContextMenuSubContent,
  ContextMenuRadioGroup,
  ContextMenuRadioItem,
  ContextMenuGroup,
  ContextMenuShortcut,
} from '@/components/ui/context-menu';

const meta = {
  title: 'UI/ContextMenu',
  component: ContextMenu,
  argTypes: {
    label: { control: 'text' },
    width: { control: 'number' },
    height: { control: 'number' },
    showCheckboxes: { control: 'boolean' },
    showSubmenu: { control: 'boolean' },
  },
  args: {
    label: 'Right-click in this area',
    width: 420,
    height: 180,
    showCheckboxes: true,
    showSubmenu: true,
  },
} satisfies Meta<typeof ContextMenu>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ label, width, height, showCheckboxes, showSubmenu }) => (
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div
          style={{ width: Number(width), height: Number(height) }}
          className="grid place-content-center rounded-md border text-sm text-muted-foreground"
        >
          {label as string}
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuLabel>My File</ContextMenuLabel>
        <ContextMenuGroup>
          <ContextMenuItem>
            Open <ContextMenuShortcut>⌘O</ContextMenuShortcut>
          </ContextMenuItem>
          <ContextMenuItem>Rename</ContextMenuItem>
          <ContextMenuItem variant="destructive">Delete</ContextMenuItem>
        </ContextMenuGroup>
        {showCheckboxes && (
          <>
            <ContextMenuSeparator />
            <ContextMenuCheckboxItem checked>Show hidden</ContextMenuCheckboxItem>
            <ContextMenuCheckboxItem>Enable sync</ContextMenuCheckboxItem>
          </>
        )}
        {showSubmenu && (
          <>
            <ContextMenuSeparator />
            <ContextMenuSub>
              <ContextMenuSubTrigger>Share</ContextMenuSubTrigger>
              <ContextMenuSubContent>
                <ContextMenuItem>Copy link</ContextMenuItem>
                <ContextMenuItem>Email…</ContextMenuItem>
              </ContextMenuSubContent>
            </ContextMenuSub>
          </>
        )}
        <ContextMenuSeparator />
        <ContextMenuLabel inset>Language</ContextMenuLabel>
        <ContextMenuRadioGroup value="en">
          <ContextMenuRadioItem value="en">English</ContextMenuRadioItem>
          <ContextMenuRadioItem value="es">Spanish</ContextMenuRadioItem>
          <ContextMenuRadioItem value="fr">French</ContextMenuRadioItem>
        </ContextMenuRadioGroup>
      </ContextMenuContent>
    </ContextMenu>
  ),
};

