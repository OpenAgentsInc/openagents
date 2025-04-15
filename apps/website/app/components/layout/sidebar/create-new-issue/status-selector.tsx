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
import { CheckIcon } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { useLoaderData } from 'react-router';

interface WorkflowState {
  id: string;
  name: string;
  color: string;
  type?: string;
}

interface StatusSelectorProps {
  stateId: string;
  onChange: (stateId: string) => void;
}

export function StatusSelector({ stateId, onChange }: StatusSelectorProps) {
  const id = useId();
  const [open, setOpen] = useState<boolean>(false);
  const loaderData = useLoaderData() || {};
  
  // Check for workflow states in various locations in the loader data
  let workflowStates: WorkflowState[] = [];
  
  if (Array.isArray(loaderData.workflowStates)) {
    workflowStates = loaderData.workflowStates;
  }

  // Set a default state if none is selected and states are available
  useEffect(() => {
    if (!stateId && workflowStates.length > 0) {
      // Try to find a "todo" or "backlog" type state as default
      const defaultState = workflowStates.find(state => 
        state.type === 'todo' || state.type === 'backlog' || state.type === 'unstarted'
      ) || workflowStates[0];
      
      if (defaultState) {
        onChange(defaultState.id);
      }
    }
  }, [stateId, workflowStates, onChange]);

  const selectedState = workflowStates.find((state) => state.id === stateId);

  // If no states are available, show a disabled button
  if (!workflowStates || workflowStates.length === 0) {
    return (
      <Button
        variant="outline"
        size="sm"
        className="h-8 border-dashed bg-muted/50"
        disabled
      >
        <span className="text-muted-foreground">No states available</span>
      </Button>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          className="flex items-center justify-center gap-1.5"
          size="sm"
          variant="outline"
          role="combobox"
          aria-expanded={open}
        >
          {selectedState && (
            <>
              <div 
                className="size-3 rounded-full"
                style={{ backgroundColor: selectedState.color }}
              />
              <span>{selectedState.name}</span>
            </>
          )}
          {!selectedState && <span>Select status</span>}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        className="w-full min-w-[var(--radix-popper-anchor-width)] p-0"
        align="start"
      >
        <Command>
          <CommandInput placeholder="Search status..." />
          <CommandList>
            <CommandEmpty>No status found.</CommandEmpty>
            <CommandGroup>
              {workflowStates.map((state) => (
                <CommandItem
                  key={state.id}
                  value={state.name}
                  onSelect={() => {
                    onChange(state.id);
                    setOpen(false);
                  }}
                  className="flex items-center justify-between"
                >
                  <div className="flex items-center gap-2">
                    <div 
                      className="size-3 rounded-full"
                      style={{ backgroundColor: state.color }}
                    />
                    {state.name}
                  </div>
                  {stateId === state.id && <CheckIcon size={16} className="ml-auto" />}
                </CommandItem>
              ))}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}