import React, { useEffect, useState, useMemo } from "react";
import { useSettings, TOOLS, ToolDefinition, extendWithMCPTools } from "@openagents/core";
import { toast } from "sonner";
import { 
  Check, 
  CheckCircle2,
  Search, 
  Wrench,
  AlertCircle,
  ToggleLeft,
  ToggleRight,
  ChevronDown,
  ChevronRight,
  CloudCog,
  Server
} from "lucide-react";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Input,
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  Tooltip,
  TooltipTrigger,
  TooltipContent,
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger
} from "@/components/ui";
import { getMCPClients, reinitializeAllClients, refreshTools } from "@/server/mcp-clients";

export default function ToolsPage() {
  const { 
    settings, 
    isLoading, 
    toggleToolEnabled,
    enableTool,
    disableTool,
    getEnabledToolIds
  } = useSettings();
  
  // Tool state
  const [allTools, setAllTools] = useState<ToolDefinition[]>([]);
  const [enabledToolIds, setEnabledToolIds] = useState<string[]>([]);
  const [filterQuery, setFilterQuery] = useState("");
  const [expandedProviders, setExpandedProviders] = useState<Record<string, boolean>>({});
  const [isRefreshing, setIsRefreshing] = useState(false);

  // Load all tools (built-in and MCP)
  useEffect(() => {
    const loadTools = async () => {
      try {
        // First, ensure MCP clients are initialized
        try {
          // Reinitialize all clients to ensure the latest tools
          console.log('[ToolsPage] Reinitializing MCP clients...');
          await reinitializeAllClients();
          console.log('[ToolsPage] MCP clients reinitialized successfully');
        } catch (initError) {
          console.error('[ToolsPage] Error initializing MCP clients:', initError);
        }
        
        // Get MCP tools from the server API endpoint instead of directly
        let mcpTools = {};
        let clientInfoMap: Record<string, { id: string; name: string; tools?: string[] }> = {};
        
        try {
          // First try to refresh tools on the server
          console.log('[ToolsPage] Refreshing MCP tools from server API...');
          try {
            const refreshResponse = await fetch('/api/mcp/tools/refresh', {
              method: 'POST',
            });
            
            if (refreshResponse.ok) {
              console.log('[ToolsPage] Server refresh successful');
              // Wait a moment for tools to be fully updated
              await new Promise(resolve => setTimeout(resolve, 1000));
            } else {
              console.warn('[ToolsPage] Tool refresh returned error:', await refreshResponse.text());
            }
          } catch (refreshError) {
            console.warn('[ToolsPage] Error refreshing tools:', refreshError);
          }
          
          // Now fetch the tools through the API endpoint
          console.log('[ToolsPage] Fetching MCP tools from server API...');
          const response = await fetch('/api/mcp/tools');
          if (!response.ok) {
            throw new Error(`Failed to fetch MCP tools: ${response.status} ${response.statusText}`);
          }
          
          // Parse the response to get tools and client info
          const data = await response.json();
          
          if (data.error) {
            throw new Error(`API returned error: ${data.error}`);
          }
          
          // Extract tools and client info
          mcpTools = data.tools || {};
          clientInfoMap = data.clientInfo || {};
          
          console.log('[ToolsPage] MCP tools fetched successfully:', {
            toolsCount: Object.keys(mcpTools).length,
            clientsCount: Object.keys(clientInfoMap).length,
            toolSample: Object.keys(mcpTools).slice(0, 5)
          });
          
          // Log client information
          Object.entries(clientInfoMap).forEach(([clientId, info]) => {
            console.log(`[ToolsPage] Client ${info.name} has ${info.tools?.length || 0} tools`);
            if (info.tools && info.tools.length > 0) {
              console.log(`[ToolsPage] Tools for ${info.name}: ${info.tools.slice(0, 5).join(', ')}${info.tools.length > 5 ? '...' : ''}`);
            } else {
              console.log(`[ToolsPage] Client ${info.name} has no tools registered`);
            }
          });
        } catch (error) {
          console.error("[ToolsPage] Error fetching MCP tools from API:", error);
        }
        
        // Combine with built-in tools
        const combinedTools = extendWithMCPTools(mcpTools, clientInfoMap);
        console.log(`[ToolsPage] Combined ${Object.keys(mcpTools).length} MCP tools with ${TOOLS.length} built-in tools = ${combinedTools.length} total tools`);
        
        // Initialize expanded state for each provider
        const providerIds = Array.from(
          new Set(combinedTools.filter(tool => tool.providerId).map(tool => tool.providerId as string))
        );
        const initialExpandState: Record<string, boolean> = {...expandedProviders};
        let changed = false;
        
        providerIds.forEach(id => {
          if (expandedProviders[id] === undefined) {
            initialExpandState[id] = true; // Default to expanded
            changed = true;
          }
        });
        
        // Only update state if there are changes
        if (changed) {
          setExpandedProviders(initialExpandState);
        }
        
        setAllTools(combinedTools);
      } catch (error) {
        console.error("Error loading tools:", error);
        // Fallback to built-in tools only
        setAllTools(TOOLS);
      }
    };
    
    loadTools();
  }, [isRefreshing]);

  // Load enabled tool IDs when settings change
  useEffect(() => {
    const fetchEnabledTools = async () => {
      try {
        const enabledIds = await getEnabledToolIds();
        setEnabledToolIds(enabledIds);
      } catch (error) {
        console.error("Error fetching enabled tool IDs:", error);
        // Default to shell_command if there's an error
        setEnabledToolIds(['shell_command']);
      }
    };
    
    fetchEnabledTools();
  }, [getEnabledToolIds, settings]);

  // Handle toggling a tool's enabled status
  const handleToggleTool = async (toolId: string) => {
    try {
      // Store the current state for the message
      const willBeEnabled = !enabledToolIds.includes(toolId);
      
      // Update UI state optimistically
      if (enabledToolIds.includes(toolId)) {
        setEnabledToolIds(prev => prev.filter(id => id !== toolId));
      } else {
        setEnabledToolIds(prev => [...prev, toolId]);
      }
      
      // Log the action for debugging
      console.log(`[ToolsPage] Toggling tool ${toolId} - Will be ${willBeEnabled ? 'enabled' : 'disabled'}`);
      
      // Call the appropriate repository method based on the action
      let result;
      if (willBeEnabled) {
        console.log(`[ToolsPage] Calling enableTool(${toolId})`);
        result = await enableTool(toolId);
      } else {
        console.log(`[ToolsPage] Calling disableTool(${toolId})`);
        result = await disableTool(toolId);
      }
      
      if (!result) {
        throw new Error(`Failed to ${willBeEnabled ? 'enable' : 'disable'} tool`);
      }
      
      // Refresh enabled tool IDs after update
      const updatedEnabledIds = await getEnabledToolIds();
      console.log(`[ToolsPage] After toggle, enabled tools:`, updatedEnabledIds);
      setEnabledToolIds(updatedEnabledIds);
      
      // Successful update
      toast.success(`Tool ${willBeEnabled ? "enabled" : "disabled"}`, {
        description: `The ${toolId} tool has been ${willBeEnabled ? "enabled" : "disabled"}.`,
        duration: 3000
      });
    } catch (error) {
      console.error(`Error toggling tool ${toolId}:`, error);
      
      // Revert optimistic update on error
      const enabledIds = await getEnabledToolIds();
      setEnabledToolIds(enabledIds);
      
      toast.error("Failed to update tool settings", {
        description: "There was a problem updating the tool configuration. Please try again.",
        duration: 4000
      });
    }
  };

  // Function to refresh MCP tools using the server API
  const refreshMCPTools = async () => {
    try {
      setIsRefreshing(true);
      
      console.log('[ToolsPage] Refreshing MCP tools via API...');
      
      // Call the refresh API endpoint
      try {
        const refreshResponse = await fetch('/api/mcp/tools/refresh', {
          method: 'POST',
        });
        
        if (!refreshResponse.ok) {
          const errorText = await refreshResponse.text();
          console.error('[ToolsPage] Server error response:', errorText);
          throw new Error(`Failed to refresh MCP tools: ${refreshResponse.status} ${refreshResponse.statusText}`);
        }
        
        // Parse the response to see what tools are available
        const data = await refreshResponse.json();
        console.log('[ToolsPage] MCP tools refresh response:', data);
        
        if (data.tools && Array.isArray(data.tools)) {
          console.log(`[ToolsPage] Available tools after refresh: ${data.tools.length > 10 ? 
            data.tools.slice(0, 10).join(', ') + '...' : 
            data.tools.join(', ')}`);
        }
        
        // Toggle refresh state to trigger re-fetch of tools
        setIsRefreshing(prev => !prev);
        
        toast.success("MCP tools refreshed", {
          description: `Successfully refreshed ${data.toolCount || 0} tools from MCP clients.`,
          duration: 3000
        });
      } catch (error) {
        console.error("[ToolsPage] Error refreshing MCP tools:", error);
        toast.error("Failed to refresh tools", {
          description: error instanceof Error ? error.message : "There was a problem refreshing MCP tools.",
          duration: 4000
        });
      } finally {
        setIsRefreshing(false);
      }
    } catch (error) {
      console.error("Error in refreshMCPTools:", error);
      setIsRefreshing(false);
      
      toast.error("Failed to refresh tools", {
        description: "There was a problem refreshing MCP tools. Please try again.",
        duration: 4000
      });
    }
  };
  
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
    allTools.forEach(tool => {
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
  }, [allTools]);
  
  // Filter tools based on search query
  const filteredTools = useMemo(() => {
    if (!filterQuery.trim()) {
      return allTools;
    }
    
    const query = filterQuery.toLowerCase();
    return allTools.filter(tool => 
      tool.id.toLowerCase().includes(query) ||
      tool.name.toLowerCase().includes(query) ||
      tool.description.toLowerCase().includes(query) ||
      tool.type.toLowerCase().includes(query) ||
      (tool.providerName && tool.providerName.toLowerCase().includes(query))
    );
  }, [allTools, filterQuery]);

  // Helper to toggle all tools in a provider group
  const toggleProviderTools = async (providerId: string, enabled: boolean) => {
    try {
      const providerTools = providerGroups[providerId].tools;
      
      // Get the IDs of all tools in this provider
      const toolIds = providerTools.map(tool => tool.id);
      
      // Update each tool's enabled status
      for (const toolId of toolIds) {
        if (enabled) {
          await enableTool(toolId);
        } else {
          await disableTool(toolId);
        }
      }
      
      // Refresh enabled tool IDs
      const updatedEnabledTools = await getEnabledToolIds();
      setEnabledToolIds(updatedEnabledTools);
      
      toast.success(`${providerGroups[providerId].name} tools ${enabled ? 'enabled' : 'disabled'}`, {
        description: `All tools in ${providerGroups[providerId].name} have been ${enabled ? 'enabled' : 'disabled'}.`,
        duration: 3000
      });
    } catch (error) {
      console.error(`Failed to ${enabled ? 'enable' : 'disable'} provider tools:`, error);
      toast.error(`Failed to ${enabled ? 'enable' : 'disable'} tools`, {
        description: "There was a problem updating tool settings. Please try again.",
        duration: 4000
      });
    }
  };

  // Toggle a provider's expanded state
  const toggleProviderExpanded = (providerId: string) => {
    setExpandedProviders(prev => ({
      ...prev,
      [providerId]: !prev[providerId]
    }));
  };
  
  // Check if all tools in a provider are enabled
  const areAllProviderToolsEnabled = (providerId: string) => {
    const tools = providerGroups[providerId]?.tools || [];
    return tools.length > 0 && tools.every(tool => enabledToolIds.includes(tool.id));
  };
  
  // Check if any tools in a provider are enabled
  const areSomeProviderToolsEnabled = (providerId: string) => {
    const tools = providerGroups[providerId]?.tools || [];
    return tools.some(tool => enabledToolIds.includes(tool.id));
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full font-mono">
        <p>Loading tools...</p>
      </div>
    );
  }

  return (
    <Card className="font-mono">
      <CardHeader>
        <CardTitle>Tool Configuration</CardTitle>
        <CardDescription>
          Enable or disable AI tools. Disabled tools will not be available to any model.
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-4">
          {/* Search and control bar */}
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center space-x-2">
              <Search className="h-4 w-4 text-muted-foreground" />
              <Input
                placeholder="Search tools..."
                value={filterQuery}
                onChange={(e) => setFilterQuery(e.target.value)}
                className="max-w-sm"
              />
            </div>
            
            <Button
              variant="outline"
              size="sm"
              onClick={refreshMCPTools}
              disabled={isRefreshing}
              className="gap-2"
            >
              <CloudCog className="h-4 w-4" />
              {isRefreshing ? "Refreshing..." : "Refresh MCP Tools"}
            </Button>
          </div>
          
          {/* Filter indicator */}
          {filterQuery.trim() && (
            <div className="flex items-center gap-2 text-sm text-muted-foreground mb-2">
              <AlertCircle className="h-4 w-4" />
              <span>
                Showing filtered results. {filteredTools.length} tools match your search.
              </span>
              <Button
                variant="ghost" 
                size="sm"
                onClick={() => setFilterQuery("")}
                className="h-6 px-2 text-xs"
              >
                Clear filter
              </Button>
            </div>
          )}
          
          {/* Tools by Provider Groups */}
          {filterQuery.trim() ? (
            // Show flat list when filtering
            <div className="rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead className="w-[50px]">Enabled</TableHead>
                    <TableHead className="w-[200px]">Tool</TableHead>
                    <TableHead className="w-[100px]">Provider</TableHead>
                    <TableHead className="hidden md:table-cell">Description</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {filteredTools.map((tool) => {
                    const isEnabled = enabledToolIds.includes(tool.id);
                    
                    return (
                      <TableRow key={tool.id}>
                        <TableCell>
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => handleToggleTool(tool.id)}
                            title={isEnabled ? "Disable this tool" : "Enable this tool"}
                          >
                            {isEnabled ? (
                              <ToggleRight className="h-5 w-5 text-green-500" />
                            ) : (
                              <ToggleLeft className="h-5 w-5 text-muted-foreground" />
                            )}
                          </Button>
                        </TableCell>
                        <TableCell>
                          <div className="font-medium flex items-center gap-2">
                            <Wrench className="h-4 w-4 text-muted-foreground" />
                            {tool.name}
                          </div>
                          <div className="text-xs text-muted-foreground font-mono">
                            {tool.id}
                          </div>
                        </TableCell>
                        <TableCell>
                          <span className={`px-2 py-1 rounded-md text-xs ${
                            tool.type === 'builtin' 
                              ? 'bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200' 
                              : 'bg-purple-100 text-purple-800 dark:bg-purple-900 dark:text-purple-200'
                          }`}>
                            {tool.providerName || tool.type}
                          </span>
                        </TableCell>
                        <TableCell className="hidden md:table-cell">
                          {tool.description}
                        </TableCell>
                      </TableRow>
                    );
                  })}
                  {filteredTools.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={4} className="text-center py-6">
                        <div className="flex flex-col items-center justify-center text-muted-foreground">
                          <AlertCircle className="h-6 w-6 mb-2" />
                          <p>No tools match your search query.</p>
                        </div>
                      </TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>
            </div>
          ) : (
            // Show grouped list when not filtering
            <div className="space-y-4">
              {Object.entries(providerGroups).map(([providerId, provider]) => {
                if (provider.tools.length === 0) return null;
                
                const allEnabled = areAllProviderToolsEnabled(providerId);
                const someEnabled = areSomeProviderToolsEnabled(providerId);
                const isExpanded = expandedProviders[providerId];
                
                return (
                  <div key={providerId} className="rounded-md border">
                    {/* Provider header */}
                    <div 
                      className={`flex items-center justify-between p-3 bg-muted/20 border-b cursor-pointer`}
                      onClick={() => toggleProviderExpanded(providerId)}
                    >
                      <div className="flex items-center gap-2">
                        {isExpanded ? (
                          <ChevronDown className="h-4 w-4 text-muted-foreground" />
                        ) : (
                          <ChevronRight className="h-4 w-4 text-muted-foreground" />
                        )}
                        <span className="font-medium flex items-center gap-2">
                          {provider.id === 'builtin' ? (
                            <Server className="h-4 w-4 text-blue-500" />
                          ) : (
                            <CloudCog className="h-4 w-4 text-purple-500" />
                          )}
                          {provider.name}
                        </span>
                        <span className="text-xs text-muted-foreground">
                          ({provider.tools.length} tools)
                        </span>
                        
                        {someEnabled && (
                          <span className={`ml-2 px-2 py-1 rounded-md text-xs ${
                            allEnabled 
                              ? 'bg-green-100 text-green-800 dark:bg-green-900 dark:text-green-200'
                              : 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900 dark:text-yellow-200'
                          }`}>
                            {allEnabled ? 'All Enabled' : 'Some Enabled'}
                          </span>
                        )}
                      </div>
                      
                      {/* Provider-level actions */}
                      <div 
                        className="flex items-center gap-2" 
                        onClick={(e) => e.stopPropagation()}
                      >
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 px-2 text-xs"
                          onClick={() => toggleProviderTools(providerId, true)}
                          disabled={allEnabled}
                        >
                          Enable All
                        </Button>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-7 px-2 text-xs"
                          onClick={() => toggleProviderTools(providerId, false)}
                          disabled={!someEnabled}
                        >
                          Disable All
                        </Button>
                      </div>
                    </div>
                    
                    {/* Provider's tools */}
                    {isExpanded && (
                      <Table>
                        <TableHeader>
                          <TableRow>
                            <TableHead className="w-[50px]">Enabled</TableHead>
                            <TableHead className="w-[200px]">Tool</TableHead>
                            <TableHead className="hidden md:table-cell">Description</TableHead>
                          </TableRow>
                        </TableHeader>
                        <TableBody>
                          {provider.tools.map((tool) => {
                            const isEnabled = enabledToolIds.includes(tool.id);
                            
                            return (
                              <TableRow key={tool.id}>
                                <TableCell>
                                  <Button
                                    variant="ghost"
                                    size="sm"
                                    onClick={() => handleToggleTool(tool.id)}
                                    title={isEnabled ? "Disable this tool" : "Enable this tool"}
                                  >
                                    {isEnabled ? (
                                      <ToggleRight className="h-5 w-5 text-green-500" />
                                    ) : (
                                      <ToggleLeft className="h-5 w-5 text-muted-foreground" />
                                    )}
                                  </Button>
                                </TableCell>
                                <TableCell>
                                  <div className="font-medium flex items-center gap-2">
                                    <Wrench className="h-4 w-4 text-muted-foreground" />
                                    {tool.name}
                                  </div>
                                  <div className="text-xs text-muted-foreground font-mono">
                                    {tool.id}
                                  </div>
                                </TableCell>
                                <TableCell className="hidden md:table-cell">
                                  {tool.description}
                                </TableCell>
                              </TableRow>
                            );
                          })}
                        </TableBody>
                      </Table>
                    )}
                  </div>
                );
              })}
            </div>
          )}
          
          {/* Global action buttons */}
          <div className="flex justify-end space-x-3">
            <Button
              variant="outline"
              size="sm"
              onClick={async () => {
                try {
                  // Enable all tools
                  const enableAll = allTools.map(tool => tool.id);
                  
                  // Update local state
                  setEnabledToolIds(enableAll);
                  
                  // Update each tool individually
                  for (const toolId of enableAll) {
                    await enableTool(toolId);
                  }
                  
                  toast.success("All tools enabled", {
                    description: "All available tools have been enabled.",
                    duration: 3000
                  });
                } catch (error) {
                  console.error("Failed to enable all tools:", error);
                  toast.error("Failed to enable all tools", {
                    description: "There was a problem updating tool settings. Please try again.",
                    duration: 4000
                  });
                }
              }}
            >
              Enable All Tools
            </Button>
            
            <Button
              variant="outline"
              size="sm"
              onClick={async () => {
                try {
                  // Disable all tools (even shell_command)
                  const disableAll: string[] = [];
                  
                  // Update local state
                  setEnabledToolIds(disableAll);
                  
                  // Update settings to disable all tools
                  for (const tool of allTools) {
                    await disableTool(tool.id);
                  }
                  
                  toast.success("All tools disabled", {
                    description: "All tools have been disabled.",
                    duration: 3000
                  });
                } catch (error) {
                  console.error("Failed to disable tools:", error);
                  toast.error("Failed to disable tools", {
                    description: "There was a problem updating tool settings. Please try again.",
                    duration: 4000
                  });
                }
              }}
            >
              Disable All Tools
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}