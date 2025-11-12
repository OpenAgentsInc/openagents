import type { Meta, StoryObj } from '@storybook/react-vite';
import {
  Sheet,
  SheetTrigger,
  SheetContent,
  SheetHeader,
  SheetFooter,
  SheetTitle,
  SheetDescription,
  SheetClose,
} from '@/components/ui/sheet';
import { Button } from '@/components/ui/button';
import { useEffect, useState } from 'react';

const meta = {
  title: 'UI/Sheet',
  component: Sheet,
  argTypes: {
    open: { control: 'boolean' },
    side: { control: 'select', options: ['right', 'left', 'top', 'bottom'] },
    title: { control: 'text' },
    description: { control: 'text' },
  },
  args: {
    open: false,
    side: 'right',
    title: 'Sheet title',
    description: 'Sheets slide in from a side using Radix Dialog.',
  },
} satisfies Meta<typeof Sheet>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ open, side, title, description }) => {
    const [isOpen, setIsOpen] = useState<boolean>(!!open);
    useEffect(() => setIsOpen(!!open), [open]);
    return (
      <Sheet open={isOpen} onOpenChange={setIsOpen}>
        <div style={{ display: 'flex', gap: 12 }}>
          <SheetTrigger asChild>
            <Button variant="outline">Open sheet</Button>
          </SheetTrigger>
          {!isOpen && <Button onClick={() => setIsOpen(true)}>Open programmatically</Button>}
        </div>
        <SheetContent side={side as any}>
          <SheetHeader>
            <SheetTitle>{title as string}</SheetTitle>
            <SheetDescription>{description as string}</SheetDescription>
          </SheetHeader>
          <div className="px-4">Sheet content goes here.</div>
          <SheetFooter>
            <SheetClose asChild>
              <Button variant="secondary">Close</Button>
            </SheetClose>
            <Button onClick={() => setIsOpen(false)}>Save</Button>
          </SheetFooter>
        </SheetContent>
      </Sheet>
    );
  },
};

