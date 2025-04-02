import React, { createContext, useContext, useState, useEffect, useCallback, useRef } from 'react';
import { MODELS, useSettings } from '@openagents/core';

type ModelContextType = {
  selectedModelId: string;
  setSelectedModelId: (id: string) => void;
  handleModelChange: (id: string) => void;
  isModelAvailable: boolean;
  modelWarning: string | null;
  checkCurrentModelAvailability: () => Promise<void>;
};

const ModelContext = createContext<ModelContextType | null>(null);

export const useModelContext = () => {
  const context = useContext(ModelContext);
  if (!context) throw new Error('useModelContext must be used within a ModelProvider');
  return context;
};

export const ModelProvider: React.FC<{children: React.ReactNode}> = ({children}) => {
  const { settings, clearSettingsCache, updateSettings, refresh: refreshSettings, selectModel, getApiKey } = useSettings();
  const [selectedModelId, setSelectedModelId] = useState<string>("");
  const [isModelAvailable, setIsModelAvailable] = useState(true);
  const [modelWarning, setModelWarning] = useState<string | null>(null);
  
  // Use a ref to track the last applied model ID to prevent loops
  const lastAppliedModelRef = useRef<string | null>(null);
  
  // We'll define a mutable ref to hold the availability check function
  const availabilityCheckRef = useRef<(() => Promise<void>) | null>(null);
  
  // Add event listener for page visibility and focus to refresh settings
  useEffect(() => {
    // Store the visibility handler to properly remove it
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        handleFocus();
      }
    };

    // Function to refresh settings from database
    const handleFocus = () => {
      console.log("Page focused, refreshing settings");
      refreshSettings().then(updatedSettings => {
        if (updatedSettings && updatedSettings.selectedModelId) {
          const newModelId = updatedSettings.selectedModelId;
          console.log(`Refreshed settings with model: ${newModelId}`);

          // Check if this model update is new (not just the one we applied)
          // and also different from what's currently selected
          if (selectedModelId !== newModelId && lastAppliedModelRef.current !== newModelId) {
            console.log(`Updating model from ${selectedModelId} to ${newModelId}`);
            // Update our ref to track this change
            lastAppliedModelRef.current = newModelId;
            setSelectedModelId(newModelId);
          }
        }
      });
    };

    // Handle custom event from settings page
    const handleModelSettingsChanged = (event: any) => {
      console.log("Received model-settings-changed event", event.detail);
      if (event.detail && event.detail.selectedModelId) {
        const newModelId = event.detail.selectedModelId;
        console.log(`Setting model from event: ${newModelId}`);

        // Track that we're about to apply this model ID to prevent loops
        lastAppliedModelRef.current = newModelId;

        if (selectedModelId !== newModelId) {
          setSelectedModelId(newModelId);
        }
      } else {
        // If no details provided, just refresh settings
        handleFocus();
      }
    };

    // Add event listeners
    window.addEventListener('focus', handleFocus);
    window.addEventListener('visibilitychange', handleVisibilityChange);
    window.addEventListener('model-settings-changed', handleModelSettingsChanged);

    // Load settings on first mount
    handleFocus();

    // Cleanup event listeners
    return () => {
      window.removeEventListener('focus', handleFocus);
      window.removeEventListener('visibilitychange', handleVisibilityChange);
      window.removeEventListener('model-settings-changed', handleModelSettingsChanged);
    };

    // Only depend on refreshSettings to avoid re-running the effect when selectedModelId changes
  }, [refreshSettings, selectedModelId]);

  useEffect(() => {
    async function setupModel() {
      try {
        // Only proceed when settings are loaded
        if (!settings) return;

        // Look for a user-selected model first (highest priority)
        let userSelectedModel = null;

        // Check active localStorage model (selected by user in this or another tab)
        try {
          if (typeof window !== 'undefined' && window.localStorage) {
            const activeModel = window.localStorage.getItem('openagents_active_model');
            if (activeModel && MODELS.some(model => model.id === activeModel)) {
              userSelectedModel = activeModel;
            }
          }
        } catch (storageError) {
          console.warn("Error reading active model from localStorage:", storageError);
        }

        // If no localStorage model, check sessionStorage (selected in this tab only)
        if (!userSelectedModel) {
          try {
            if (typeof window !== 'undefined' && window.sessionStorage) {
              const currentModel = window.sessionStorage.getItem('openagents_current_model');
              if (currentModel && MODELS.some(model => model.id === currentModel)) {
                console.log(`Using current model from sessionStorage: ${currentModel}`);
                userSelectedModel = currentModel;
              }
            }
          } catch (storageError) {
            console.warn("Error reading from sessionStorage:", storageError);
          }
        }

        // If user has manually selected a model, use it and skip default logic
        if (userSelectedModel) {
          // Skip if already selected
          if (selectedModelId === userSelectedModel) {
            return;
          }

          setSelectedModelId(userSelectedModel);
          return;
        }

        // If no user selection, use selected model from settings (lower priority)
        // First get the model ID from settings, preferring selectedModelId but falling back to defaultModel
        const settingsModelId = settings.selectedModelId || settings.defaultModel;

        // Check if we already have a selected model that matches settings (to prevent unnecessary reselection)
        if (selectedModelId && selectedModelId === settingsModelId) {
          return;
        }

        if (settingsModelId) {
          console.log(`Loading model from settings: ${settingsModelId}`);

          // Check if the model exists in our models list
          const modelExists = MODELS.some(model => model.id === settingsModelId);

          if (modelExists) {
            console.log(`Model ${settingsModelId} found, selecting it`);
            setSelectedModelId(settingsModelId);

            // Also save to sessionStorage for resilience
            try {
              if (typeof window !== 'undefined' && window.sessionStorage) {
                window.sessionStorage.setItem('openagents_current_model', settingsModelId);
              }
            } catch (storageError) {
              console.warn("Error storing model in sessionStorage:", storageError);
            }
          } else {
            // If model doesn't exist, default to first model AND update settings
            console.warn(`Model ${settingsModelId} not found in models list, using fallback`);

            if (MODELS.length > 0) {
              const fallbackModel = MODELS[0].id;
              setSelectedModelId(fallbackModel);

              // Update the settings to use a valid model
              console.log(`Automatically updating settings to use valid model: ${fallbackModel}`);
              try {
                // Update both fields for compatibility
                const result = await updateSettings({
                  defaultModel: fallbackModel,
                  selectedModelId: fallbackModel
                });
                if (result) {
                  console.log("Settings auto-corrected:", result.selectedModelId || result.defaultModel);
                }
              } catch (error) {
                console.error("Failed to auto-correct settings:", error);
              }
            }
          }
        } else {
          // Default to first model if no default is set
          console.log("No model selected in settings, using first model");
          if (MODELS.length > 0) {
            const firstModel = MODELS[0].id;
            setSelectedModelId(firstModel);

            // Save this as the selected model
            console.log(`Setting first model as selected model: ${firstModel}`);
            try {
              await updateSettings({
                defaultModel: firstModel, // For backward compatibility
                selectedModelId: firstModel
              });
              console.log("Selected model saved");
            } catch (error) {
              console.error("Failed to save selected model:", error);
            }
          }
        }
      } catch (error) {
        console.error("Error setting up model:", error);
      }
    }

    setupModel();
  }, [settings, updateSettings, selectedModelId]);

  // Function to check model availability - extracted to be reusable
  const checkCurrentModelAvailability = useCallback(async () => {
    try {
      if (!selectedModelId) return;

      console.log("Checking availability for model:", selectedModelId);

      const selectedModel = MODELS.find(model => model.id === selectedModelId);
      if (!selectedModel) return;

      let isAvailable = true;
      let warning = null;

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

      // Function to check if LMStudio is running - use proxy to avoid CORS issues
      const checkLMStudioAvailable = async (): Promise<boolean> => {
        try {
          // We'll use a stored preference value via settings hook
          let savedLMStudioUrl = "http://localhost:1234";

          try {
            // Use the existing settings hook to get the preference
            if (settings) {
              // Use API to get the LMStudio URL - we can call getApiKey with a special case
              const lmstudioUrlFromApiKeys = await getApiKey('lmstudioUrl');
              if (lmstudioUrlFromApiKeys) {
                savedLMStudioUrl = lmstudioUrlFromApiKeys;
              }
            }

            // If not found in settings object, try to load it directly
            if (!savedLMStudioUrl || savedLMStudioUrl === "http://localhost:1234") {
              const storedPref = localStorage.getItem("openagents_lmstudio_url");
              if (storedPref) {
                savedLMStudioUrl = storedPref;
              }
            }
          } catch (e) {
            console.warn("Error getting LMStudio URL from settings, using default:", e);
          }

          console.log("Using LMStudio URL from settings:", savedLMStudioUrl);

          // Use our server proxy to avoid CORS issues
          const proxyUrl = `/api/proxy/lmstudio/models?url=${encodeURIComponent(`${savedLMStudioUrl}/v1/models`)}`;
          console.log("Checking LMStudio via proxy:", proxyUrl);

          const response = await fetch(proxyUrl);
          console.log("LMStudio check response:", response.status, response.ok);
          return response.ok;
        } catch (error) {
          console.warn("Failed to connect to LMStudio API via proxy:", error);
          return false;
        }
      };

      // Check based on provider
      if (selectedModel.provider === 'openrouter') {
        const key = await getApiKey('openrouter');
        isAvailable = !!key;
        if (!key) {
          warning = "OpenRouter API key required. Add it in Settings > API Keys.";
        }
      }
      else if (selectedModel.provider === 'anthropic') {
        const key = await getApiKey('anthropic');
        isAvailable = !!key;
        if (!key) {
          warning = "Anthropic API key required. Add it in Settings > API Keys.";
        }
      }
      else if (selectedModel.provider === 'openai') {
        const key = await getApiKey('openai');
        isAvailable = !!key;
        if (!key) {
          warning = "OpenAI API key required. Add it in Settings > API Keys.";
        }
      }
      else if (selectedModel.provider === 'google') {
        const key = await getApiKey('google');
        isAvailable = !!key;
        if (!key) {
          warning = "Google API key required. Add it in Settings > API Keys.";
        }
      }
      else if (selectedModel.provider === 'ollama') {
        const ollamaModels = await checkOllamaModels();
        const modelName = selectedModel.id.split('/')[1];

        isAvailable = ollamaModels.length > 0 && ollamaModels.some(m =>
          m === modelName || m.startsWith(`${modelName}:`)
        );

        if (!isAvailable) {
          if (ollamaModels.length === 0) {
            warning = "Ollama server not running. Configure Ollama in Settings > Local Models.";
          } else {
            warning = `Model '${modelName}' not available. Run 'ollama pull ${modelName}'`;
          }
        }
      }
      else if (selectedModel.provider === 'lmstudio') {
        isAvailable = await checkLMStudioAvailable();
        if (!isAvailable) {
          warning = "LMStudio not running or unreachable.";
        }
      }

      console.log(`Model ${selectedModel.id} availability:`, isAvailable);
      setIsModelAvailable(isAvailable);
      setModelWarning(warning);
    } catch (error) {
      console.error("Error checking model availability:", error);
      setIsModelAvailable(true);
      setModelWarning(null);
    }
  }, [selectedModelId, getApiKey, settings]);

  // Save the function to a ref so we can access it before initialization
  useEffect(() => {
    // Save the current availability check function to the ref
    availabilityCheckRef.current = checkCurrentModelAvailability;
  }, [checkCurrentModelAvailability]);

  // Add listener for LMStudio URL changes
  useEffect(() => {
    const handleLMStudioUrlChange = () => {
      console.log("LMStudio URL changed, rechecking model availability");
      if (availabilityCheckRef.current) {
        availabilityCheckRef.current();
      }
    };

    // Listen for URL changes
    window.addEventListener('lmstudio-url-changed', handleLMStudioUrlChange);

    // Also listen for the api-key-changed event as it's also triggered when URL changes
    window.addEventListener('api-key-changed', handleLMStudioUrlChange);

    // Initial check
    if (availabilityCheckRef.current) {
      availabilityCheckRef.current();
    }

    return () => {
      window.removeEventListener('lmstudio-url-changed', handleLMStudioUrlChange);
      window.removeEventListener('api-key-changed', handleLMStudioUrlChange);
    };
  }, []);

  // Handle model change - this happens when user selects from dropdown
  const handleModelChange = useCallback((modelId: string) => {
    console.log(`Model changed via dropdown to: ${modelId}`);

    // Track that we're about to apply this model to prevent loops
    lastAppliedModelRef.current = modelId;

    // Set the model ID for current session
    setSelectedModelId(modelId);

    // Save the selection to sessionStorage for persistence within this tab
    try {
      if (typeof window !== 'undefined' && window.sessionStorage) {
        window.sessionStorage.setItem('openagents_current_model', modelId);
        console.log("Saved model to sessionStorage:", modelId);
      }
    } catch (storageError) {
      console.warn("Error storing model in sessionStorage:", storageError);
    }

    // Also save to localStorage for persistence across tabs (but not as default)
    try {
      if (typeof window !== 'undefined' && window.localStorage) {
        window.localStorage.setItem('openagents_active_model', modelId);
        console.log("Saved model to localStorage:", modelId);

        // If it's an LMStudio model (like gemma), store it in a special key as well
        // This helps with restoring LMStudio models specifically
        if (modelId.includes('gemma') || modelId.toLowerCase().includes('llama')) {
          window.localStorage.setItem('openagents_lmstudio_model', modelId);
          console.log("Saved LMStudio model to localStorage:", modelId);
        }
      }
    } catch (localStorageError) {
      console.warn("Error storing model in localStorage:", localStorageError);
    }

    // Also update the settings to persist this selection
    selectModel(modelId).catch(error => {
      console.error("Failed to update settings with selected model:", error);
    });

    // Dispatch an event to update model availability
    window.dispatchEvent(new CustomEvent('model-selected', {
      detail: { modelId }
    }));
  }, [selectModel]);
  
  return (
    <ModelContext.Provider value={{
      selectedModelId,
      setSelectedModelId,
      handleModelChange,
      isModelAvailable,
      modelWarning,
      checkCurrentModelAvailability,
    }}>
      {children}
    </ModelContext.Provider>
  );
};