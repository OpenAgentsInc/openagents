import React, { useState, useEffect, useMemo } from "react";
import { 
  TOOLS, 
  ToolDefinition, 
  extendWithMCPTools, 
  useSettings 
} from "@openagents/core";
import { 
  Check, 
  ChevronDown, 
  ChevronRight, 
  Wrench, 
  AlertCircle, 
  CloudCog,
  Server
} from "lucide-react";
import { cn } from "@/utils/tailwind";
import {
  Button,
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandSeparator,
  CommandList,
  Popover,
  PopoverContent,
  PopoverTrigger,
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  Badge,
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "@/components/ui";
import { getMCPClients, refreshTools } from "@/server/mcp-clients";

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
  // Keep local state for selected tools to ensure UI updates immediately
  const [localSelectedToolIds, setSelectedToolIds] = useState<string[]>(selectedToolIds);
  // Track expanded provider groups
  const [expandedProviders, setExpandedProviders] = useState<Record<string, boolean>>({});

  // Load all tools (built-in and MCP)
  useEffect(() => {
    const loadTools = async () => {
      try {
        // Get globally enabled tool IDs
        const enabledIds = await getEnabledToolIds();
        setEnabledToolIds(enabledIds);
        
        // Get MCP tools and clients
        let mcpTools = {};
        let clientInfoMap: Record<string, { id: string; name: string; tools?: string[] }> = {};
        
        try {
          // In browser environment, handle appropriately
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
              },
              'github_repo': {
                name: 'GitHub Repository Info',
                description: 'Get information about a GitHub repository',
                parameters: {
                  type: 'object',
                  properties: {
                    owner: {
                      type: 'string',
                      description: 'Repository owner'
                    },
                    repo: {
                      type: 'string',
                      description: 'Repository name'
                    }
                  },
                  required: ['owner', 'repo']
                }
              }
            };
            
            // Add mock client info
            clientInfoMap = {
              'mock-github': {
                id: 'mock-github',
                name: 'GitHub MCP (Mock)',
                tools: ['github_search', 'github_repo']
              }
            };
          } else {
            // Try to get real tools and client info
            try {
              // Try to refresh tools first
              try {
                console.log('[ToolSelect] Refreshing MCP tools before fetching...');
                await refreshTools();
              } catch (refreshError) {
                console.warn('[ToolSelect] Error refreshing tools:', refreshError);
              }
              
              // Now get the updated tools
              const mcpClientsInfo = getMCPClients();
              const { allTools: mcpToolsList, clientTools, configs, clients } = mcpClientsInfo;
              
              console.log('[ToolSelect] MCP Clients info:', {
                clientsCount: Object.keys(clients).length,
                toolsCount: Object.keys(mcpToolsList || {}).length,
                clientToolsInfo: clientTools,
                configsCount: Object.keys(configs).length
              });
              
              mcpTools = mcpToolsList || {};
              
              // Create client info map for tracking which tools belong to which client
              Object.entries(configs).forEach(([clientId, config]) => {
                if (clientTools[clientId]) {
                  clientInfoMap[clientId] = {
                    id: clientId,
                    name: config.name,
                    tools: clientTools[clientId]
                  };
                  
                  console.log(`[ToolSelect] Client ${config.name} has ${clientTools[clientId].length} tools`);
                } else {
                  console.log(`[ToolSelect] Client ${config.name} has no tools registered`);
                }
              });
            } catch (error) {
              console.warn("Error fetching MCP tools, using empty object:", error);
            }
          }
        } catch (error) {
          console.error("Error setting up MCP tools:", error);
        }
        
        // Combine with built-in tools
        const combinedTools = extendWithMCPTools(mcpTools, clientInfoMap);
        console.log(`[ToolSelect] Combined tools: ${combinedTools.length} tools (${Object.keys(mcpTools).length} MCP tools + ${TOOLS.length} built-in tools)`);
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
  
  // Group tools by provider
  const providerGroups = useMemo(() => {
    const result: Record<string, { 
      name: string,
      id: string,
      tools: ToolDefinition[]
    }> = {
      builtin: {
        name: "Built-in Tools",
        id: "builtin",
        tools: []
      }
    };
    
    // Group tools by provider
    availableTools.forEach(tool => {
      if (tool.type === 'builtin') {
        result.builtin.tools.push(tool);
      } else if (tool.providerId && tool.providerName) {
        // Create provider group if it doesn't exist
        if (!result[tool.providerId]) {
          result[tool.providerId] = {
            name: tool.providerName,
            id: tool.providerId,
            tools: []
          };
        }
        
        // Add tool to provider group
        result[tool.providerId].tools.push(tool);
      } else {
        // If no provider specified but type is 'mcp', add to "Other MCP Tools" group
        if (!result.otherMcp) {
          result.otherMcp = {
            name: "Other MCP Tools",
            id: "otherMcp",
            tools: []
          };
        }
        
        result.otherMcp.tools.push(tool);
      }
    });
    
    return result;
  }, [availableTools]);

  // Toggle a tool's selection
  const toggleTool = (toolId: string) => {
    if (localSelectedToolIds.includes(toolId)) {
      const newSelection = localSelectedToolIds.filter(id => id !== toolId);
      setSelectedToolIds(newSelection); // Update local state immediately for visual feedback
      onChange(newSelection);
    } else {
      const newSelection = [...localSelectedToolIds, toolId];
      setSelectedToolIds(newSelection); // Update local state immediately for visual feedback
      onChange(newSelection);
    }
  };

  // Select all available tools
  const selectAllTools = () => {
    const allToolIds = availableTools.map(tool => tool.id);
    setSelectedToolIds(allToolIds); // Update local state immediately
    onChange(allToolIds);
  };

  // Clear all selected tools
  const clearSelection = () => {
    setSelectedToolIds([]); // Update local state immediately
    onChange([]);
  };

  // Check if all available tools are selected
  const allSelected = useMemo(() => {
    return availableTools.length > 0 && 
      availableTools.every(tool => localSelectedToolIds.includes(tool.id));
  }, [availableTools, localSelectedToolIds]);

  // Update local state when props change, but only if different
  useEffect(() => {
    // Check if arrays are different to avoid unnecessary state updates
    const isSameSelection = 
      localSelectedToolIds.length === selectedToolIds.length && 
      localSelectedToolIds.every(id => selectedToolIds.includes(id));
    
    if (!isSameSelection) {
      setSelectedToolIds(selectedToolIds);
    }
  }, [selectedToolIds, localSelectedToolIds]);

  // Get display text for button
  const displayText = useMemo(() => {
    if (localSelectedToolIds.length === 0) {
      return placeholder;
    } else if (localSelectedToolIds.length === 1) {
      const tool = allTools.find(t => t.id === localSelectedToolIds[0]);
      return tool ? tool.name : localSelectedToolIds[0];
    } else if (allSelected) {
      return "All tools enabled";
    } else {
      return `${localSelectedToolIds.length} tools enabled`;
    }
  }, [localSelectedToolIds, allTools, placeholder, allSelected]);

  // Initialize expanded state when providers change
  const providerIds = useMemo(() => Object.keys(providerGroups), [providerGroups]);
  
  useEffect(() => {
    const initialExpandState: Record<string, boolean> = { ...expandedProviders };
    
    // Initialize any new providers to expanded by default
    let changed = false;
    providerIds.forEach(id => {
      if (expandedProviders[id] === undefined) {
        initialExpandState[id] = true; // Default to expanded
        changed = true;
      }
    });
    
    // Only set state if there are actual changes to avoid unnecessary renders
    if (changed) {
      setExpandedProviders(initialExpandState);
    }
  }, [providerIds, expandedProviders]);
  
  // Toggle a provider's expanded state
  const toggleProviderExpanded = (providerId: string) => {
    setExpandedProviders(prev => ({
      ...prev,
      [providerId]: !prev[providerId]
    }));
  };
  
  // Select all tools in a provider
  const selectProviderTools = (providerId: string) => {
    const providerTools = providerGroups[providerId]?.tools || [];
    const providerToolIds = providerTools.map(tool => tool.id);
    
    // Combine existing selection with all provider tools
    const newSelection = [...new Set([
      ...localSelectedToolIds,
      ...providerToolIds
    ])];
    
    setSelectedToolIds(newSelection);
    onChange(newSelection);
  };
  
  // Deselect all tools in a provider
  const deselectProviderTools = (providerId: string) => {
    const providerTools = providerGroups[providerId]?.tools || [];
    const providerToolIds = providerTools.map(tool => tool.id);
    
    // Remove all provider tools from selection
    const newSelection = localSelectedToolIds.filter(
      id => !providerToolIds.includes(id)
    );
    
    setSelectedToolIds(newSelection);
    onChange(newSelection);
  };
  
  // Check if all tools in a provider are selected
  const areAllProviderToolsSelected = (providerId: string) => {
    const providerTools = providerGroups[providerId]?.tools || [];
    return providerTools.length > 0 && 
      providerTools.every(tool => localSelectedToolIds.includes(tool.id));
  };
  
  // Check if any tools in a provider are selected
  const areSomeProviderToolsSelected = (providerId: string) => {
    const providerTools = providerGroups[providerId]?.tools || [];
    return providerTools.some(tool => localSelectedToolIds.includes(tool.id));
  };

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
            {localSelectedToolIds.length > 0 && (
              <Badge variant="secondary" className="ml-2">
                {localSelectedToolIds.length}
              </Badge>
            )}
          </div>
          <ChevronDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[350px] p-0" align="start">
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
          
          <CommandList className="max-h-[350px] overflow-auto">
            {availableTools.length === 0 ? (
              <div className="flex flex-col items-center justify-center p-4 text-sm text-muted-foreground">
                <AlertCircle className="h-6 w-6 mb-2" />
                <p>No tools are enabled globally.</p>
                <p className="text-xs mt-1">Enable tools in Settings → Tools.</p>
              </div>
            ) : (
              // Show tools grouped by provider
              Object.entries(providerGroups).map(([providerId, provider]) => {
                if (provider.tools.length === 0) return null;
                
                const isExpanded = expandedProviders[providerId] !== false; // Default to true
                const allSelected = areAllProviderToolsSelected(providerId);
                const someSelected = areSomeProviderToolsSelected(providerId);
                
                return (
                  <div key={providerId} className="mb-1">
                    {/* Provider header */}
                    <div 
                      className="flex items-center justify-between px-2 py-1.5 text-sm font-medium cursor-pointer hover:bg-muted/50"
                      onClick={() => toggleProviderExpanded(providerId)}
                    >
                      <div className="flex items-center gap-1.5">
                        {isExpanded ? (
                          <ChevronDown className="h-3 w-3 text-muted-foreground" />
                        ) : (
                          <ChevronRight className="h-3 w-3 text-muted-foreground" />
                        )}
                        {provider.id === 'builtin' ? (
                          <Server className="h-3.5 w-3.5 text-blue-500" />
                        ) : (
                          <CloudCog className="h-3.5 w-3.5 text-purple-500" />
                        )}
                        <span>{provider.name}</span>
                        <span className="text-xs text-muted-foreground ml-1">
                          ({provider.tools.length})
                        </span>
                        
                        {someSelected && (
                          <Badge variant={allSelected ? "default" : "outline"} className="ml-2 h-5 text-xs px-1.5">
                            {allSelected ? 'All' : `${provider.tools.filter(t => 
                              localSelectedToolIds.includes(t.id)).length}/${provider.tools.length}`}
                          </Badge>
                        )}
                      </div>
                      
                      {/* Provider actions */}
                      <div className="flex items-center gap-1" onClick={e => e.stopPropagation()}>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-2 text-[10px]"
                          onClick={() => selectProviderTools(providerId)}
                          disabled={allSelected}
                        >
                          Select
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-2 text-[10px]"
                          onClick={() => deselectProviderTools(providerId)}
                          disabled={!someSelected}
                        >
                          Clear
                        </Button>
                      </div>
                    </div>
                    
                    {/* Provider's tools */}
                    {isExpanded && (
                      <CommandGroup>
                        {provider.tools.map((tool) => {
                          const isSelected = localSelectedToolIds.includes(tool.id);
                          
                          return (
                            <CommandItem
                              key={tool.id}
                              value={`${providerId}:${tool.id}`}
                              onSelect={() => toggleTool(tool.id)}
                              className="pl-9"
                            >
                              <div className="flex flex-col gap-1 truncate">
                                <div className="flex items-center gap-2">
                                  <span className="h-4 w-4 flex items-center justify-center">
                                    {isSelected ? <Check className="h-4 w-4" /> : null}
                                  </span>
                                  <span className="font-medium">{tool.name}</span>
                                </div>
                                <div className="text-xs text-muted-foreground pl-6">
                                  {tool.description}
                                </div>
                              </div>
                            </CommandItem>
                          );
                        })}
                      </CommandGroup>
                    )}
                    
                    <CommandSeparator />
                  </div>
                );
              })
            )}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}