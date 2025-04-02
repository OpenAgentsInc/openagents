import React, { useEffect, useState } from "react";
import { useSettings, MODELS } from "@openagents/core";
import { 
  Eye, 
  EyeOff, 
  Check, 
  CheckCircle2,
  Search, 
  ArrowUpDown
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
    }
  }, [settings]);

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

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-full font-mono">
        <p>Loading settings...</p>
      </div>
    );
  }

  return (
    <Card className="font-mono">
      <CardHeader>
        <CardTitle>Model Configuration</CardTitle>
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
                if (confirm("Reset all model settings to default? This will reset your model preferences but will not affect your API keys.")) {
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

                      alert("Model settings reset successfully.");
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
              Reset Model Settings
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}