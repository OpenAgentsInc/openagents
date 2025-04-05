import React, { useEffect, useState } from "react";
import { useSettings, TOOLS, ToolDefinition, extendWithMCPTools } from "@openagents/core";
import { toast } from "sonner";
import { 
  Check, 
  CheckCircle2,
  Search, 
  Wrench,
  AlertCircle,
  ToggleLeft,
  ToggleRight
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
  TooltipContent
} from "@/components/ui";
import { getMCPClients } from "@/server/mcp-clients";

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

  // Load all tools (built-in and MCP)
  useEffect(() => {
    const loadTools = async () => {
      try {
        // Get MCP tools
        let mcpTools = {};
        try {
          const { allTools: mcpToolsList } = getMCPClients();
          mcpTools = mcpToolsList || {};
        } catch (error) {
          console.error("Error fetching MCP tools:", error);
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
  }, []);

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

  // Filter tools based on search query
  const filteredTools = allTools.filter(tool => {
    if (!filterQuery.trim()) return true;
    
    const query = filterQuery.toLowerCase();
    return (
      tool.id.toLowerCase().includes(query) ||
      tool.name.toLowerCase().includes(query) ||
      tool.description.toLowerCase().includes(query) ||
      tool.type.toLowerCase().includes(query)
    );
  });

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
          {/* Search and filter */}
          <div className="flex items-center space-x-2 mb-4">
            <Search className="h-4 w-4 text-muted-foreground" />
            <Input
              placeholder="Search tools..."
              value={filterQuery}
              onChange={(e) => setFilterQuery(e.target.value)}
              className="max-w-sm"
            />
          </div>
          
          {/* Tools Table */}
          <div className="rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-[50px]">Enabled</TableHead>
                  <TableHead className="w-[200px]">Tool</TableHead>
                  <TableHead className="w-[100px]">Type</TableHead>
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
                          {tool.type}
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
          
          {/* Action buttons */}
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
              Enable All
            </Button>
            
            <Button
              variant="outline"
              size="sm"
              onClick={async () => {
                try {
                  // Ensure shell_command is always enabled
                  const disableAll = ['shell_command'];
                  
                  // Update local state
                  setEnabledToolIds(disableAll);
                  
                  // Update settings to only include shell_command
                  for (const tool of allTools) {
                    if (tool.id !== 'shell_command') {
                      await disableTool(tool.id);
                    }
                  }
                  
                  toast.success("Most tools disabled", {
                    description: "All tools except the essential shell command tool have been disabled.",
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
              Disable All
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}