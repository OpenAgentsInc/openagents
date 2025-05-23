import React, { useState, useEffect, useMemo, useCallback } from "react";
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
  Server,
  RefreshCw
} from "lucide-react";
import { cn } from "@/utils/tailwind";
import {
  Button,
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
  // Component state
  const [open, setOpen] = useState(false);
  const [allTools, setAllTools] = useState<ToolDefinition[]>([]);
  const { getEnabledToolIds } = useSettings();
  const [enabledToolIds, setEnabledToolIds] = useState<string[]>([]);
  const [refreshing, setRefreshing] = useState(false);
  const [loading, setLoading] = useState(true); // Add loading state for initial fetch
  const [lastRefreshed, setLastRefreshed] = useState<Date | null>(null);

  // CRITICAL: Use a ref to store selected tools to ensure UI updates properly
  const [internalSelection, setInternalSelection] = useState<string[]>([]);
  // Also maintain a separate copy of the selection for rendering
  const [forceRender, setForceRender] = useState<number>(0);

  // Track expanded provider groups
  const [expandedProviders, setExpandedProviders] = useState<Record<string, boolean>>({});

  // Initialize with props on mount and when props change
  useEffect(() => {
    console.log('[ToolSelect] Setting selection from props:', selectedToolIds);
    setInternalSelection(selectedToolIds || []);
    // Force a rerender to ensure UI is updated
    setForceRender(prev => prev + 1);
  }, [selectedToolIds]);

  // Load all tools (built-in and MCP)
  const loadTools = useCallback(async () => {
    try {
      // Set loading state
      setLoading(true);

      // Get globally enabled tool IDs
      const enabledIds = await getEnabledToolIds();
      setEnabledToolIds(enabledIds);

      // First try to trigger a refresh to ensure we have the latest tools
      try {
        console.log('[ToolSelect] Refreshing tools before fetch...');
        const refreshResponse = await fetch('/api/mcp/tools/refresh', {
          method: 'POST',
        });

        if (refreshResponse.ok) {
          console.log('[ToolSelect] Refresh successful, proceeding to fetch updated tools');
          // Wait a moment for the tools to be fully refreshed
          await new Promise(resolve => setTimeout(resolve, 500));
        } else {
          const errorText = await refreshResponse.text();
          console.warn('[ToolSelect] Tool refresh not successful, error:', errorText);
          console.warn('[ToolSelect] Will try to fetch existing tools instead');
        }
      } catch (refreshError) {
        console.warn('[ToolSelect] Error refreshing tools:', refreshError);
      }

      // Get MCP tools from the server via API
      let mcpTools = {};
      let clientInfoMap: Record<string, { id: string; name: string; tools?: string[] }> = {};

      try {
        console.log('[ToolSelect] Fetching MCP tools from server API...');

        // Call the API endpoint we created
        const response = await fetch('/api/mcp/tools');
        if (!response.ok) {
          throw new Error(`Failed to fetch MCP tools: ${response.status} ${response.statusText}`);
        }

        // Parse the response
        const data = await response.json();
        if (data.error) {
          throw new Error(`API returned error: ${data.error}`);
        }

        // Extract tools and client info
        mcpTools = data.tools || {};
        clientInfoMap = data.clientInfo || {};

        console.log('[ToolSelect] MCP tools fetched successfully:', {
          toolsCount: Object.keys(mcpTools).length,
          clientsCount: Object.keys(clientInfoMap).length,
          toolSample: Object.keys(mcpTools).slice(0, 5)
        });

        // Log detailed client info
        Object.entries(clientInfoMap).forEach(([clientId, info]) => {
          console.log(`[ToolSelect] Client ${info.name} has ${info.tools?.length || 0} tools`);
          if (info.tools) {
            info.tools.forEach(toolId => {
              console.log(`[ToolSelect] - Tool: ${toolId}`);
            });
          }
        });
      } catch (error) {
        console.error("[ToolSelect] Error fetching MCP tools from API:", error);
      }

      // Combine with built-in tools
      const combinedTools = extendWithMCPTools(mcpTools, clientInfoMap);
      console.log(`[ToolSelect] Combined tools: ${combinedTools.length} tools (${Object.keys(mcpTools).length} MCP tools + ${TOOLS.length} built-in tools)`);
      setAllTools(combinedTools);
      setLastRefreshed(new Date());
      setLoading(false); // Set loading to false after tools are fetched
    } catch (error) {
      console.error("Error loading tools:", error);
      // Fallback to built-in tools only
      setAllTools(TOOLS);
      setLoading(false); // Set loading to false even on error
    }
  }, [getEnabledToolIds]);

  // Run loadTools on component mount
  useEffect(() => {
    loadTools();
  }, [loadTools]);

  // Function to refresh MCP tools from the server
  const refreshMCPTools = useCallback(async () => {
    try {
      setRefreshing(true);
      setLoading(true); // Also set loading state
      console.log('[ToolSelect] Manually refreshing MCP tools...');

      // Call the refresh API endpoint
      const response = await fetch('/api/mcp/tools/refresh', {
        method: 'POST',
      });

      if (!response.ok) {
        const errorText = await response.text();
        console.error('[ToolSelect] Server error response:', errorText);

        // Try to parse the error for more details
        let errorDetails = '';
        try {
          const errorJson = JSON.parse(errorText);
          if (errorJson.details) {
            errorDetails = `: ${errorJson.details}`;
          } else if (errorJson.error) {
            errorDetails = `: ${errorJson.error}`;
          }
        } catch (e) {
          // If it's not JSON, use the raw text
          if (errorText) {
            errorDetails = `: ${errorText}`;
          }
        }

        throw new Error(`Failed to refresh MCP tools: ${response.status} ${response.statusText}${errorDetails}`);
      }

      // Parse the response to see what tools are available
      const data = await response.json();
      console.log('[ToolSelect] MCP tools refresh response:', data);

      if (data.tools && Array.isArray(data.tools)) {
        console.log(`[ToolSelect] Available tools after refresh: ${data.tools.join(', ')}`);
      }

      // Wait a moment to ensure the server has fully processed all tools
      await new Promise(resolve => setTimeout(resolve, 1000));

      // Load the updated tools
      await loadTools();

      // We don't need to set lastRefreshed here since loadTools does it
      console.log('[ToolSelect] MCP tools refreshed successfully');

      // Show success message - with a more concise summary if there are lots of tools
      const toolMessage = data.toolCount > 5
        ? `${data.tools?.slice(0, 5).join(', ')}... and ${data.toolCount - 5} more`
        : data.tools?.join(', ') || 'none';

      alert(`MCP tools refreshed successfully. Found ${data.toolCount} tools: ${toolMessage}`);
    } catch (error) {
      console.error('[ToolSelect] Error refreshing MCP tools:', error);
      alert(`Error refreshing MCP tools: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setRefreshing(false);
    }
  }, [loadTools]);

  // Filter to only show globally enabled tools
  const availableTools = useMemo(() => {
    // Debug logging
    console.log('[ToolSelect] Filtering tools:');
    console.log('  - All tools:', allTools.map(t => ({ id: t.id, provider: t.providerName || t.type })));
    console.log('  - Enabled tool IDs:', enabledToolIds);

    // Restore proper filtering but with detailed logging
    const filtered = allTools.filter(tool => {
      const isEnabled = enabledToolIds.includes(tool.id);
      if (!isEnabled) {
        console.log(`[ToolSelect] Tool not enabled: ${tool.id} (${tool.providerName || tool.type})`);
      }
      return isEnabled;
    });

    console.log('  - Filtered tools:', filtered.map(t => ({ id: t.id, provider: t.providerName || t.type })));

    return filtered;
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

  // Toggle a tool's selection - enhanced with super reliable state handling
  const toggleTool = (toolId: string) => {
    console.log(`[ToolSelect] Toggling tool selection: ${toolId}`);

    // Use a function to ensure we're working with the most up-to-date state
    setInternalSelection(currentSelection => {
      // Check if already selected
      const isSelected = currentSelection.includes(toolId);
      console.log(`[ToolSelect] Tool ${toolId} is currently: ${isSelected ? 'SELECTED' : 'NOT SELECTED'}`);

      // Create a new array (never mutate the old one)
      let newSelection: string[];

      if (isSelected) {
        // Remove if selected
        newSelection = currentSelection.filter(id => id !== toolId);
        console.log(`[ToolSelect] REMOVING tool from selection`);
      } else {
        // Add if not selected
        newSelection = [...currentSelection, toolId];
        console.log(`[ToolSelect] ADDING tool to selection`);
      }

      console.log(`[ToolSelect] New selection will be:`, newSelection);

      // Schedule calling the onChange callback after state update
      setTimeout(() => {
        console.log(`[ToolSelect] Notifying parent of selection change`, newSelection);
        onChange(newSelection);
      }, 0);

      // Return the new selection
      return newSelection;
    });

    // Explicitly tell React to rerender
    setForceRender(prev => prev + 1);

    // Force a DOM update to be doubly sure
    setTimeout(() => {
      console.log('[ToolSelect] After update, verified selection is:', internalSelection);
    }, 50);
  };

  // Select all available tools
  const selectAllTools = () => {
    const allToolIds = availableTools.map(tool => tool.id);
    console.log('[ToolSelect] Selecting ALL tools:', allToolIds);
    setInternalSelection(allToolIds);
    setForceRender(prev => prev + 1);
    onChange(allToolIds);
  };

  // Clear all selected tools
  const clearSelection = () => {
    console.log('[ToolSelect] Clearing ALL tools');
    setInternalSelection([]);
    setForceRender(prev => prev + 1);
    onChange([]);
  };

  // Check if all available tools are selected
  const allSelected = useMemo(() => {
    return availableTools.length > 0 &&
      availableTools.every(tool => internalSelection.includes(tool.id));
  }, [availableTools, internalSelection, forceRender]);

  // Update internal state when props change
  useEffect(() => {
    // Only update if props actually changed
    const propsSelectionSet = new Set(selectedToolIds);
    const currentSelectionSet = new Set(internalSelection);

    // Check if selection has changed
    let changed = false;

    // Different lengths = definitely changed
    if (propsSelectionSet.size !== currentSelectionSet.size) {
      changed = true;
    } else {
      // Same length, check if every item in props is in current
      for (const id of propsSelectionSet) {
        if (!currentSelectionSet.has(id)) {
          changed = true;
          break;
        }
      }
    }

    if (changed) {
      console.log('[ToolSelect] Props selection changed, updating:', selectedToolIds);
      setInternalSelection(selectedToolIds || []);
      setForceRender(prev => prev + 1);
    }
  }, [selectedToolIds]);

  // Get display text for button
  const displayText = useMemo(() => {
    if (internalSelection.length === 0) {
      return placeholder;
    } else if (internalSelection.length === 1) {
      const tool = allTools.find(t => t.id === internalSelection[0]);
      return tool ? tool.name : internalSelection[0];
    } else if (allSelected) {
      return "";
    } else {
      return `${internalSelection.length} tools `;
    }
  }, [internalSelection, allTools, placeholder, allSelected, forceRender]);

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
      ...internalSelection,
      ...providerToolIds
    ])];

    setInternalSelection(newSelection);
    setForceRender(prev => prev + 1);
    onChange(newSelection);
  };

  // Deselect all tools in a provider
  const deselectProviderTools = (providerId: string) => {
    const providerTools = providerGroups[providerId]?.tools || [];
    const providerToolIds = providerTools.map(tool => tool.id);

    // Remove all provider tools from selection
    const newSelection = internalSelection.filter(
      id => !providerToolIds.includes(id)
    );

    setInternalSelection(newSelection);
    setForceRender(prev => prev + 1);
    onChange(newSelection);
  };

  // Check if all tools in a provider are selected
  const areAllProviderToolsSelected = (providerId: string) => {
    const providerTools = providerGroups[providerId]?.tools || [];
    return providerTools.length > 0 &&
      providerTools.every(tool => internalSelection.includes(tool.id));
  };

  // Check if any tools in a provider are selected
  const areSomeProviderToolsSelected = (providerId: string) => {
    const providerTools = providerGroups[providerId]?.tools || [];
    return providerTools.some(tool => internalSelection.includes(tool.id));
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <div className="flex w-full">
          <Button
            variant="outline"
            role="combobox"
            aria-expanded={open}
            className={cn(
              "flex-1 justify-between overflow-hidden text-ellipsis whitespace-nowrap rounded-r-none",
              className
            )}
            disabled={disabled}
          >
            <div className="flex items-center overflow-hidden">
              <Wrench className="mr-2 h-4 w-4" />
              <span className="overflow-hidden text-ellipsis">
                {displayText}
              </span>
              {internalSelection.length > 0 && (
                <Badge variant="secondary" className="ml-2">
                  {internalSelection.length}
                </Badge>
              )}
            </div>
            <ChevronDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
          </Button>
          <Button
            variant="outline"
            size="icon"
            className="rounded-l-none border-l-0"
            onClick={(e) => {
              e.preventDefault();
              e.stopPropagation();
              refreshMCPTools();
            }}
            disabled={refreshing || disabled}
            title="Refresh MCP Tools"
          >
            <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
          </Button>
        </div>
      </PopoverTrigger>
      <PopoverContent className="w-[350px] p-0 font-mono" align="start">
        <div className="font-mono">
          {/* Action buttons */}
          <div className="flex items-center justify-between p-2 border-b font-mono">
            <Button
              variant="ghost"
              size="sm"
              className="text-xs font-mono"
              onClick={selectAllTools}
              disabled={allSelected || availableTools.length === 0}
            >
              Select all
            </Button>
            <Button
              variant="ghost"
              size="sm"
              className="text-xs font-mono"
              onClick={clearSelection}
              disabled={internalSelection.length === 0}
            >
              Clear selection
            </Button>
          </div>

          <div className="max-h-[350px] overflow-auto font-mono">
            {loading ? (
              <div className="flex flex-col items-center justify-center p-4 text-sm text-muted-foreground font-mono">
                <RefreshCw className="h-6 w-6 mb-2 animate-spin" />
                <p>Loading tools...</p>
              </div>
            ) : allTools.length > 0 && availableTools.length === 0 ? (
              <div className="flex flex-col items-center justify-center p-4 text-sm text-muted-foreground font-mono">
                <AlertCircle className="h-6 w-6 mb-2" />
                <p>No tools are enabled globally.</p>
                <p className="text-xs mt-1">Enable tools in Settings → Tools.</p>
                <Button
                  className="mt-4"
                  size="sm"
                  onClick={refreshMCPTools}
                  disabled={refreshing}
                >
                  <RefreshCw className={`h-3 w-3 mr-2 ${refreshing ? 'animate-spin' : ''}`} />
                  Refresh MCP Tools
                </Button>
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
                      className="flex items-center justify-between px-2 py-1.5 text-sm font-mono cursor-pointer hover:bg-muted/50"
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
                              internalSelection.includes(t.id)).length}/${provider.tools.length}`}
                          </Badge>
                        )}
                      </div>

                      {/* Provider actions */}
                      <div className="flex items-center gap-1" onClick={e => e.stopPropagation()}>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-2 text-[10px] font-mono"
                          onClick={() => selectProviderTools(providerId)}
                          disabled={allSelected}
                        >
                          Select
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-2 text-[10px] font-mono"
                          onClick={() => deselectProviderTools(providerId)}
                          disabled={!someSelected}
                        >
                          Clear
                        </Button>
                      </div>
                    </div>

                    {/* Provider's tools */}
                    {isExpanded && (
                      <div className="py-1">
                        {provider.tools.map((tool) => {
                          const isSelected = internalSelection.includes(tool.id);

                          return (
                            <div
                              key={tool.id}
                              className="px-2 py-1.5 text-sm font-mono cursor-pointer hover:bg-muted/50 pl-9"
                              onClick={(e) => {
                                e.preventDefault();
                                e.stopPropagation();
                                toggleTool(tool.id);
                              }}
                            >
                              <div className="flex flex-col gap-1 truncate">
                                <div className="flex items-center gap-2">
                                  <div
                                    className="h-4 w-4 flex items-center justify-center border border-muted-foreground/70 rounded-sm"
                                    style={{
                                      backgroundColor: isSelected ? 'var(--primary)' : 'transparent',
                                      transition: 'all 0.1s ease-in-out',
                                      boxShadow: isSelected ? '0 0 0 1px rgba(var(--primary-rgb), 0.3)' : 'none'
                                    }}
                                  >
                                    {isSelected && <Check className="h-3 w-3 text-white" />}
                                  </div>
                                  <span className="font-mono">{tool.name}</span>
                                </div>
                                <div className="text-xs text-muted-foreground pl-6 font-mono">
                                  {tool.description}
                                </div>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}

                    <hr className="my-1" />
                  </div>
                );
              })
            )}
          </div>

          {/* Status footer */}
          <div className="p-2 border-t flex items-center justify-between text-xs text-muted-foreground">
            <div className="flex items-center">
              {lastRefreshed ? (
                <span>Last refreshed: {lastRefreshed.toLocaleTimeString()}</span>
              ) : (
                <span>Tool status: {refreshing ? 'Refreshing...' : 'Idle'}</span>
              )}
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 px-2"
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                refreshMCPTools();
              }}
              disabled={refreshing}
            >
              <RefreshCw className={`h-3 w-3 mr-1 ${refreshing ? 'animate-spin' : ''}`} />
              Refresh
            </Button>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}
