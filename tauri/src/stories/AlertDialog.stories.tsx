import type { Meta, StoryObj } from '@storybook/react-vite';
import { useEffect, useState } from 'react';
import {
  AlertDialog,
  AlertDialogTrigger,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogCancel,
  AlertDialogAction,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';

const meta = {
  title: 'UI/AlertDialog',
  component: AlertDialog,
  argTypes: {
    open: { control: 'boolean' },
    title: { control: 'text' },
    description: { control: 'text' },
    confirmLabel: { control: 'text' },
    cancelLabel: { control: 'text' },
  },
  args: {
    open: false,
    title: 'Delete file?',
    description: 'This action cannot be undone. This will permanently delete the file.',
    confirmLabel: 'Delete',
    cancelLabel: 'Cancel',
  },
} satisfies Meta<typeof AlertDialog>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ open, title, description, confirmLabel, cancelLabel }) => {
    const [isOpen, setIsOpen] = useState<boolean>(!!open);
    useEffect(() => setIsOpen(!!open), [open]);
    return (
      <AlertDialog open={isOpen} onOpenChange={setIsOpen}>
        <div style={{ display: 'flex', gap: 12 }}>
          <AlertDialogTrigger asChild>
            <Button variant="destructive">Open alert</Button>
          </AlertDialogTrigger>
          {!isOpen && (
            <Button variant="outline" onClick={() => setIsOpen(true)}>Open programmatically</Button>
          )}
        </div>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>{title as string}</AlertDialogTitle>
            <AlertDialogDescription>{description as string}</AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{cancelLabel as string}</AlertDialogCancel>
            <AlertDialogAction autoFocus onClick={() => setIsOpen(false)}>{confirmLabel as string}</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    );
  },
};

