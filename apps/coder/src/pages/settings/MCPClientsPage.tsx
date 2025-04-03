import React, { useState, useEffect, useCallback } from "react";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
  Button,
  Input,
  Switch,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Label,
  Alert,
  AlertTitle,
  AlertDescription,
  Badge,
  Separator,
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
  Tabs,
  TabsList,
  TabsTrigger,
  TabsContent,
} from "@/components/ui";
import { Toaster, toast as sonnerToast } from "sonner";
import { 
  Plus, 
  Trash, 
  Edit, 
  RefreshCw, 
  CheckCircle, 
  XCircle, 
  AlertTriangle, 
  Terminal, 
  Globe, 
  Server,
  TerminalSquare 
} from "lucide-react";
import { MCPClientConfig } from "@openagents/core/src/db/types";

// Import the necessary functions from the MCP clients module
const MCP_CLIENT_API_PATH = "/api/mcp";

interface MCPClientFormData extends Omit<MCPClientConfig, 'id' | 'status' | 'lastConnected' | 'statusMessage'> {
  id?: string;
}

export default function MCPClientsPage() {
  const [clients, setClients] = useState<MCPClientConfig[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [openDialog, setOpenDialog] = useState(false);
  const [currentClient, setCurrentClient] = useState<MCPClientFormData | null>(null);
  const [isEditMode, setIsEditMode] = useState(false);
  const [activeTab, setActiveTab] = useState<string>("sse");
  
  // Form data with validation states
  const [formData, setFormData] = useState<MCPClientFormData>({
    name: '',
    enabled: true,
    type: 'sse',
    url: '',
    command: '',
    args: [],
    env: {}
  });
  
  const [formErrors, setFormErrors] = useState<Record<string, string>>({});

  // Load MCP clients from the server
  const loadClients = useCallback(async () => {
    try {
      setLoading(true);
      const response = await fetch(`${MCP_CLIENT_API_PATH}/clients`);
      if (response.ok) {
        const data = await response.json();
        setClients(data.clients || []);
      } else {
        sonnerToast("Failed to load MCP clients", {
          description: "Check the console for more details"
        });
        console.error("Failed to load MCP clients:", await response.text());
      }
    } catch (error) {
      console.error("Error loading MCP clients:", error);
      sonnerToast("Error loading MCP clients", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    } finally {
      setLoading(false);
    }
  }, []);

  // Load clients on component mount
  useEffect(() => {
    loadClients();
  }, [loadClients]);

  // Refresh a specific client
  const refreshClient = async (id: string) => {
    try {
      setRefreshing(true);
      const response = await fetch(`${MCP_CLIENT_API_PATH}/clients/${id}/refresh`, {
        method: 'POST',
      });
      
      if (response.ok) {
        sonnerToast("Client refreshed", {
          description: "MCP client has been refreshed successfully"
        });
        await loadClients(); // Reload clients to get updated status
      } else {
        sonnerToast("Failed to refresh client", {
          description: "Check the console for more details"
        });
        console.error("Failed to refresh client:", await response.text());
      }
    } catch (error) {
      console.error("Error refreshing client:", error);
      sonnerToast("Error refreshing client", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    } finally {
      setRefreshing(false);
    }
  };

  // Refresh all clients
  const refreshAllClients = async () => {
    try {
      setRefreshing(true);
      const response = await fetch(`${MCP_CLIENT_API_PATH}/refresh`, {
        method: 'POST',
      });
      
      if (response.ok) {
        sonnerToast("All clients refreshed", {
          description: "All MCP clients have been refreshed successfully"
        });
        await loadClients(); // Reload clients to get updated status
      } else {
        sonnerToast("Failed to refresh clients", {
          description: "Check the console for more details"
        });
        console.error("Failed to refresh clients:", await response.text());
      }
    } catch (error) {
      console.error("Error refreshing clients:", error);
      sonnerToast("Error refreshing clients", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    } finally {
      setRefreshing(false);
    }
  };

  // Toggle client enabled state
  const toggleClientEnabled = async (id: string, enabled: boolean) => {
    try {
      const response = await fetch(`${MCP_CLIENT_API_PATH}/clients/${id}`, {
        method: 'PATCH',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ enabled }),
      });
      
      if (response.ok) {
        sonnerToast(`Client ${enabled ? 'enabled' : 'disabled'}`, {
          description: `MCP client has been ${enabled ? 'enabled' : 'disabled'} successfully`
        });
        await loadClients(); // Reload clients to get updated status
      } else {
        sonnerToast(`Failed to ${enabled ? 'enable' : 'disable'} client`, {
          description: "Check the console for more details"
        });
        console.error(`Failed to ${enabled ? 'enable' : 'disable'} client:`, await response.text());
      }
    } catch (error) {
      console.error(`Error ${enabled ? 'enabling' : 'disabling'} client:`, error);
      sonnerToast(`Error ${enabled ? 'enabling' : 'disabling'} client`, {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    }
  };

  // Delete a client
  const deleteClient = async (id: string) => {
    if (!confirm("Are you sure you want to delete this MCP client configuration?")) {
      return;
    }
    
    try {
      const response = await fetch(`${MCP_CLIENT_API_PATH}/clients/${id}`, {
        method: 'DELETE',
      });
      
      if (response.ok) {
        sonnerToast("Client deleted", {
          description: "MCP client has been deleted successfully"
        });
        await loadClients(); // Reload clients to get updated list
      } else {
        sonnerToast("Failed to delete client", {
          description: "Check the console for more details"
        });
        console.error("Failed to delete client:", await response.text());
      }
    } catch (error) {
      console.error("Error deleting client:", error);
      sonnerToast("Error deleting client", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    }
  };

  // Open dialog to create a new client
  const openNewClientDialog = () => {
    setIsEditMode(false);
    setCurrentClient(null);
    setFormData({
      name: '',
      enabled: true,
      type: activeTab as 'sse' | 'stdio',
      url: '',
      command: '',
      args: [],
      env: {}
    });
    setFormErrors({});
    setOpenDialog(true);
  };

  // Open dialog to edit an existing client
  const openEditClientDialog = (client: MCPClientConfig) => {
    setIsEditMode(true);
    setCurrentClient(client);
    setActiveTab(client.type);
    
    const formattedClient: MCPClientFormData = {
      id: client.id,
      name: client.name,
      enabled: client.enabled,
      type: client.type,
      url: client.url || '',
      command: client.command || '',
      args: client.args || [],
      env: client.env || {},
    };
    
    setFormData(formattedClient);
    setFormErrors({});
    setOpenDialog(true);
  };

  // Handle form input changes
  const handleInputChange = (field: string, value: any) => {
    setFormData(prev => ({ ...prev, [field]: value }));
    
    // Clear error for this field if it exists
    if (formErrors[field]) {
      setFormErrors(prev => {
        const newErrors = { ...prev };
        delete newErrors[field];
        return newErrors;
      });
    }
  };

  // Handle form tab change
  const handleTabChange = (tab: string) => {
    setActiveTab(tab);
    setFormData(prev => ({ ...prev, type: tab as 'sse' | 'stdio' }));
  };

  // Handle environment variable changes
  const handleEnvChange = (key: string, value: string, action: 'add' | 'update' | 'delete') => {
    setFormData(prev => {
      const newEnv = { ...prev.env };
      
      if (action === 'delete') {
        delete newEnv[key];
      } else {
        newEnv[key] = value;
      }
      
      return { ...prev, env: newEnv };
    });
  };

  // Handle arguments changes
  const handleArgsChange = (args: string) => {
    try {
      // Split by spaces, but respect quoted strings
      const argsArray: string[] = [];
      let current = '';
      let inQuotes = false;
      let quoteChar = '';
      
      for (let i = 0; i < args.length; i++) {
        const char = args[i];
        
        if ((char === '"' || char === "'") && (i === 0 || args[i-1] !== '\\')) {
          if (!inQuotes) {
            inQuotes = true;
            quoteChar = char;
          } else if (char === quoteChar) {
            inQuotes = false;
            quoteChar = '';
          } else {
            current += char;
          }
        } else if (char === ' ' && !inQuotes) {
          if (current) {
            argsArray.push(current);
            current = '';
          }
        } else {
          current += char;
        }
      }
      
      if (current) {
        argsArray.push(current);
      }
      
      setFormData(prev => ({ ...prev, args: argsArray }));
    } catch (e) {
      console.error("Error parsing arguments:", e);
      // Just keep the raw string split by spaces as fallback
      setFormData(prev => ({ ...prev, args: args.split(' ').filter(Boolean) }));
    }
  };

  // Validate the form
  const validateForm = (): boolean => {
    const errors: Record<string, string> = {};
    
    if (!formData.name.trim()) {
      errors.name = "Name is required";
    }
    
    if (formData.type === 'sse') {
      if (!formData.url) {
        errors.url = "URL is required for SSE clients";
      } else if (!/^https?:\/\//.test(formData.url)) {
        errors.url = "URL must start with http:// or https://";
      }
    } else if (formData.type === 'stdio') {
      if (!formData.command) {
        errors.command = "Command is required for stdio clients";
      }
    }
    
    setFormErrors(errors);
    return Object.keys(errors).length === 0;
  };

  // Handle form submission
  const handleSubmit = async () => {
    if (!validateForm()) {
      return;
    }
    
    try {
      let response;
      
      if (isEditMode && currentClient?.id) {
        // Update existing client
        response = await fetch(`${MCP_CLIENT_API_PATH}/clients/${currentClient.id}`, {
          method: 'PATCH',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(formData),
        });
      } else {
        // Create new client
        response = await fetch(`${MCP_CLIENT_API_PATH}/clients`, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
          body: JSON.stringify(formData),
        });
      }
      
      if (response.ok) {
        sonnerToast(isEditMode ? "Client updated" : "Client created", {
          description: `MCP client has been ${isEditMode ? 'updated' : 'created'} successfully`
        });
        setOpenDialog(false);
        await loadClients(); // Reload clients to get updated list
      } else {
        const errorText = await response.text();
        sonnerToast(isEditMode ? "Failed to update client" : "Failed to create client", {
          description: errorText || "Check the console for more details"
        });
        console.error(isEditMode ? "Failed to update client:" : "Failed to create client:", errorText);
      }
    } catch (error) {
      console.error(isEditMode ? "Error updating client:" : "Error creating client:", error);
      sonnerToast(isEditMode ? "Error updating client" : "Error creating client", {
        description: error instanceof Error ? error.message : "Unknown error"
      });
    }
  };

  // Render status badge for client
  const renderStatusBadge = (status?: string) => {
    switch (status) {
      case 'connected':
        return <Badge className="flex items-center gap-1 bg-green-500 hover:bg-green-600"><CheckCircle className="h-3 w-3" /> Connected</Badge>;
      case 'error':
        return <Badge variant="destructive" className="flex items-center gap-1"><XCircle className="h-3 w-3" /> Error</Badge>;
      case 'disconnected':
      default:
        return <Badge variant="secondary" className="flex items-center gap-1"><AlertTriangle className="h-3 w-3" /> Disconnected</Badge>;
    }
  };

  return (
    <>
      <Card className="font-mono">
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <div>
            <CardTitle>MCP Client Configuration</CardTitle>
            <CardDescription>
              Configure Model Context Protocol clients for AI tools
            </CardDescription>
          </div>
          <div className="flex space-x-2">
            <Button
              size="sm"
              onClick={refreshAllClients}
              disabled={refreshing || loading}
              className="flex items-center gap-1"
            >
              <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
              Refresh All
            </Button>
            <Button
              size="sm"
              onClick={openNewClientDialog}
              className="flex items-center gap-1"
            >
              <Plus className="h-4 w-4" />
              Add Client
            </Button>
          </div>
        </CardHeader>

        <CardContent className="space-y-4">
          {loading ? (
            <div className="flex justify-center p-4">
              <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
            </div>
          ) : clients.length === 0 ? (
            <Alert>
              <Terminal className="h-4 w-4" />
              <AlertTitle>No MCP clients configured</AlertTitle>
              <AlertDescription>
                Click the "Add Client" button to create your first MCP client configuration.
              </AlertDescription>
            </Alert>
          ) : (
            <div className="space-y-4">
              {clients.map((client) => (
                <Card key={client.id} className="overflow-hidden">
                  <div className="flex justify-between items-center p-4 bg-secondary/20">
                    <div className="flex items-center gap-2">
                      {client.type === 'sse' ? (
                        <Globe className="h-5 w-5 text-primary" />
                      ) : (
                        <TerminalSquare className="h-5 w-5 text-primary" />
                      )}
                      <div className="font-medium">{client.name}</div>
                      {renderStatusBadge(client.status)}
                      {client.lastConnected && (
                        <div className="text-xs text-muted-foreground">
                          Last connected: {new Date(client.lastConnected).toLocaleString()}
                        </div>
                      )}
                    </div>
                    <div className="flex gap-2">
                      <Switch
                        checked={client.enabled}
                        onCheckedChange={(checked) => toggleClientEnabled(client.id, checked)}
                        aria-label={`${client.enabled ? 'Disable' : 'Enable'} ${client.name}`}
                      />
                      <Button
                        size="icon"
                        variant="outline"
                        onClick={() => refreshClient(client.id)}
                        disabled={refreshing}
                      >
                        <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
                      </Button>
                      <Button
                        size="icon"
                        variant="outline"
                        onClick={() => openEditClientDialog(client)}
                      >
                        <Edit className="h-4 w-4" />
                      </Button>
                      <Button
                        size="icon"
                        variant="outline"
                        onClick={() => deleteClient(client.id)}
                      >
                        <Trash className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                  <CardContent className="pt-4">
                    <div className="grid grid-cols-2 gap-4">
                      <div>
                        <Label className="text-xs font-medium">Type</Label>
                        <div className="text-sm mt-1">{client.type === 'sse' ? 'Server-Sent Events (SSE)' : 'Standard I/O (stdio)'}</div>
                      </div>
                      {client.type === 'sse' && client.url && (
                        <div>
                          <Label className="text-xs font-medium">URL</Label>
                          <div className="text-sm mt-1 truncate">{client.url}</div>
                        </div>
                      )}
                      {client.type === 'stdio' && client.command && (
                        <>
                          <div>
                            <Label className="text-xs font-medium">Command</Label>
                            <div className="text-sm mt-1">{client.command}</div>
                          </div>
                          {client.args && client.args.length > 0 && (
                            <div>
                              <Label className="text-xs font-medium">Arguments</Label>
                              <div className="text-sm mt-1">{client.args.join(' ')}</div>
                            </div>
                          )}
                        </>
                      )}
                    </div>
                    
                    {client.statusMessage && (
                      <Alert className="mt-4" variant={client.status === 'error' ? 'destructive' : 'default'}>
                        <AlertTitle>Status Message</AlertTitle>
                        <AlertDescription className="text-xs">{client.statusMessage}</AlertDescription>
                      </Alert>
                    )}
                    
                    {client.env && Object.keys(client.env).length > 0 && (
                      <div className="mt-4">
                        <Label className="text-xs font-medium">Environment Variables</Label>
                        <div className="mt-1 grid grid-cols-2 gap-2">
                          {Object.entries(client.env).map(([key, value]) => (
                            <div key={key} className="text-xs">
                              <span className="font-medium">{key}:</span>{' '}
                              <span className="text-muted-foreground">
                                {value.includes('TOKEN') || key.includes('TOKEN') || key.includes('KEY') 
                                  ? value.replace(/./g, '*') 
                                  : value}
                              </span>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
          
          <Alert className="mt-4">
            <Server className="h-4 w-4" />
            <AlertTitle>MCP Client Information</AlertTitle>
            <AlertDescription>
              <p className="text-sm mb-2">
                Model Context Protocol (MCP) clients provide tools to AI models.
                Configure remote or local clients to enable various capabilities like file operations,
                GitHub integration, and shell commands.
              </p>
              <ul className="list-disc pl-5 text-sm space-y-1">
                <li>SSE clients connect to remote servers via HTTP Server-Sent Events</li>
                <li>stdio clients spawn local child processes and communicate via standard input/output</li>
                <li>Environment variables can be used to configure client behavior</li>
              </ul>
            </AlertDescription>
          </Alert>
        </CardContent>
      </Card>

      {/* Client Creation/Edit Dialog */}
      <Dialog open={openDialog} onOpenChange={setOpenDialog}>
        <DialogContent className="sm:max-w-[600px]">
          <DialogHeader>
            <DialogTitle>{isEditMode ? 'Edit MCP Client' : 'New MCP Client'}</DialogTitle>
            <DialogDescription>
              {isEditMode 
                ? 'Modify the MCP client configuration. Changes will take effect immediately.' 
                : 'Configure a new Model Context Protocol client to provide tools to AI models.'}
            </DialogDescription>
          </DialogHeader>
          
          <div className="grid gap-4 py-4">
            <div className="grid gap-2">
              <Label htmlFor="name">Client Name</Label>
              <Input
                id="name"
                value={formData.name}
                onChange={(e) => handleInputChange('name', e.target.value)}
                placeholder="GitHub MCP Client"
              />
              {formErrors.name && (
                <p className="text-sm text-destructive">{formErrors.name}</p>
              )}
            </div>
            
            <div className="grid gap-2">
              <Label>Client Type</Label>
              <Tabs value={activeTab} onValueChange={handleTabChange} className="w-full">
                <TabsList className="grid w-full grid-cols-2">
                  <TabsTrigger value="sse" className="flex items-center gap-1">
                    <Globe className="h-4 w-4" /> SSE (Remote)
                  </TabsTrigger>
                  <TabsTrigger value="stdio" className="flex items-center gap-1">
                    <Terminal className="h-4 w-4" /> stdio (Local)
                  </TabsTrigger>
                </TabsList>
                
                <TabsContent value="sse" className="space-y-4 mt-4">
                  <div className="grid gap-2">
                    <Label htmlFor="url">Server URL</Label>
                    <Input
                      id="url"
                      value={formData.url}
                      onChange={(e) => handleInputChange('url', e.target.value)}
                      placeholder="https://example.com/mcp/sse"
                    />
                    {formErrors.url && (
                      <p className="text-sm text-destructive">{formErrors.url}</p>
                    )}
                  </div>
                  
                  <div className="grid gap-2">
                    <Label>Headers (Environment Variables)</Label>
                    <div className="grid grid-cols-2 gap-2">
                      {Object.entries(formData.env || {}).map(([key, value]) => (
                        <div key={key} className="flex gap-2 items-center">
                          <Input
                            value={key}
                            onChange={(e) => {
                              const newKey = e.target.value;
                              const oldValue = formData.env?.[key] || '';
                              handleEnvChange(key, '', 'delete');
                              if (newKey) {
                                handleEnvChange(newKey, oldValue, 'add');
                              }
                            }}
                            placeholder="HEADER_NAME"
                            className="flex-1"
                          />
                          <Input
                            value={value}
                            onChange={(e) => handleEnvChange(key, e.target.value, 'update')}
                            placeholder="Value"
                            className="flex-1"
                            type={key.includes('TOKEN') || key.includes('KEY') ? 'password' : 'text'}
                          />
                          <Button
                            size="icon"
                            variant="outline"
                            onClick={() => handleEnvChange(key, '', 'delete')}
                          >
                            <Trash className="h-4 w-4" />
                          </Button>
                        </div>
                      ))}
                    </div>
                    <Button
                      variant="outline"
                      className="mt-1"
                      onClick={() => handleEnvChange(`HEADER_${Object.keys(formData.env || {}).length + 1}`, '', 'add')}
                    >
                      Add Header
                    </Button>
                  </div>
                </TabsContent>
                
                <TabsContent value="stdio" className="space-y-4 mt-4">
                  <div className="grid gap-2">
                    <Label htmlFor="command">Command</Label>
                    <Input
                      id="command"
                      value={formData.command}
                      onChange={(e) => handleInputChange('command', e.target.value)}
                      placeholder="npx"
                    />
                    {formErrors.command && (
                      <p className="text-sm text-destructive">{formErrors.command}</p>
                    )}
                  </div>
                  
                  <div className="grid gap-2">
                    <Label htmlFor="args">Arguments</Label>
                    <Input
                      id="args"
                      value={formData.args?.join(' ') || ''}
                      onChange={(e) => handleArgsChange(e.target.value)}
                      placeholder="-y @modelcontextprotocol/server-github"
                    />
                    <p className="text-xs text-muted-foreground">
                      Separate arguments by spaces. Use quotes for arguments containing spaces.
                    </p>
                  </div>
                  
                  <div className="grid gap-2">
                    <Label>Environment Variables</Label>
                    <div className="grid grid-cols-2 gap-2">
                      {Object.entries(formData.env || {}).map(([key, value]) => (
                        <div key={key} className="flex gap-2 items-center">
                          <Input
                            value={key}
                            onChange={(e) => {
                              const newKey = e.target.value;
                              const oldValue = formData.env?.[key] || '';
                              handleEnvChange(key, '', 'delete');
                              if (newKey) {
                                handleEnvChange(newKey, oldValue, 'add');
                              }
                            }}
                            placeholder="ENV_NAME"
                            className="flex-1"
                          />
                          <Input
                            value={value}
                            onChange={(e) => handleEnvChange(key, e.target.value, 'update')}
                            placeholder="Value"
                            className="flex-1"
                            type={key.includes('TOKEN') || key.includes('KEY') ? 'password' : 'text'}
                          />
                          <Button
                            size="icon"
                            variant="outline"
                            onClick={() => handleEnvChange(key, '', 'delete')}
                          >
                            <Trash className="h-4 w-4" />
                          </Button>
                        </div>
                      ))}
                    </div>
                    <Button
                      variant="outline"
                      className="mt-1"
                      onClick={() => handleEnvChange(`ENV_${Object.keys(formData.env || {}).length + 1}`, '', 'add')}
                    >
                      Add Environment Variable
                    </Button>
                  </div>
                </TabsContent>
              </Tabs>
            </div>
            
            <div className="flex items-center gap-2">
              <Switch
                id="enabled"
                checked={formData.enabled}
                onCheckedChange={(checked) => handleInputChange('enabled', checked)}
              />
              <Label htmlFor="enabled">Enable this client</Label>
            </div>
          </div>
          
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpenDialog(false)}>
              Cancel
            </Button>
            <Button onClick={handleSubmit}>
              {isEditMode ? 'Save Changes' : 'Create Client'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}