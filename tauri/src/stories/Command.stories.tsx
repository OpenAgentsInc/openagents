import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  CommandDialog,
  CommandInput,
  CommandList,
  CommandEmpty,
  CommandGroup,
  CommandItem,
  CommandSeparator,
  CommandShortcut,
} from "@openagentsinc/ui";
import { useEffect, useState } from 'react';

const meta = {
  title: 'UI/Command',
  component: CommandDialog,
  argTypes: {
    open: { control: 'boolean' },
    showCloseButton: { control: 'boolean' },
    title: { control: 'text' },
    description: { control: 'text' },
  },
  args: {
    open: false,
    showCloseButton: true,
    title: 'Command Palette',
    description: 'Search and run quick actions.',
  },
} satisfies Meta<typeof CommandDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ open, showCloseButton, title, description }) => {
    const [isOpen, setIsOpen] = useState<boolean>(!!open);
    useEffect(() => setIsOpen(!!open), [open]);
    return (
      <CommandDialog open={isOpen} onOpenChange={setIsOpen} showCloseButton={!!showCloseButton} title={title as string} description={description as string}>
        <CommandInput placeholder="Type a command or search…" />
        <CommandList>
          <CommandEmpty>No results found.</CommandEmpty>
          <CommandGroup heading="Suggestions">
            <CommandItem onSelect={() => setIsOpen(false)}>
              New File <CommandShortcut>⌘N</CommandShortcut>
            </CommandItem>
            <CommandItem onSelect={() => setIsOpen(false)}>
              Open File <CommandShortcut>⌘O</CommandShortcut>
            </CommandItem>
            <CommandItem onSelect={() => setIsOpen(false)}>
              Save <CommandShortcut>⌘S</CommandShortcut>
            </CommandItem>
          </CommandGroup>
          <CommandSeparator />
          <CommandGroup heading="Navigation">
            <CommandItem>Home</CommandItem>
            <CommandItem>Settings</CommandItem>
            <CommandItem>About</CommandItem>
          </CommandGroup>
        </CommandList>
      </CommandDialog>
    );
  },
};

