import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  Drawer,
  DrawerTrigger,
  DrawerContent,
  DrawerHeader,
  DrawerFooter,
  DrawerTitle,
  DrawerDescription,
  DrawerClose,
} from "@openagentsinc/ui";
import { Button } from "@openagentsinc/ui";
import { useEffect, useState } from 'react';

const meta = {
  title: 'UI/Drawer',
  component: Drawer,
  argTypes: {
    open: { control: 'boolean' },
    direction: { control: 'select', options: ['top', 'right', 'bottom', 'left'] },
    title: { control: 'text' },
    description: { control: 'text' },
  },
  args: {
    open: false,
    direction: 'bottom',
    title: 'Drawer title',
    description: 'This is a Vaul-based drawer.',
  },
} satisfies Meta<typeof Drawer>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ open, direction, title, description }) => {
    const [isOpen, setIsOpen] = useState<boolean>(!!open);
    useEffect(() => setIsOpen(!!open), [open]);
    return (
      <Drawer open={isOpen} onOpenChange={setIsOpen} direction={direction as any}>
        <div style={{ display: 'flex', gap: 12 }}>
          <DrawerTrigger asChild>
            <Button variant="outline">Open drawer</Button>
          </DrawerTrigger>
          {!isOpen && <Button onClick={() => setIsOpen(true)}>Open programmatically</Button>}
        </div>
        <DrawerContent>
          <DrawerHeader>
            <DrawerTitle>{title as string}</DrawerTitle>
            <DrawerDescription>{description as string}</DrawerDescription>
          </DrawerHeader>
          <div className="px-4">Drawer content goes here.</div>
          <DrawerFooter>
            <DrawerClose asChild>
              <Button variant="secondary">Close</Button>
            </DrawerClose>
            <Button onClick={() => setIsOpen(false)}>Confirm</Button>
          </DrawerFooter>
        </DrawerContent>
      </Drawer>
    );
  },
};

