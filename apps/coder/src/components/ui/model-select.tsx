import React, { useState, useEffect, useMemo } from "react";
import { MODELS, useSettings } from "@openagents/core";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "@/utils/tailwind";
import {
  Button,
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui";

interface ModelSelectProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  className?: string;
  disabled?: boolean;
}

export const ModelSelect = React.memo(function ModelSelect({
  value,
  onChange,
  placeholder = "Select a model",
  className,
  disabled = false,
}: ModelSelectProps) {
  const [open, setOpen] = useState(false);
  const [visibleModels, setVisibleModels] = useState<typeof MODELS>([]);
  const { settings } = useSettings();

  // Filter models based on visibility settings - create memoized value outside the effect
  const filteredModels = useMemo(() => {
    if (settings && settings.visibleModelIds && settings.visibleModelIds.length > 0) {
      // Filter MODELS to only include those in visibleModelIds
      return MODELS.filter(model => 
        settings.visibleModelIds!.includes(model.id)
      );
    } else {
      // Fall back to all models if no visibility settings are found
      return MODELS;
    }
  }, [settings]);
  
  // Update visible models only when filtered models change
  useEffect(() => {
    setVisibleModels(filteredModels);
  }, [filteredModels]);

  // Find the currently selected model with useMemo
  const selectedModel = useMemo(() => 
    MODELS.find((model) => model.id === value), 
    [value]
  );

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          className={cn(
            "w-full justify-between overflow-hidden text-ellipsis whitespace-nowrap font-mono",
            className
          )}
          disabled={disabled}
        >
          <span className="overflow-hidden text-ellipsis">
            {selectedModel ? selectedModel.name : placeholder}
          </span>
          <ChevronDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[300px] p-0 font-mono" align="start">
        <Command className="font-mono">
          <CommandInput placeholder="Search models..." className="font-mono" />
          <CommandEmpty className="font-mono">No model found.</CommandEmpty>
          <CommandGroup className="max-h-[300px] overflow-auto font-mono">
            {visibleModels.map((model) => {
              // Precompute this value to avoid recalculation in the render
              const isSelected = value === model.id;
              
              return (
                <CommandItem
                  key={model.id}
                  value={model.id}
                  onSelect={() => {
                    onChange(model.id);
                    setOpen(false);
                    window.dispatchEvent(new Event('focus-chat-input'));
                  }}
                  className="font-mono"
                >
                  <div className="flex flex-col gap-1 truncate font-mono">
                    <div className="flex items-center gap-2">
                      <Check
                        className={cn(
                          "h-4 w-4",
                          isSelected ? "opacity-100" : "opacity-0"
                        )}
                      />
                      <span className="font-medium font-mono">{model.name}</span>
                    </div>
                  </div>
                </CommandItem>
              );
            })}
          </CommandGroup>
        </Command>
      </PopoverContent>
    </Popover>
  );
});
