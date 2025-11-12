import type { Meta, StoryObj } from '@storybook/react-vite';
import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  DialogClose,
} from '@/components/ui/dialog';

const meta = {
  title: 'UI/Dialog',
  component: Dialog,
  argTypes: {
    title: { control: 'text' },
    description: { control: 'text' },
    showCloseButton: { control: 'boolean' },
    open: { control: 'boolean' },
    confirmLabel: { control: 'text' },
    cancelLabel: { control: 'text' },
  },
  args: {
    title: 'Example dialog',
    description: 'This is a simple dialog using our UI kit.',
    showCloseButton: true,
    open: false,
    confirmLabel: 'Confirm',
    cancelLabel: 'Cancel',
  },
} satisfies Meta<typeof Dialog>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ title, description, showCloseButton, open, confirmLabel, cancelLabel }) => {
    const [isOpen, setIsOpen] = useState<boolean>(!!open);
    useEffect(() => setIsOpen(!!open), [open]);
    return (
      <Dialog open={isOpen} onOpenChange={setIsOpen}>
        <div style={{ display: 'flex', gap: 12 }}>
          <DialogTrigger asChild>
            <Button variant="outline">Open dialog</Button>
          </DialogTrigger>
          {!isOpen && (
            <Button onClick={() => setIsOpen(true)}>Open programmatically</Button>
          )}
        </div>
        <DialogContent showCloseButton={showCloseButton}>
          <DialogHeader>
            <DialogTitle>{title}</DialogTitle>
            {description && <DialogDescription>{description}</DialogDescription>}
          </DialogHeader>
          <p>Put any content here.</p>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="secondary">{cancelLabel}</Button>
            </DialogClose>
            <Button onClick={() => setIsOpen(false)}>{confirmLabel}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    );
  },
};
