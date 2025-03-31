import React from "react";
import { models } from "@openagents/core";
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
  showFreePlanOnly?: boolean;
}

export function ModelSelect({
  value,
  onChange,
  placeholder = "Select a model",
  className,
  disabled = false,
  showFreePlanOnly = false,
}: ModelSelectProps) {
  const [open, setOpen] = React.useState(false);

  // Filter models based on plan if needed
  const filteredModels = showFreePlanOnly
    ? models.filter((model) => model.plan === "free")
    : models;

  // Find the currently selected model
  const selectedModel = filteredModels.find((model) => model.id === value);
  
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          className={cn(
            "w-full justify-between overflow-hidden text-ellipsis whitespace-nowrap",
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
      <PopoverContent className="w-[300px] p-0" align="start">
        <Command>
          <CommandInput placeholder="Search models..." />
          <CommandEmpty>No model found.</CommandEmpty>
          <CommandGroup className="max-h-[300px] overflow-auto">
            {filteredModels.map((model) => (
              <CommandItem
                key={model.id}
                value={model.id}
                onSelect={() => {
                  onChange(model.id);
                  setOpen(false);
                }}
              >
                <div className="flex flex-col gap-1 truncate">
                  <div className="flex items-center gap-2">
                    <Check
                      className={cn(
                        "h-4 w-4",
                        value === model.id ? "opacity-100" : "opacity-0"
                      )}
                    />
                    <span className="font-medium">{model.name}</span>
                    {model.plan === "pro" && (
                      <span className="ml-auto text-xs rounded bg-yellow-500/10 text-yellow-500 px-1.5 py-0.5">
                        PRO
                      </span>
                    )}
                  </div>
                  {model.shortDescription && (
                    <span className="text-xs text-muted-foreground pl-6">
                      {model.shortDescription}
                    </span>
                  )}
                </div>
              </CommandItem>
            ))}
          </CommandGroup>
        </Command>
      </PopoverContent>
    </Popover>
  );
}