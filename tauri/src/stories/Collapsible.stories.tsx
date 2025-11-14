import type { Meta, StoryObj } from '@storybook/react-vite';
import { useEffect, useState } from 'react';
import { Collapsible, CollapsibleTrigger, CollapsibleContent } from "@openagentsinc/ui";
import { Button } from "@openagentsinc/ui";

const meta = {
  title: 'UI/Collapsible',
  component: Collapsible,
  argTypes: {
    open: { control: 'boolean' },
    label: { control: 'text' },
    content: { control: 'text' },
  },
  args: {
    open: false,
    label: 'Toggle details',
    content: 'This is the collapsible content area. Put any content here.',
  },
} satisfies Meta<typeof Collapsible>;

export default meta;
type Story = StoryObj<typeof meta>;

export const Basic: Story = {
  render: ({ open, label, content }) => {
    const [isOpen, setIsOpen] = useState<boolean>(!!open);
    useEffect(() => setIsOpen(!!open), [open]);
    return (
      <Collapsible open={isOpen} onOpenChange={setIsOpen}>
        <CollapsibleTrigger asChild>
          <Button variant="outline">{label as string}</Button>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div className="mt-3 max-w-sm text-sm text-muted-foreground">{content as string}</div>
        </CollapsibleContent>
      </Collapsible>
    );
  },
};

