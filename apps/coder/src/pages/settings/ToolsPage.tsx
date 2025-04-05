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
        
        // Get MCP tools and clients
        let mcpTools = {};
        let clientInfoMap: Record<string, { id: string; name: string; tools?: string[] }> = {};
        
        try {
          const mcpClientsInfo = getMCPClients();
          const { allTools: mcpToolsList, clientTools, configs, clients } = mcpClientsInfo;
          
          console.log('[ToolsPage] MCP Clients: ', {
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
              
              console.log(`[ToolsPage] Client ${config.name} has ${clientTools[clientId].length} tools`);
            } else {
              console.log(`[ToolsPage] Client ${config.name} has no tools registered`);
            }
          });
        } catch (error) {
          console.error("Error fetching MCP tools:", error);
        }
        
        // Combine with built-in tools
        const combinedTools = extendWithMCPTools(mcpTools, clientInfoMap);
        
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
      // Update UI state optimistically
      if (enabledToolIds.includes(toolId)) {
        setEnabledToolIds(prev => prev.filter(id => id !== toolId));
      } else {
        setEnabledToolIds(prev => [...prev, toolId]);
      }
      
      // Call the repository method
      const result = await toggleToolEnabled(toolId);
      
      if (!result) {
        throw new Error("Failed to toggle tool");
      }
      
      // Successful update
      toast.success(`Tool ${enabledToolIds.includes(toolId) ? "enabled" : "disabled"}`, {
        description: `The ${toolId} tool has been ${enabledToolIds.includes(toolId) ? "enabled" : "disabled"}.`,
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

  // Function to refresh MCP tools
  const refreshMCPTools = async () => {
    try {
      setIsRefreshing(true);
      
      // Re-initalize MCP clients to fetch fresh tools
      try {
        // First reinitialize clients to ensure proper connection
        console.log('[ToolsPage] Reinitializing all MCP clients...');
        await reinitializeAllClients();
        console.log('[ToolsPage] All MCP clients reinitialized');
        
        // Then refresh tools
        console.log('[ToolsPage] Refreshing MCP tools...');
        const tools = await refreshTools();
        console.log('[ToolsPage] MCP tools refreshed, count:', Object.keys(tools).length);
        
        // Log current state
        const mcpClients = getMCPClients();
        console.log('[ToolsPage] Current MCP state:', {
          clients: Object.keys(mcpClients.clients),
          toolsCount: Object.keys(mcpClients.allTools).length,
          clientTools: mcpClients.clientTools
        });
      } catch (error) {
        console.error("Error refreshing MCP clients and tools:", error);
      }
      
      // Toggle refresh state to trigger re-fetch
      setIsRefreshing(false);
      
      toast.success("MCP tools refreshed", {
        description: "Successfully refreshed tools from MCP clients.",
        duration: 3000
      });
    } catch (error) {
      console.error("Error refreshing MCP tools:", error);
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