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
import { TagIcon } from 'lucide-react';
import { useEffect, useId, useState } from 'react';
import { cn } from '@/lib/utils';
import { useLoaderData } from 'react-router';
import { CheckIcon } from 'lucide-react';

interface Label {
  id: string;
  name: string;
  color: string;
  description?: string;
}

interface LabelSelectorProps {
  selectedLabelIds: string[];
  onChange: (labelIds: string[]) => void;
}

export function LabelSelector({ selectedLabelIds, onChange }: LabelSelectorProps) {
  const id = useId();
  const [open, setOpen] = useState<boolean>(false);
  const loaderData = useLoaderData() || {};
  
  // Check for labels in various locations in the loader data
  let labels: Label[] = [];
  
  if (Array.isArray(loaderData.labels)) {
    labels = loaderData.labels;
  }
  
  // Handle label selection/deselection
  const handleLabelToggle = (labelId: string) => {
    const isSelected = selectedLabelIds.includes(labelId);
    let newLabelIds: string[];

    if (isSelected) {
      newLabelIds = selectedLabelIds.filter((id) => id !== labelId);
    } else {
      newLabelIds = [...selectedLabelIds, labelId];
    }

    onChange(newLabelIds);
  };

  // Get selected labels as full objects
  const selectedLabels = labels.filter(label => selectedLabelIds.includes(label.id));

  // If no labels are available, show a disabled button
  if (!labels || labels.length === 0) {
    return (
      <Button 
        variant="outline" 
        size="sm" 
        className="h-8 border-dashed flex items-center gap-1.5 bg-muted/50" 
        disabled
      >
        <TagIcon className="size-4" />
        <span className="text-muted-foreground">No labels available</span>
      </Button>
    );
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          className="flex items-center gap-1.5"
          size="sm"
          variant="outline"
          role="combobox"
          aria-expanded={open}
        >
          <TagIcon className="size-4" />
          {selectedLabelIds.length > 0 ? (
            <div className="flex -space-x-0.5">
              {selectedLabels.slice(0, 3).map((label) => (
                <div
                  key={label.id}
                  className="size-3 rounded-full"
                  style={{ backgroundColor: label.color }}
                />
              ))}
              {selectedLabelIds.length > 3 && (
                <span className="text-xs text-muted-foreground">
                  +{selectedLabelIds.length - 3}
                </span>
              )}
            </div>
          ) : (
            <span>Labels</span>
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        className="w-full min-w-[var(--radix-popper-anchor-width)] p-0"
        align="start"
      >
        <Command>
          <CommandInput placeholder="Search labels..." />
          <CommandList>
            <CommandEmpty>No labels found.</CommandEmpty>
            <CommandGroup>
              {labels.map((label) => {
                const isSelected = selectedLabelIds.includes(label.id);
                return (
                  <CommandItem
                    key={label.id}
                    value={label.name}
                    onSelect={() => handleLabelToggle(label.id)}
                    className="flex items-center justify-between"
                  >
                    <div className="flex items-center gap-2">
                      <div
                        className="size-3 rounded-full"
                        style={{ backgroundColor: label.color }}
                      />
                      <span>{label.name}</span>
                    </div>
                    {isSelected && <CheckIcon size={16} className="ml-auto" />}
                  </CommandItem>
                );
              })}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}