import React, { useEffect, useState } from "react";
import { useSettings, MODELS } from "@openagents/core";
import { 
  Trash2, 
  Plus, 
  Eye, 
  EyeOff, 
  Check, 
  CheckCircle2,
  CircleSlash, 
  Search, 
  ArrowUpDown
} from "lucide-react";
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Input,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  Alert,
  AlertDescription,
  AlertTitle,
  Table,
  TableBody,
  TableCaption,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui";
import { ModelSelect } from "@/components/ui/model-select";

// Group models by provider for better organization
const providerGroups = MODELS.reduce((acc, model) => {
  const provider = model.provider;
  if (!acc[provider]) {
    acc[provider] = [];
  }
  acc[provider].push(model);
  return acc;
}, {} as Record<string, typeof MODELS>);

// Get unique provider names
const providers = Object.keys(providerGroups);

interface Settings {
  defaultModel: string;
  // Add other settings properties as needed
}

export default function ModelsPage() {
  const { 
    settings, 
    isLoading, 
    setApiKey, 
    getApiKey, 
    deleteApiKey, 
    updateSettings, 
    clearSettingsCache, 
    resetSettings,
    selectModel,
    toggleModelVisibility,
    getVisibleModelIds
  } = useSettings();
  
  // Model state
  const [selectedModelId, setSelectedModelId] = useState("");
  const [visibleModelIds, setVisibleModelIds] = useState<string[]>([]);
  const [currentTab, setCurrentTab] = useState("grid"); // "grid" or "api"
  const [currentProvider, setCurrentProvider] = useState(providers[0] || "");
  
  // API keys state
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});
  const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
  const [keyInputs, setKeyInputs] = useState<Record<string, string>>({});
  
  // Filter and sort state for model grid
  const [filterQuery, setFilterQuery] = useState("");
  const [sortField, setSortField] = useState<string | null>(null);
  const [sortDirection, setSortDirection] = useState<"asc" | "desc">("asc");

  // Load settings when component mounts
  useEffect(() => {
    if (settings) {
      console.log("ModelsPage: Loading settings");
      
      // Get the selected model (prefer selectedModelId, fall back to defaultModel)
      let modelToUse = settings.selectedModelId || settings.defaultModel;
      if (modelToUse) {
        const modelExists = MODELS.some(model => model.id === modelToUse);
        if (!modelExists) {
          console.warn(`Model ${modelToUse} not found in models list`);
          modelToUse = MODELS[0]?.id || "";
        }
      } else {
        modelToUse = MODELS[0]?.id || "";
      }
      
      setSelectedModelId(modelToUse);
      
      // Get visible model IDs
      if (settings.visibleModelIds && settings.visibleModelIds.length > 0) {
        setVisibleModelIds(settings.visibleModelIds);
      } else {
        // If no visible models are set, use default (all models)
        setVisibleModelIds(MODELS.map(model => model.id));
      }

      // Load API keys for all providers
      const loadApiKeys = async () => {
        const keys: Record<string, string> = {};
        for (const provider of providers) {
          const key = await getApiKey(provider);
          if (key) {
            keys[provider] = key;
          }
        }
        setApiKeys(keys);
      };

      loadApiKeys();
    }
  }, [settings, getApiKey]);

  // Handle model selection (replacement for "default model")
  const handleModelSelection = async (modelId: string) => {
    try {
      // Check if the model exists in the list
      const modelExists = MODELS.some(model => model.id === modelId);
      if (!modelExists) {
        console.error(`Model ${modelId} not found in models list`);
        return;
      }

      // Update UI immediately to give user feedback
      setSelectedModelId(modelId);

      console.log(`Selecting model: ${modelId}`);

      // Use the new selectModel method from useSettings
      const result = await selectModel(modelId);
      if (!result) {
        throw new Error("Model selection failed");
      }
      
      console.log("Model selection successful");
      
      // If the selected model is not currently visible, make it visible
      if (visibleModelIds.indexOf(modelId) === -1) {
        handleToggleModelVisibility(modelId);
      }
      
      // Dispatch a custom event to notify that model settings have changed
      // This will trigger the HomePage to refresh its settings
      try {
        const event = new CustomEvent('model-settings-changed', { 
          detail: { selectedModelId: modelId } 
        });
        window.dispatchEvent(event);
        console.log("Dispatched model-settings-changed event");
      } catch (eventError) {
        console.warn("Error dispatching model-settings-changed event:", eventError);
      }
    } catch (error) {
      console.error("Error selecting model:", error);
      alert("There was an error selecting this model. The model will be used for this session only.");
    }
  };
  
  // Handle toggling model visibility
  const handleToggleModelVisibility = async (modelId: string) => {
    try {
      // Check if the model exists in the list
      const modelExists = MODELS.some(model => model.id === modelId);
      if (!modelExists) {
        console.error(`Model ${modelId} not found in models list`);
        return;
      }
      
      // Update local state immediately for better UX
      if (visibleModelIds.includes(modelId)) {
        // Don't allow hiding the selected model
        if (modelId === selectedModelId) {
          alert("You cannot hide the currently selected model.");
          return;
        }
        
        // Don't allow hiding the last visible model
        if (visibleModelIds.length <= 1) {
          alert("You must have at least one visible model.");
          return;
        }
        
        // Hide the model
        setVisibleModelIds(prev => prev.filter(id => id !== modelId));
      } else {
        // Show the model
        setVisibleModelIds(prev => [...prev, modelId]);
      }
      
      // Use the toggleModelVisibility method from useSettings
      const result = await toggleModelVisibility(modelId);
      if (!result) {
        throw new Error("Failed to toggle model visibility");
      }
      
      console.log(`Model ${modelId} visibility toggled successfully`);
    } catch (error) {
      console.error("Error toggling model visibility:", error);
      // Revert the local state changes on error
      if (settings?.visibleModelIds) {
        setVisibleModelIds(settings.visibleModelIds);
      }
    }
  };

  // Handle API key changes
  const handleApiKeyChange = (provider: string, value: string) => {
    setKeyInputs(prev => ({ ...prev, [provider]: value }));
  };
  
  // Handle sort header click
  const handleSortClick = (field: string) => {
    if (sortField === field) {
      // Toggle sort direction if the same field is clicked again
      setSortDirection(sortDirection === 'asc' ? 'desc' : 'asc');
    } else {
      // Set new sort field and reset direction to ascending
      setSortField(field);
      setSortDirection('asc');
    }
  };
  
  // Handle sort and filter for model grid
  const getFilteredAndSortedModels = () => {
    // First filter the models based on the search query
    let filteredModels = MODELS;
    
    if (filterQuery.trim()) {
      const query = filterQuery.toLowerCase();
      filteredModels = MODELS.filter(model => 
        model.name.toLowerCase().includes(query) ||
        model.id.toLowerCase().includes(query) ||
        model.author.toLowerCase().includes(query) ||
        model.provider.toLowerCase().includes(query) ||
        (model.shortDescription && model.shortDescription.toLowerCase().includes(query))
      );
    }
    
    // Then sort the models if a sort field is selected
    if (sortField) {
      filteredModels = [...filteredModels].sort((a, b) => {
        let aValue: any = a[sortField as keyof typeof a];
        let bValue: any = b[sortField as keyof typeof b];
        
        // Handle special cases like numeric values, booleans, etc.
        if (typeof aValue === 'boolean') {
          aValue = aValue ? 1 : 0;
          bValue = bValue ? 1 : 0;
        }
        
        // Handle undefined or null values
        if (aValue === undefined || aValue === null) aValue = '';
        if (bValue === undefined || bValue === null) bValue = '';
        
        // String comparison
        if (typeof aValue === 'string' && typeof bValue === 'string') {
          return sortDirection === 'asc' 
            ? aValue.localeCompare(bValue) 
            : bValue.localeCompare(aValue);
        }
        
        // Numeric comparison
        return sortDirection === 'asc' ? (aValue - bValue) : (bValue - aValue);
      });
    }
    
    return filteredModels;
  };

  // Save API key
  const handleSaveApiKey = async (provider: string) => {
    const key = keyInputs[provider];
    if (key) {
      await setApiKey(provider, key);
      setApiKeys(prev => ({ ...prev, [provider]: key }));
      setKeyInputs(prev => ({ ...prev, [provider]: "" }));
    }
  };

  // Delete API key
  const handleDeleteApiKey = async (provider: string) => {
    await deleteApiKey(provider);
    setApiKeys(prev => {
      const updated = { ...prev };
      delete updated[provider];
      return updated;
    });
  };

  // Toggle API key visibility
  const toggleKeyVisibility = (provider: string) => {
    setShowKeys(prev => ({ ...prev, [provider]: !prev[provider] }));
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full font-mono">
        <p>Loading settings...</p>
      </div>
    );
  }

  return (
    <>
      <Tabs defaultValue="grid" value={currentTab} onValueChange={setCurrentTab}>
        <TabsList className="grid grid-cols-2 mb-4">
          <TabsTrigger value="grid">Models</TabsTrigger>
          <TabsTrigger value="api">API Keys</TabsTrigger>
        </TabsList>
        
        <TabsContent value="grid" className="space-y-4">
          {/* Model Grid */}
          <Card className="font-mono">
            <CardHeader>
              <CardTitle>Model Grid</CardTitle>
              <CardDescription>
                Select and manage your AI models. Toggle visibility to show/hide models in the dropdown.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <div className="space-y-4">
                {/* Search and filter */}
                <div className="flex items-center space-x-2 mb-4">
                  <Search className="h-4 w-4 text-muted-foreground" />
                  <Input
                    placeholder="Search models..."
                    value={filterQuery}
                    onChange={(e) => setFilterQuery(e.target.value)}
                    className="max-w-sm"
                  />
                </div>
                
                {/* Model Table */}
                <div className="rounded-md border">
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead className="w-[50px]">Select</TableHead>
                        <TableHead className="w-[50px]">Show</TableHead>
                        <TableHead className="w-[100px]" onClick={() => handleSortClick('author')}>
                          <div className="flex items-center">
                            Author
                            {sortField === 'author' && (
                              <ArrowUpDown className="ml-2 h-4 w-4" />
                            )}
                          </div>
                        </TableHead>
                        <TableHead onClick={() => handleSortClick('provider')}>
                          <div className="flex items-center">
                            Provider
                            {sortField === 'provider' && (
                              <ArrowUpDown className="ml-2 h-4 w-4" />
                            )}
                          </div>
                        </TableHead>
                        <TableHead className="min-w-[150px]" onClick={() => handleSortClick('name')}>
                          <div className="flex items-center">
                            Name
                            {sortField === 'name' && (
                              <ArrowUpDown className="ml-2 h-4 w-4" />
                            )}
                          </div>
                        </TableHead>
                        <TableHead onClick={() => handleSortClick('id')}>
                          <div className="flex items-center">
                            ID
                            {sortField === 'id' && (
                              <ArrowUpDown className="ml-2 h-4 w-4" />
                            )}
                          </div>
                        </TableHead>
                        <TableHead className="hidden md:table-cell">Description</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {getFilteredAndSortedModels().map((model) => (
                        <TableRow key={model.id}>
                          <TableCell>
                            <Button
                              variant={selectedModelId === model.id ? "default" : "outline"}
                              size="sm"
                              onClick={() => handleModelSelection(model.id)}
                              title={selectedModelId === model.id ? "Selected" : "Select this model"}
                            >
                              {selectedModelId === model.id ? (
                                <CheckCircle2 className="h-4 w-4" />
                              ) : (
                                <Check className="h-4 w-4" />
                              )}
                            </Button>
                          </TableCell>
                          <TableCell>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => handleToggleModelVisibility(model.id)}
                              disabled={model.id === selectedModelId}
                              title={visibleModelIds.includes(model.id) ? "Hide this model" : "Show this model"}
                            >
                              {visibleModelIds.includes(model.id) ? (
                                <Eye className="h-4 w-4" />
                              ) : (
                                <EyeOff className="h-4 w-4" />
                              )}
                            </Button>
                          </TableCell>
                          <TableCell>
                            {model.author}
                          </TableCell>
                          <TableCell>
                            {model.provider}
                          </TableCell>
                          <TableCell>
                            <div className="font-medium">{model.name}</div>
                            <div className="text-xs text-muted-foreground">
                              {model.supportsTools ? "Supports tools" : "No tools"} | {(model.context_length / 1000).toFixed(0)}k ctx
                            </div>
                          </TableCell>
                          <TableCell className="font-mono text-xs">
                            {model.id}
                          </TableCell>
                          <TableCell className="hidden md:table-cell">
                            <div className="text-sm line-clamp-2">
                              {model.shortDescription || "No description available."}
                            </div>
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </div>
                
                {/* Reset button */}
                <div className="flex justify-end">
                  <Button
                    variant="destructive"
                    size="sm"
                    onClick={async () => {
                      if (confirm("Reset all settings to default? This will clear your saved API keys and model preferences.")) {
                        try {
                          const defaultSettings = await resetSettings();

                          if (defaultSettings) {
                            // Update UI to reflect new settings
                            setSelectedModelId(defaultSettings.selectedModelId || defaultSettings.defaultModel || '');
                            if (defaultSettings.visibleModelIds) {
                              setVisibleModelIds(defaultSettings.visibleModelIds);
                            } else {
                              setVisibleModelIds(MODELS.map(model => model.id));
                            }
                            setApiKeys({});

                            alert("Settings reset successfully.");

                            // Load API keys (there should be none after reset)
                            const loadApiKeys = async () => {
                              const keys: Record<string, string> = {};
                              for (const provider of providers) {
                                const key = await getApiKey(provider);
                                if (key) {
                                  keys[provider] = key;
                                }
                              }
                              setApiKeys(keys);
                            };

                            await loadApiKeys();
                          } else {
                            // Fallback if reset returns null
                            console.error("Settings reset returned null result");
                            alert("Settings reset partially completed. You may need to refresh the page.");
                          }
                        } catch (error) {
                          console.error("Failed to reset settings:", error);
                          alert("There was a problem resetting settings. Please try again later.");
                        }
                      }
                    }}
                  >
                    Reset All Settings
                  </Button>
                </div>
              </div>
            </CardContent>
          </Card>
        </TabsContent>
        
        <TabsContent value="api" className="space-y-4">
          {/* API Keys Management */}
          <Card className="font-mono">
            <CardHeader>
              <CardTitle>API Keys</CardTitle>
              <CardDescription>
                Manage your API keys for different model providers
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <Tabs value={currentProvider} onValueChange={setCurrentProvider}>
                <TabsList className="grid font-mono" style={{ gridTemplateColumns: `repeat(${providers.length}, 1fr)` }}>
                  {providers.map(provider => (
                    <TabsTrigger key={provider} value={provider} className="font-mono">
                      {provider}
                    </TabsTrigger>
                  ))}
                </TabsList>

                {providers.map(provider => (
                  <TabsContent key={provider} value={provider} className="space-y-4">
                    {/* Provider Info */}
                    <div className="space-y-2">
                      <h3 className="text-lg font-medium">{provider} Models</h3>
                      <p className="text-sm text-muted-foreground">
                        {provider === "anthropic" && "Anthropic provides Claude models with exceptional reasoning capabilities."}
                        {provider === "openrouter" && "OpenRouter provides access to many AI models from different providers."}
                        {provider === "groq" && "Groq offers ultra-fast inference for various open models."}
                      </p>
                    </div>

                    {/* API Key Management */}
                    <div className="space-y-4">
                      {apiKeys[provider] ? (
                        <div className="space-y-4">
                          <div className="flex items-center space-x-2">
                            <Input
                              type={showKeys[provider] ? "text" : "password"}
                              value={apiKeys[provider]}
                              readOnly
                              className="font-mono"
                            />
                            <Button
                              variant="outline"
                              size="icon"
                              onClick={() => toggleKeyVisibility(provider)}
                            >
                              {showKeys[provider] ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                            </Button>
                            <Button
                              variant="destructive"
                              size="icon"
                              onClick={() => handleDeleteApiKey(provider)}
                            >
                              <Trash2 className="h-4 w-4" />
                            </Button>
                          </div>
                        </div>
                      ) : (
                        <Alert>
                          <AlertTitle className="font-mono">No API key set</AlertTitle>
                          <AlertDescription className="font-mono">
                            You haven't set an API key for {provider} yet. Add one below to use {provider} models.
                          </AlertDescription>
                        </Alert>
                      )}

                      <div className="flex items-center space-x-2">
                        <Input
                          type="password"
                          placeholder={`Enter your ${provider} API key`}
                          value={keyInputs[provider] || ""}
                          onChange={(e) => handleApiKeyChange(provider, e.target.value)}
                          className="font-mono"
                        />
                        <Button onClick={() => handleSaveApiKey(provider)}>
                          <Plus className="h-4 w-4 mr-2" />
                          Save Key
                        </Button>
                      </div>
                    </div>
                  </TabsContent>
                ))}
              </Tabs>
            </CardContent>
            <CardFooter className="text-xs text-muted-foreground">
              Your API keys are stored securely in your browser's local database.
            </CardFooter>
          </Card>
        </TabsContent>
      </Tabs>
    </>
  );
}
