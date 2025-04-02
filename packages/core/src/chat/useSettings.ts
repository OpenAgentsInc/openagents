import { useState, useEffect, useCallback } from 'react';
import { settingsRepository } from '../db/repositories';
import { Settings } from '../db/types';

/**
 * Hook for accessing application settings
 */
export function useSettings() {
  // Settings state
  const [settings, setSettings] = useState<Settings | null>(null);
  
  // Loading state
  const [isLoading, setIsLoading] = useState<boolean>(true);
  
  // Error state
  const [error, setError] = useState<Error | null>(null);
  
  // Load settings
  const loadSettings = useCallback(async () => {
    try {
      setIsLoading(true);
      setError(null);
      
      const appSettings = await settingsRepository.getSettings();
      setSettings(appSettings);
      
      return appSettings;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      setError(error);
      console.error('Error loading settings:', error);
      return null;
    } finally {
      setIsLoading(false);
    }
  }, []);
  
  // Update settings
  const updateSettings = useCallback(async (updates: Partial<Settings>): Promise<Settings | null> => {
    try {
      const updatedSettings = await settingsRepository.updateSettings(updates);
      setSettings(updatedSettings);
      return updatedSettings;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error updating settings:', error);
      return null;
    }
  }, []);
  
  // Set API key for a provider
  const setApiKey = useCallback(async (provider: string, key: string): Promise<void> => {
    try {
      await settingsRepository.setApiKey(provider, key);
      await loadSettings();
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error setting API key:', error);
    }
  }, [loadSettings]);
  
  // Get API key for a provider
  const getApiKey = useCallback(async (provider: string): Promise<string | null> => {
    try {
      return await settingsRepository.getApiKey(provider);
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error getting API key:', error);
      return null;
    }
  }, []);
  
  // Delete API key for a provider
  const deleteApiKey = useCallback(async (provider: string): Promise<void> => {
    try {
      await settingsRepository.deleteApiKey(provider);
      await loadSettings();
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error deleting API key:', error);
    }
  }, [loadSettings]);
  
  // Set a preference value
  const setPreference = useCallback(async <T>(key: string, value: T): Promise<void> => {
    try {
      await settingsRepository.setPreference(key, value);
      await loadSettings();
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error setting preference:', error);
    }
  }, [loadSettings]);
  
  // Get a preference value
  const getPreference = useCallback(async <T>(key: string, defaultValue: T): Promise<T> => {
    try {
      return await settingsRepository.getPreference(key, defaultValue);
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error getting preference:', error);
      return defaultValue;
    }
  }, []);
  
  // Initial load
  useEffect(() => {
    loadSettings();
  }, [loadSettings]);
  
  // Clear settings cache in repository
  const clearSettingsCache = useCallback(() => {
    settingsRepository.clearCache();
    // Reload settings
    return loadSettings();
  }, [loadSettings]);
  
  // Reset settings to defaults
  const resetSettings = useCallback(async () => {
    try {
      const result = await settingsRepository.resetSettings();
      await loadSettings(); // Reload after reset
      return result;
    } catch (err) {
      console.error('Error resetting settings:', err);
      return null;
    }
  }, [loadSettings]);

  // Select a model as the current active model
  const selectModel = useCallback(async (modelId: string): Promise<Settings | null> => {
    try {
      const updatedSettings = await settingsRepository.selectModel(modelId);
      setSettings(updatedSettings);
      return updatedSettings;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error selecting model:', error);
      return null;
    }
  }, []);

  // Show a model in the selector
  const showModel = useCallback(async (modelId: string): Promise<Settings | null> => {
    try {
      const updatedSettings = await settingsRepository.showModel(modelId);
      setSettings(updatedSettings);
      return updatedSettings;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error showing model:', error);
      return null;
    }
  }, []);

  // Hide a model from the selector
  const hideModel = useCallback(async (modelId: string): Promise<Settings | null> => {
    try {
      const updatedSettings = await settingsRepository.hideModel(modelId);
      setSettings(updatedSettings);
      return updatedSettings;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error hiding model:', error);
      return null;
    }
  }, []);

  // Toggle a model's visibility
  const toggleModelVisibility = useCallback(async (modelId: string): Promise<Settings | null> => {
    try {
      const updatedSettings = await settingsRepository.toggleModelVisibility(modelId);
      setSettings(updatedSettings);
      return updatedSettings;
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error toggling model visibility:', error);
      return null;
    }
  }, []);

  // Get visible model IDs
  const getVisibleModelIds = useCallback(async (): Promise<string[]> => {
    try {
      return await settingsRepository.getVisibleModelIds();
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      console.error('Error getting visible model IDs:', error);
      return [];
    }
  }, []);

  return {
    settings,
    isLoading,
    error,
    refresh: loadSettings,
    updateSettings,
    setApiKey,
    getApiKey,
    deleteApiKey,
    setPreference,
    getPreference,
    clearSettingsCache,
    resetSettings,
    // New model grid methods
    selectModel,
    showModel,
    hideModel,
    toggleModelVisibility,
    getVisibleModelIds
  };
}