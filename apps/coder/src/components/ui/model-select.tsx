import React, { useState, useEffect, useMemo, useCallback } from "react";
import { MODELS, useSettings } from "@openagents/core";
import { Check, ChevronDown, AlertCircle } from "lucide-react";
import { cn } from "@/utils/tailwind";
import { Tooltip, TooltipTrigger, TooltipContent } from "@/components/ui";
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
}

export const ModelSelect = React.memo(function ModelSelect({
  value,
  onChange,
  placeholder = "Select a model",
  className,
  disabled = false,
}: ModelSelectProps) {
  const [open, setOpen] = useState(false);
  const [visibleModels, setVisibleModels] = useState<typeof MODELS>([]);
  const [modelAvailability, setModelAvailability] = useState<Record<string, boolean>>({});
  const [modelMessages, setModelMessages] = useState<Record<string, string>>({});
  const [dynamicLmStudioModels, setDynamicLmStudioModels] = useState<typeof MODELS>([]);
  const { settings, getApiKey } = useSettings();

  // Create refs at component level, not inside callbacks
  const lastCheckTimeRef = React.useRef<number>(0);
  // Add a ref to store the current dynamic models for safe access in callbacks
  const dynamicModelsRef = React.useRef<typeof MODELS>([]);

  // Function to check availability (extracted for reuse)
  const checkAvailability = useCallback(async () => {
    // Add safety check to prevent running too frequently (throttle)
    const now = Date.now();
    if (now - lastCheckTimeRef.current < 2000) { // 2 second minimum delay
      console.log("Throttling model availability check - ran too recently");
      return { 
        availability: modelAvailability,  // Return current values
        messages: modelMessages 
      };
    }
    
    lastCheckTimeRef.current = now;
    console.log("Checking model availability in ModelSelect");
    const availability: Record<string, boolean> = {};
    const messages: Record<string, string> = {};

    try {

      // Function to check if Ollama is running and get available models
      const checkOllamaModels = async (): Promise<string[]> => {
        try {
          const response = await fetch("http://localhost:11434/api/tags");
          if (response.ok) {
            const data = await response.json();
            return data.models.map((model: any) => model.name);
          }
        } catch (error) {
          console.warn("Failed to connect to Ollama API:", error);
        }
        return [];
      };

      // Function to check if LMStudio is running and get available models - using proxy to avoid CORS issues
      const checkLMStudioModels = async (): Promise<boolean> => {
        try {
          // Get the stored LMStudio URL from localStorage or fall back to default
          let lmStudioUrl = "http://localhost:1234";

          try {
            // First try to get from localStorage (faster than waiting for DB)
            const storedUrl = localStorage.getItem("openagents_lmstudio_url");
            if (storedUrl) {
              lmStudioUrl = storedUrl;
              console.log("Using LMStudio URL from localStorage:", lmStudioUrl);
            }
          } catch (e) {
            console.warn("Error reading LMStudio URL from localStorage:", e);
          }

          // Use our server-side proxy to avoid CORS issues
          const proxyUrl = `/api/proxy/lmstudio/models?url=${encodeURIComponent(`${lmStudioUrl}/v1/models`)}`;
          console.log("Checking LMStudio via proxy:", proxyUrl);

          const controller = new AbortController();
          const timeoutId = setTimeout(() => controller.abort(), 10000); // 10 second timeout

          const response = await fetch(proxyUrl, {
            method: 'GET',
            headers: { 'Accept': 'application/json' },
            signal: controller.signal
          });

          clearTimeout(timeoutId);
          console.log("LMStudio proxy response status:", response.status, "ok:", response.ok);

          if (response.ok) {
            const data = await response.json();
            console.log("LMStudio models data:", data);
            
            // Extract model IDs from different formats and create model entries
            let newDynamicModels: typeof MODELS = [];
            
            if (data) {
              if (data.data && Array.isArray(data.data) && data.data.length > 0) {
                // Standard OpenAI format
                console.log("Found LMStudio models in data.data array:", data.data);
                
                newDynamicModels = data.data.map((model: any) => {
                  const id = model.id || "unknown";
                  // Check if we have this model in MODELS
                  const existingModel = MODELS.find(m => m.id === id && m.provider === 'lmstudio');
                  
                  if (existingModel) {
                    return existingModel;
                  } else {
                    // Create new model
                    const modelName = id.split('/').pop() || id;
                    const formattedName = modelName
                      .replace(/-/g, ' ')
                      .replace(/([a-z])([A-Z])/g, '$1 $2') // Add space between camelCase
                      .split(' ')
                      .map(word => word.charAt(0).toUpperCase() + word.slice(1))
                      .join(' ');
                    
                    return {
                      id,
                      name: formattedName,
                      provider: 'lmstudio' as const,
                      author: model.owned_by?.split('_')[0] as any || 'unknown' as any,
                      created: Date.now(),
                      description: `${formattedName} model running on LMStudio`,
                      context_length: 8192, // Default
                      supportsTools: true,
                      shortDescription: `Local ${formattedName} model running on LMStudio`
                    };
                  }
                });
                
              } else if (Array.isArray(data) && data.length > 0) {
                // Array format
                console.log("Found LMStudio models in root array:", data);
                
                newDynamicModels = data.map((model: any) => {
                  const id = typeof model === 'string' ? model : (model.id || model.name || model.model || "unknown");
                  
                  // Check if we have this model in MODELS
                  const existingModel = MODELS.find(m => m.id === id && m.provider === 'lmstudio');
                  
                  if (existingModel) {
                    return existingModel;
                  } else {
                    // Create new model
                    const modelName = id.split('/').pop() || id;
                    const formattedName = modelName
                      .replace(/-/g, ' ')
                      .replace(/([a-z])([A-Z])/g, '$1 $2')
                      .split(' ')
                      .map(word => word.charAt(0).toUpperCase() + word.slice(1))
                      .join(' ');
                    
                    return {
                      id,
                      name: formattedName,
                      provider: 'lmstudio' as const,
                      author: 'unknown' as any,
                      created: Date.now(),
                      description: `${formattedName} model running on LMStudio`,
                      context_length: 8192,
                      supportsTools: true,
                      shortDescription: `Local ${formattedName} model running on LMStudio`
                    };
                  }
                });
              } else if (data.models && Array.isArray(data.models) && data.models.length > 0) {
                // models array format
                console.log("Found LMStudio models in data.models array:", data.models);
                
                newDynamicModels = data.models.map((model: any) => {
                  const id = typeof model === 'string' ? model : (model.id || model.name || model.model || "unknown");
                  
                  // Check for existing model
                  const existingModel = MODELS.find(m => m.id === id && m.provider === 'lmstudio');
                  
                  if (existingModel) {
                    return existingModel;
                  } else {
                    // Create new model
                    const modelName = id.split('/').pop() || id;
                    const formattedName = modelName
                      .replace(/-/g, ' ')
                      .replace(/([a-z])([A-Z])/g, '$1 $2')
                      .split(' ')
                      .map(word => word.charAt(0).toUpperCase() + word.slice(1))
                      .join(' ');
                    
                    return {
                      id,
                      name: formattedName,
                      provider: 'lmstudio' as const,
                      author: 'unknown' as any,
                      created: Date.now(),
                      description: `${formattedName} model running on LMStudio`,
                      context_length: 8192,
                      supportsTools: true,
                      shortDescription: `Local ${formattedName} model running on LMStudio`
                    };
                  }
                });
              }
            }
            
            console.log("Created dynamic LMStudio models:", newDynamicModels);
            
            // Compare with current models to see if we actually need to update
            // This prevents infinite update loops
            const currentModels = dynamicLmStudioModels;
            const needsUpdate = newDynamicModels.length !== currentModels.length || 
              newDynamicModels.some(newModel => 
                !currentModels.some(currentModel => currentModel.id === newModel.id)
              );
            
            if (needsUpdate) {
              console.log("Updating dynamic models - models have changed");
              setDynamicLmStudioModels(newDynamicModels);
            } else {
              console.log("Skipping dynamic models update - no changes detected");
            }
            
            return newDynamicModels.length > 0;
          }

          return false; // Response was not OK

        } catch (error) {
          console.warn("Failed to connect to LMStudio API via proxy:", error);

          if (error instanceof DOMException && error.name === 'AbortError') {
            console.error("LMStudio connection timed out");
          }
          
          // Only clear dynamic models on error if we actually have some
          // This prevents unnecessary re-renders
          if (dynamicLmStudioModels.length > 0) {
            console.log("Clearing dynamic models due to error");
            setDynamicLmStudioModels([]);
          }
          return false;
        }
      };

      // Focus only on LMStudio for now, skipping Ollama
      // const ollamaModels = await checkOllamaModels();
      const ollamaModels: string[] = []; // Empty for now to focus on LMStudio
      const lmStudioAvailable = await checkLMStudioModels();
      
      // Add availability for dynamic LMStudio models
      // These are always available since they were just discovered
      for (const model of dynamicLmStudioModels) {
        availability[model.id] = true;
      }

      for (const model of MODELS) {
        // Default to available
        availability[model.id] = true;

        // Check based on provider
        if (model.provider === 'openrouter') {
          const key = await getApiKey('openrouter');
          availability[model.id] = !!key;
          if (!key) {
            messages[model.id] = "OpenRouter API key required. Add it in Settings > API Keys.";
          }
        }
        else if (model.provider === 'anthropic') {
          const key = await getApiKey('anthropic');
          availability[model.id] = !!key;
          if (!key) {
            messages[model.id] = "Anthropic API key required. Add it in Settings > API Keys.";
          }
        }
        else if (model.provider === 'openai') {
          const key = await getApiKey('openai');
          availability[model.id] = !!key;
          if (!key) {
            messages[model.id] = "OpenAI API key required. Add it in Settings > API Keys.";
          }
        }
        else if (model.provider === 'google') {
          const key = await getApiKey('google');
          availability[model.id] = !!key;
          if (!key) {
            messages[model.id] = "Google API key required. Add it in Settings > API Keys.";
          }
        }
        else if (model.provider === 'ollama') {
          // Temporarily disable all Ollama models to focus on LMStudio
          availability[model.id] = false;
          messages[model.id] = "Ollama support temporarily disabled";
        }
        else if (model.provider === 'lmstudio') {
          // Only show static LMStudio models if we don't have dynamic ones
          if (dynamicLmStudioModels.length > 0) {
            // Hide static models when we have dynamic ones
            availability[model.id] = false;
            messages[model.id] = "Using dynamically discovered LMStudio models instead.";
          } else {
            availability[model.id] = lmStudioAvailable;
            if (!lmStudioAvailable) {
              messages[model.id] = "LMStudio not running or unreachable.";
            }
          }
        }
      }
      return { availability, messages };
    } catch (error) {
      console.error("Error checking model availability:", error);
      // Return current state on error to prevent additional thrashing
      return { 
        availability: modelAvailability, 
        messages: modelMessages 
      };
    }
  }, [getApiKey, settings, modelAvailability, modelMessages]);

  // Create a ref to track updates outside of the effect
  const lastUpdateRef = React.useRef<number>(0);
  
  // Check API key availability and model availability for local models
  useEffect(() => {
    // Using the ref created outside the effect
    
    async function updateAvailability() {
      // Prevent update-triggered loops by enforcing a minimum delay between updates
      const now = Date.now();
      if (now - lastUpdateRef.current < 1000) {
        console.log("Skipping rapid availability update to prevent loop");
        return;
      }
      
      lastUpdateRef.current = now;
      console.log("Running availability update at:", new Date().toISOString());
      
      const { availability, messages } = await checkAvailability();
      setModelAvailability(availability);
      setModelMessages(messages);
    }

    // Initial check
    updateAvailability();

    // Listen for URL changes
    const handleLmStudioUrlChange = () => {
      console.log("LMStudio URL changed, updating model availability in ModelSelect");
      updateAvailability();
    };
    
    // Handle when a model is selected from outside this component
    const handleModelSelected = (event: CustomEvent<{modelId: string}>) => {
      console.log("Model selected event received:", event.detail?.modelId);
      
      // If the selected model is not in our dynamic models, add it
      const modelId = event.detail?.modelId;
      // Skip if we already have this model to prevent loops
      // Use the ref instead of the state to avoid hook rules issues
      if (modelId && 
          !dynamicModelsRef.current.some(model => model.id === modelId) && 
          (modelId.includes('gemma') || modelId.toLowerCase().includes('llama'))) {
            
        // Create a temporary model entry
        const modelName = modelId.split('/').pop() || modelId;
        const formattedName = modelName
          .replace(/-/g, ' ')
          .replace(/([a-z])([A-Z])/g, '$1 $2')
          .split(' ')
          .map(word => word.charAt(0).toUpperCase() + word.slice(1))
          .join(' ');
        
        const newModel = {
          id: modelId,
          name: formattedName,
          provider: 'lmstudio' as const,
          author: 'unknown' as any,
          created: Date.now(),
          description: `${formattedName} model running on LMStudio`,
          context_length: 8192,
          supportsTools: true,
          shortDescription: `Local ${formattedName} model running on LMStudio`
        };
        
        // Use a function that doesn't trigger a dependency update
        setModelAvailability(prev => ({
          ...prev,
          [modelId]: true
        }));
        
        // Add the model to our dynamic models list - but don't allow this to
        // trigger checkAvailability again by doing it outside the normal effect flow
        setTimeout(() => {
          setDynamicLmStudioModels(prev => {
            // Prevent duplicates
            if (prev.some(m => m.id === modelId)) return prev;
            return [...prev, newModel];
          });
        }, 0);
      }
    };

    // Listen for various events
    window.addEventListener('api-key-changed', handleLmStudioUrlChange);
    window.addEventListener('lmstudio-url-changed', handleLmStudioUrlChange);
    window.addEventListener('model-selected', handleModelSelected as EventListener);

    return () => {
      window.removeEventListener('api-key-changed', handleLmStudioUrlChange);
      window.removeEventListener('lmstudio-url-changed', handleLmStudioUrlChange);
      window.removeEventListener('model-selected', handleModelSelected as EventListener);
    };
    // We still only depend on checkAvailability to avoid unnecessary re-runs
    // dynamicLmStudioModels is accessed via ref.current inside handleModelSelected
  }, [checkAvailability]);

  // Filter models based on visibility settings and include dynamic LMStudio models
  const filteredModels = useMemo(() => {
    // Start with static models based on visibility settings
    let baseModels: typeof MODELS = [];
    
    if (settings && settings.visibleModelIds && settings.visibleModelIds.length > 0) {
      // Include only models in visibleModelIds (except for LMStudio models)
      baseModels = MODELS.filter(model => 
        // For non-LMStudio models, check visibility
        (model.provider !== 'lmstudio' && settings.visibleModelIds!.includes(model.id)) ||
        // Always include LMStudio models that are in MODELS
        (model.provider === 'lmstudio')
      );
    } else {
      // Fall back to all static models if no visibility settings
      baseModels = MODELS;
    }
    
    // Filter out LMStudio models from MODELS if we have dynamic ones
    const staticModels = dynamicLmStudioModels.length > 0 
      ? baseModels.filter(model => model.provider !== 'lmstudio')
      : baseModels;
    
    // Make sure the selected model is in the list
    let modelsWithSelected = [...staticModels, ...dynamicLmStudioModels];
    
    // If we have a selected model that's not in the list
    if (value && !modelsWithSelected.some(model => model.id === value)) {
      const modelName = value.split('/').pop() || value;
      const formattedName = modelName
        .replace(/-/g, ' ')
        .replace(/([a-z])([A-Z])/g, '$1 $2')
        .split(' ')
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');
      
      // Create a temporary model entry to represent the selected model
      const dynamicSelectedModel = {
        id: value,
        name: formattedName,
        provider: value.includes("gemma") || value.toLowerCase().includes("llama") ? 'lmstudio' as const : 'unknown' as any,
        author: 'unknown' as any,
        created: Date.now(),
        description: `${formattedName} model`,
        context_length: 8192,
        supportsTools: true,
        shortDescription: `Dynamic model: ${formattedName}`
      };
      
      modelsWithSelected.push(dynamicSelectedModel);
    }
      
    // Return the combination of static models and dynamic LMStudio models
    return modelsWithSelected;
  }, [settings, dynamicLmStudioModels, value]);

  // Keep the ref in sync with the state
  useEffect(() => {
    dynamicModelsRef.current = dynamicLmStudioModels;
  }, [dynamicLmStudioModels]);

  // Update visible models when filtered models change
  // Also make sure we include the currently selected model if it exists
  useEffect(() => {
    // If value is set but not in filtered models, add it specially
    if (value && filteredModels.every(model => model.id !== value)) {
      console.log("Currently selected model not found in filtered models. Adding it:", value);
      
      // Create temporary model object for the current value
      const modelName = value.split('/').pop() || value;
      const formattedName = modelName
        .replace(/-/g, ' ')
        .replace(/([a-z])([A-Z])/g, '$1 $2')
        .split(' ')
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');
      
      const temporaryModel = {
        id: value,
        name: formattedName,
        provider: value.includes("gemma") ? 'lmstudio' as const : 'unknown' as any,
        author: 'unknown' as any,
        created: Date.now(),
        description: `${formattedName} model`,
        context_length: 8192,
        supportsTools: true,
        shortDescription: `Model: ${formattedName}`
      };
      
      // Add the current model to visible models
      setVisibleModels([...filteredModels, temporaryModel]);
    } else {
      setVisibleModels(filteredModels);
    }
  }, [filteredModels, value]);

  // Find the currently selected model with useMemo - check both static and dynamic models
  const selectedModel = useMemo(() => {
    // First check dynamic models (they take priority)
    const dynamicMatch = dynamicLmStudioModels.find(model => model.id === value);
    if (dynamicMatch) return dynamicMatch;
    
    // Then check static models
    const staticMatch = MODELS.find(model => model.id === value);
    if (staticMatch) return staticMatch;
    
    // If no match found but we have a value, create a temporary model object
    // This handles cases where the model ID exists but hasn't been loaded into either array yet
    if (value) {
      console.log("Creating temporary model object for ID:", value);
      // Try to extract a decent name from the ID
      const modelName = value.split('/').pop() || value;
      const formattedName = modelName
        .replace(/-/g, ' ')
        .replace(/([a-z])([A-Z])/g, '$1 $2')
        .split(' ')
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(' ');
      
      return {
        id: value,
        name: formattedName,
        provider: value.includes("gemma") ? 'lmstudio' : 'unknown',
        author: 'unknown' as any,
        created: Date.now(),
        description: `${formattedName} model`,
        context_length: 8192,
        supportsTools: true,
        shortDescription: `Dynamic model: ${formattedName}`
      };
    }
    
    return undefined;
  }, [value, dynamicLmStudioModels]);

  // Check if the selected model is available
  const isSelectedModelAvailable = useMemo(() => {
    if (!selectedModel) return true;
    
    // If it's an LMStudio model (which includes Gemma models),
    // assume it's available even if not explicitly in the availability list
    if (selectedModel.provider === 'lmstudio' && selectedModel.id.includes('gemma')) {
      // If not explicitly marked as unavailable, consider it available
      return modelAvailability[selectedModel.id] !== false;
    }
    
    return modelAvailability[selectedModel.id] !== false;
  }, [selectedModel, modelAvailability]);

  // Get warning message for the selected model
  const selectedModelWarning = useMemo(() => {
    if (!selectedModel) return null;
    return modelMessages[selectedModel.id] || null;
  }, [selectedModel, modelMessages]);

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          className={cn(
            "w-full justify-between overflow-hidden text-ellipsis whitespace-nowrap font-mono",
            !isSelectedModelAvailable && "border-yellow-500 text-yellow-600 dark:text-yellow-400",
            className
          )}
          disabled={disabled}
        >
          <div className="flex items-center overflow-hidden">
            <span className="overflow-hidden text-ellipsis">
              {selectedModel ? selectedModel.name : placeholder}
            </span>

            {!isSelectedModelAvailable && selectedModelWarning && (
              <Tooltip>
                <TooltipTrigger asChild>
                  <AlertCircle className="ml-2 h-4 w-4 text-yellow-500 shrink-0" />
                </TooltipTrigger>
                <TooltipContent>
                  <p>{selectedModelWarning}</p>
                </TooltipContent>
              </Tooltip>
            )}
          </div>
          <ChevronDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[300px] p-0 font-mono" align="start">
        <Command className="font-mono">
          <CommandInput placeholder="Search models..." className="font-mono" />
          <CommandEmpty className="font-mono">No model found.</CommandEmpty>
          <CommandGroup className="max-h-[300px] overflow-auto font-mono">
            {visibleModels.map((model) => {
              // Precompute this value to avoid recalculation in the render
              const isSelected = value === model.id;
              const isAvailable = modelAvailability[model.id] !== false;
              const warningMessage = modelMessages[model.id];

              return (
                <CommandItem
                  key={model.id}
                  value={model.id}
                  onSelect={() => {
                    // Only allow selection if model is available
                    if (isAvailable) {
                      console.log("Selecting model:", model.id, model.name);
                      onChange(model.id);
                      setOpen(false);
                      
                      // Update availability to ensure this model stays available
                      setModelAvailability(prev => ({
                        ...prev,
                        [model.id]: true
                      }));
                      
                      // Additionally update dynamic models list if needed
                      if (model.provider === 'lmstudio' && 
                          !dynamicModelsRef.current.some(m => m.id === model.id)) {
                        console.log("Adding selected model to dynamic models list");
                        setDynamicLmStudioModels(prev => [...prev, model]);
                      }
                      
                      window.dispatchEvent(new Event('focus-chat-input'));
                    }
                  }}
                  className={cn(
                    "font-mono",
                    !isAvailable && "cursor-not-allowed opacity-60"
                  )}
                  disabled={!isAvailable}
                >
                  <div className="flex flex-col gap-1 truncate font-mono">
                    <div className="flex items-center gap-2">
                      {isSelected ? (
                        <Check className="h-4 w-4" />
                      ) : (
                        <span className="h-4 w-4" />
                      )}
                      <span className="font-medium font-mono">{model.name}</span>

                      {!isAvailable && warningMessage && (
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <AlertCircle className="h-4 w-4 text-yellow-500" />
                          </TooltipTrigger>
                          <TooltipContent>
                            <p>{warningMessage}</p>
                          </TooltipContent>
                        </Tooltip>
                      )}
                    </div>
                    <div className="text-xs text-muted-foreground pl-6">
                      {model.provider} {model.supportsTools ? "• Tools" : ""} • {Math.round(model.context_length / 1000)}k ctx
                    </div>
                  </div>
                </CommandItem>
              );
            })}
          </CommandGroup>
        </Command>
      </PopoverContent>
    </Popover>
  );
});
