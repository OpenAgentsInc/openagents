import React, { createContext, useContext, useState, useEffect, useCallback } from 'react';
import { useSettings } from '@openagents/core';

type ApiKeyContextType = {
  apiKeys: Record<string, string>;
  loadApiKeys: () => Promise<void>;
};

const ApiKeyContext = createContext<ApiKeyContextType | null>(null);

export const useApiKeyContext = () => {
  const context = useContext(ApiKeyContext);
  if (!context) throw new Error('useApiKeyContext must be used within an ApiKeyProvider');
  return context;
};

export const ApiKeyProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  // Get settings
  const { settings } = useSettings();

  // Store API keys
  const [apiKeys, setApiKeys] = useState<Record<string, string>>({});

  // Load API keys from settings
  const loadApiKeys = useCallback(async () => {
    try {
      if (!settings) return;

      const { settingsRepository } = await import('@openagents/core/src/db/repositories');
      const providers = ['openrouter', 'anthropic', 'openai', 'google', 'ollama', 'lmstudio', 'github'];
      const keys: Record<string, string> = {};

      for (const provider of providers) {
        const key = await settingsRepository.getApiKey(provider);
        if (key) {
          keys[provider] = key;
        }
      }

      // Also load LMStudio URL from settings and add it to the keys object
      try {
        const lmStudioUrlPreference = await settingsRepository.getPreference("lmstudioUrl", "http://localhost:1234");
        if (lmStudioUrlPreference) {
          // Add the URL to the apiKeys object to be sent to the server
          keys['lmstudioUrl'] = lmStudioUrlPreference;
          // console.log(`Loaded LMStudio URL from settings: ${lmStudioUrlPreference}`);
        }
      } catch (error) {
        console.warn("Error loading LMStudio URL from settings:", error);
      }

      // console.log(`Loaded API keys and settings for providers: ${Object.keys(keys).join(', ')}`);
      setApiKeys(keys);
    } catch (error) {
      console.error("Error loading API keys:", error);
    }
  }, [settings]);

  // Load API keys from settings when component mounts and listen for API key changes
  useEffect(() => {
    // Load API keys initially
    loadApiKeys();

    // Handle API key changes from settings page
    const handleApiKeyChange = () => {
      console.log("API key changed, refreshing keys");
      loadApiKeys();
    };

    // Add event listener for API key changes
    window.addEventListener('api-key-changed', handleApiKeyChange);

    // Cleanup event listener
    return () => {
      window.removeEventListener('api-key-changed', handleApiKeyChange);
    };
  }, [loadApiKeys]);

  return (
    <ApiKeyContext.Provider value={{
      apiKeys,
      loadApiKeys
    }}>
      {children}
    </ApiKeyContext.Provider>
  );
};
