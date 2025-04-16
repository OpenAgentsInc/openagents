import { Button } from '@/components/ui/button';
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { useIssuesStore } from '@/store/issues-store';
import { type Priority, priorities } from '@/mock-data/priorities';
import { CheckIcon } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { useSubmit } from 'react-router';

interface PrioritySelectorProps {
  priority: Priority;
  issueId?: string;
}

export function PrioritySelector({ priority, issueId }: PrioritySelectorProps) {
  const id = useId();
  const [open, setOpen] = useState<boolean>(false);
  const [value, setValue] = useState<string>(priority.id);
  const submit = useSubmit();

  const { filterByPriority, updateIssuePriority } = useIssuesStore();

  useEffect(() => {
    setValue(priority.id);
  }, [priority.id]);

  const handlePriorityChange = (priorityId: string) => {
    setValue(priorityId);
    setOpen(false);

    if (issueId) {
      const newPriority = priorities.find((p) => p.id === priorityId);
      if (newPriority) {
        // Update UI state immediately
        updateIssuePriority(issueId, newPriority);
        
        // Map priority IDs to numeric values for the database
        let priorityValue = 0; // default: no priority
        switch (priorityId) {
          case 'urgent': priorityValue = 1; break;
          case 'high': priorityValue = 2; break;
          case 'medium': priorityValue = 3; break;
          case 'low': priorityValue = 4; break;
        }
        
        // Then send the update to the server
        const formData = new FormData();
        formData.append('_action', 'update');
        formData.append('id', issueId);
        formData.append('priority', priorityValue.toString());
        
        // Always submit to the issues route as a fetch request instead of navigation
        submit(formData, {
          method: 'post',
          action: '/issues', // Explicitly target the issues route which has the action
          navigate: false, // This prevents navigation and keeps the current route
          replace: true // This causes the page state to be updated with the server response
        });
      }
    }
  };

  return (
    <div className="*:not-first:mt-2 stop-propagation">
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            id={id}
            className="size-7 flex items-center justify-center"
            size="icon"
            variant="ghost"
            role="combobox"
            aria-expanded={open}
          >
            {(() => {
              const selectedItem = priorities.find((item) => item.id === value);
              if (selectedItem) {
                const Icon = selectedItem.icon;
                return <Icon className="text-muted-foreground size-4" />;
              }
              return null;
            })()}
          </Button>
        </PopoverTrigger>
        <PopoverContent
          className="border-input w-full min-w-[var(--radix-popper-anchor-width)] p-0 popover-content"
          align="start"
        >
          <Command>
            <CommandInput placeholder="Set priority..." />
            <CommandList>
              <CommandEmpty>No priority found.</CommandEmpty>
              <CommandGroup>
                {priorities.map((item) => (
                  <CommandItem
                    key={item.id}
                    value={item.id}
                    onSelect={handlePriorityChange}
                    className="flex items-center justify-between"
                  >
                    <div className="flex items-center gap-2">
                      <item.icon className="text-muted-foreground size-4" />
                      {item.name}
                    </div>
                    {value === item.id && <CheckIcon size={16} className="ml-auto" />}
                    <span className="text-muted-foreground text-xs">
                      {filterByPriority(item.id).length}
                    </span>
                  </CommandItem>
                ))}
              </CommandGroup>
            </CommandList>
          </Command>
        </PopoverContent>
      </Popover>
    </div>
  );
}
