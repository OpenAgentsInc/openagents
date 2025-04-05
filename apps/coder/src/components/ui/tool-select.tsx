import React, { useState, useEffect, useMemo } from "react";
import { 
  TOOLS, 
  ToolDefinition, 
  extendWithMCPTools, 
  useSettings 
} from "@openagents/core";
import { Check, ChevronDown, Wrench, AlertCircle } from "lucide-react";
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
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  Badge
} from "@/components/ui";
import { getMCPClients } from "@/server/mcp-clients";

interface ToolSelectProps {
  selectedToolIds: string[];
  onChange: (toolIds: string[]) => void;
  placeholder?: string;
  className?: string;
  disabled?: boolean;
}

export function ToolSelect({
  selectedToolIds,
  onChange,
  placeholder = "Select tools",
  className,
  disabled = false
}: ToolSelectProps) {
  const [open, setOpen] = useState(false);
  const [allTools, setAllTools] = useState<ToolDefinition[]>([]);
  const { getEnabledToolIds } = useSettings();
  const [enabledToolIds, setEnabledToolIds] = useState<string[]>([]);

  // Load all tools (built-in and MCP)
  useEffect(() => {
    const loadTools = async () => {
      try {
        // Get globally enabled tool IDs
        const enabledIds = await getEnabledToolIds();
        setEnabledToolIds(enabledIds);
        
        // Get MCP tools
        let mcpTools = {};
        try {
          // In browser environment, just use empty object for MCP tools
          if (typeof window !== 'undefined' && process.env.NODE_ENV === 'development') {
            // In development, use mock tools
            mcpTools = {
              'github_search': {
                name: 'GitHub Search',
                description: 'Search GitHub repositories',
                parameters: {
                  type: 'object',
                  properties: {
                    query: {
                      type: 'string',
                      description: 'The search query'
                    }
                  },
                  required: ['query']
                }
              }
            };
          } else {
            // Try to get real tools
            try {
              const { allTools: mcpToolsList } = getMCPClients();
              mcpTools = mcpToolsList || {};
            } catch (error) {
              console.warn("Error fetching MCP tools, using empty object:", error);
            }
          }
        } catch (error) {
          console.error("Error setting up MCP tools:", error);
        }
        
        // Combine with built-in tools
        const combinedTools = extendWithMCPTools(mcpTools);
        setAllTools(combinedTools);
      } catch (error) {
        console.error("Error loading tools:", error);
        // Fallback to built-in tools only
        setAllTools(TOOLS);
      }
    };
    
    loadTools();
  }, [getEnabledToolIds]);

  // Filter to only show globally enabled tools
  const availableTools = useMemo(() => {
    return allTools.filter(tool => enabledToolIds.includes(tool.id));
  }, [allTools, enabledToolIds]);

  // Toggle a tool's selection
  const toggleTool = (toolId: string) => {
    if (selectedToolIds.includes(toolId)) {
      onChange(selectedToolIds.filter(id => id !== toolId));
    } else {
      onChange([...selectedToolIds, toolId]);
    }
  };

  // Select all available tools
  const selectAllTools = () => {
    onChange(availableTools.map(tool => tool.id));
  };

  // Clear all selected tools
  const clearSelection = () => {
    onChange([]);
  };

  // Check if all available tools are selected
  const allSelected = useMemo(() => {
    return availableTools.length > 0 && 
      availableTools.every(tool => selectedToolIds.includes(tool.id));
  }, [availableTools, selectedToolIds]);

  // Get display text for button
  const displayText = useMemo(() => {
    if (selectedToolIds.length === 0) {
      return placeholder;
    } else if (selectedToolIds.length === 1) {
      const tool = allTools.find(t => t.id === selectedToolIds[0]);
      return tool ? tool.name : selectedToolIds[0];
    } else if (allSelected) {
      return "All tools enabled";
    } else {
      return `${selectedToolIds.length} tools enabled`;
    }
  }, [selectedToolIds, allTools, placeholder, allSelected]);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          className={cn(
            "justify-between overflow-hidden text-ellipsis whitespace-nowrap",
            className
          )}
          disabled={disabled}
        >
          <div className="flex items-center overflow-hidden">
            <Wrench className="mr-2 h-4 w-4" />
            <span className="overflow-hidden text-ellipsis">
              {displayText}
            </span>
            {selectedToolIds.length > 0 && (
              <Badge variant="secondary" className="ml-2">
                {selectedToolIds.length}
              </Badge>
            )}
          </div>
          <ChevronDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[300px] p-0" align="start">
        <Command>
          <CommandInput placeholder="Search tools..." />
          <CommandEmpty>
            <div className="flex flex-col items-center justify-center p-4 text-sm text-muted-foreground">
              <AlertCircle className="h-6 w-6 mb-2" />
              <p>No tools found.</p>
            </div>
          </CommandEmpty>
          
          {/* Action buttons */}
          <div className="flex items-center justify-between p-2 border-b">
            <Button 
              variant="ghost" 
              size="sm" 
              className="text-xs"
              onClick={selectAllTools}
              disabled={allSelected || availableTools.length === 0}
            >
              Select all
            </Button>
            <Button 
              variant="ghost" 
              size="sm" 
              className="text-xs"
              onClick={clearSelection}
              disabled={selectedToolIds.length === 0}
            >
              Clear selection
            </Button>
          </div>
          
          <CommandGroup className="max-h-[200px] overflow-auto">
            {availableTools.length === 0 ? (
              <div className="flex flex-col items-center justify-center p-4 text-sm text-muted-foreground">
                <AlertCircle className="h-6 w-6 mb-2" />
                <p>No tools are enabled globally.</p>
                <p className="text-xs mt-1">Enable tools in Settings → Tools.</p>
              </div>
            ) : (
              availableTools.map((tool) => {
                const isSelected = selectedToolIds.includes(tool.id);
                
                return (
                  <CommandItem
                    key={tool.id}
                    value={tool.id}
                    onSelect={() => toggleTool(tool.id)}
                  >
                    <div className="flex flex-col gap-1 truncate">
                      <div className="flex items-center gap-2">
                        <span className="h-4 w-4 flex items-center justify-center">
                          {isSelected ? <Check className="h-4 w-4" /> : null}
                        </span>
                        <span className="font-medium">{tool.name}</span>
                        <span className={`ml-auto px-2 py-0.5 rounded-md text-xs ${
                          tool.type === 'builtin' 
                            ? 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200' 
                            : 'bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200'
                        }`}>
                          {tool.type}
                        </span>
                      </div>
                      <div className="text-xs text-muted-foreground pl-6">
                        {tool.description}
                      </div>
                    </div>
                  </CommandItem>
                );
              })
            )}
          </CommandGroup>
        </Command>
      </PopoverContent>
    </Popover>
  );
}