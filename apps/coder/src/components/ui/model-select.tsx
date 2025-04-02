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
  const { settings, getApiKey } = useSettings();

  // Function to check availability (extracted for reuse)
  const checkAvailability = useCallback(async () => {
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

            // Handle different response formats
            if (data) {
              if (data.data && Array.isArray(data.data) && data.data.length > 0) {
                console.log("Found LMStudio models in data.data array:", data.data);
                return true;
              } else if (Array.isArray(data) && data.length > 0) {
                console.log("Found LMStudio models in root array:", data);
                return true;
              } else if (data.models && Array.isArray(data.models) && data.models.length > 0) {
                console.log("Found LMStudio models in data.models array:", data.models);
                return true;
              } else if (typeof data === 'object' && Object.keys(data).length > 0) {
                // Consider any response with data as valid
                console.log("Found some data from LMStudio:", data);
                return true;
              }
            }

            // If we didn't find any recognizable model data, but the server responded successfully
            console.log("LMStudio server responded but no model data found. Considering it running anyway");
            return true;
          }

          return false; // Response was not OK

        } catch (error) {
          console.warn("Failed to connect to LMStudio API via proxy:", error);

          if (error instanceof DOMException && error.name === 'AbortError') {
            console.error("LMStudio connection timed out");
          }

          return false;
        }
      };

      // Focus only on LMStudio for now, skipping Ollama
      // const ollamaModels = await checkOllamaModels();
      const ollamaModels: string[] = []; // Empty for now to focus on LMStudio
      const lmStudioAvailable = await checkLMStudioModels();

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
          availability[model.id] = lmStudioAvailable;
          if (!lmStudioAvailable) {
            messages[model.id] = "LMStudio not running or unreachable.";
          }
        }
      }
      return { availability, messages };
    } catch (error) {
      console.error("Error checking model availability:", error);
      return { availability: {}, messages: {} };
    }
  }, [getApiKey, settings]);

  // Check API key availability and model availability for local models
  useEffect(() => {
    async function updateAvailability() {
      const { availability, messages } = await checkAvailability();
      setModelAvailability(availability);
      setModelMessages(messages);
    }

    // Initial check
    updateAvailability();

    // Also listen for URL changes
    const handleLmStudioUrlChange = () => {
      console.log("LMStudio URL changed, updating model availability in ModelSelect");
      updateAvailability();
    };

    // Listen for both regular API key changes and specific LMStudio URL changes
    window.addEventListener('api-key-changed', handleLmStudioUrlChange);
    window.addEventListener('lmstudio-url-changed', handleLmStudioUrlChange);

    return () => {
      window.removeEventListener('api-key-changed', handleLmStudioUrlChange);
      window.removeEventListener('lmstudio-url-changed', handleLmStudioUrlChange);
    };
  }, [checkAvailability]);

  // Filter models based on visibility settings - create memoized value outside the effect
  const filteredModels = useMemo(() => {
    if (settings && settings.visibleModelIds && settings.visibleModelIds.length > 0) {
      // Filter MODELS to only include those in visibleModelIds
      return MODELS.filter(model =>
        settings.visibleModelIds!.includes(model.id)
      );
    } else {
      // Fall back to all models if no visibility settings are found
      return MODELS;
    }
  }, [settings]);

  // Update visible models only when filtered models change
  useEffect(() => {
    setVisibleModels(filteredModels);
  }, [filteredModels]);

  // Find the currently selected model with useMemo
  const selectedModel = useMemo(() =>
    MODELS.find((model) => model.id === value),
    [value]
  );

  // Check if the selected model is available
  const isSelectedModelAvailable = useMemo(() => {
    if (!selectedModel) return true;
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
                      onChange(model.id);
                      setOpen(false);
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
