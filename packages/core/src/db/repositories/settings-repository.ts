import { getDatabase } from '../database';
import { Settings, Database } from '../types';
import { RxDocument } from 'rxdb';

// Global settings ID
const GLOBAL_SETTINGS_ID = 'global';

/**
 * Helper function to create a mutable copy of settings
 * We treat all settings as potentially readonly
 */
function toMutableSettings(settings: any): Settings {
  if (!settings) {
    // Default settings if null or undefined
    return {
      id: GLOBAL_SETTINGS_ID,
      theme: 'system',
      apiKeys: {},
      defaultModel: 'anthropic/claude-3.7-sonnet',
      selectedModelId: 'anthropic/claude-3.7-sonnet',
      visibleModelIds: [],
      preferences: {}
    };
  }

  // Create a mutable copy with explicit handling for arrays
  const result: Settings = {
    id: settings.id,
    theme: settings.theme,
    apiKeys: settings.apiKeys ? { ...settings.apiKeys } : {},
    defaultModel: settings.defaultModel,
    selectedModelId: settings.selectedModelId,
    // Ensure visibleModelIds is a mutable array
    visibleModelIds: settings.visibleModelIds ? [...settings.visibleModelIds] : [],
    preferences: settings.preferences ? { ...settings.preferences } : {}
  };

  return result;
}

/**
 * Repository for settings operations
 */
export class SettingsRepository {
  private db: Database | null = null;
  private cachedSettings: Settings | null = null;
  private settingsInitInProgress = false;
  private settingsInitPromise: Promise<Settings> | null = null;
  // Track pending model updates for optimistic responses
  private _pendingModelUpdate: string | null = null;
  // Track pending visibility updates for optimistic responses
  private _pendingVisibilityUpdates: string[] | null = null;

  constructor() {
    // Try to load any pending model update from localStorage
    try {
      if (typeof window !== 'undefined' && window.localStorage) {
        const pendingModel = window.localStorage.getItem('openagents_pending_model');
        if (pendingModel) {
          console.log(`Loading pending model from localStorage: ${pendingModel}`);
          this._pendingModelUpdate = pendingModel;
        }

        const pendingVisibility = window.localStorage.getItem('openagents_pending_visibility');
        if (pendingVisibility) {
          try {
            const visibleIds = JSON.parse(pendingVisibility);
            if (Array.isArray(visibleIds)) {
              console.log(`Loading pending visibility from localStorage: ${visibleIds.length} models`);
              this._pendingVisibilityUpdates = visibleIds;
            }
          } catch (e) {
            console.warn("Error parsing pending visibility from localStorage:", e);
          }
        }
      }
    } catch (error) {
      console.warn("Error loading pending updates from localStorage:", error);
    }
  }

  /**
   * Initialize the repository with a database connection
   */
  async initialize(): Promise<void> {
    if (!this.db) {
      this.db = await getDatabase();
    }
  }

  /**
   * Get the global settings
   */
  async getSettings(): Promise<Settings> {
    // Apply any pending updates to cached settings
    if (this.cachedSettings) {
      let updatedSettings = { ...this.cachedSettings };
      let hasChanges = false;

      // Apply pending model update if available
      if (this._pendingModelUpdate) {
        updatedSettings = {
          ...updatedSettings,
          defaultModel: this._pendingModelUpdate,
          selectedModelId: this._pendingModelUpdate
        };
        hasChanges = true;
      }

      // Apply pending visibility updates if available
      if (this._pendingVisibilityUpdates) {
        updatedSettings = {
          ...updatedSettings,
          visibleModelIds: this._pendingVisibilityUpdates
        };
        hasChanges = true;
      }

      // If we have changes, return the updated settings
      if (hasChanges) {
        console.log("Returning settings with pending updates applied");
        return toMutableSettings(updatedSettings);
      }

      // Otherwise return cached settings as-is
      console.log("Returning cached settings");
      return toMutableSettings(this.cachedSettings!);
    }

    // Always try to fetch from database first before using localStorage backups
    await this.initialize();

    try {
      // Try fetching directly from database
      const settingsDoc = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
      if (settingsDoc) {
        const settings = settingsDoc.toJSON();
        console.log("Found settings in database:", settings.selectedModelId || settings.defaultModel);
        this.cachedSettings = toMutableSettings(settings);
        return toMutableSettings(settings);
      }
    } catch (e) {
      console.warn("Error fetching settings from database:", e);
    }

    await this.initialize();

    // If settings initialization is already in progress, wait for that to complete
    if (this.settingsInitInProgress && this.settingsInitPromise) {
      return this.settingsInitPromise;
    }

    // Set flag to indicate we're initializing settings
    this.settingsInitInProgress = true;

    // Create a promise to handle concurrent calls
    this.settingsInitPromise = (async () => {
      try {
        // Check if there's persisted settings in localStorage as a backup
        let localSettings: Settings | null = null;
        try {
          if (typeof window !== 'undefined' && window.localStorage) {
            const storedSettings = window.localStorage.getItem('openagents_settings_backup');
            if (storedSettings) {
              try {
                localSettings = JSON.parse(storedSettings) as Settings;
                // console.log("Found backup settings in localStorage");
              } catch (parseError) {
                console.warn("Error parsing localStorage settings:", parseError);
              }
            }
          }
        } catch (localStorageError) {
          console.warn("Error accessing localStorage:", localStorageError);
        }

        // Try to find existing settings
        let settings: RxDocument<Settings, {}> | null = null;
        try {
          settings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
        } catch (findError) {
          console.error("Error finding settings:", findError);
          // Continue to the default settings creation path
        }

        if (settings) {
          let settingsData = settings.toJSON();

          // Validate the default model - ensure it's a string
          if (settingsData.defaultModel && typeof settingsData.defaultModel !== 'string') {
            console.warn("Invalid defaultModel type, resetting to default");
            settingsData = { ...settingsData, defaultModel: 'qwen-qwq-32b' }; // Create new object with fallback model
          }

          // Keep a backup of valid settings in localStorage
          try {
            if (typeof window !== 'undefined' && window.localStorage) {
              window.localStorage.setItem('openagents_settings_backup', JSON.stringify(settingsData));
            }
          } catch (backupError) {
            console.warn("Error backing up settings to localStorage:", backupError);
          }

          const mutableData = toMutableSettings(settingsData);
          this.cachedSettings = mutableData;
          return mutableData;
        }

        // If we have settings from localStorage, try to use those
        if (localSettings && localSettings.id === GLOBAL_SETTINGS_ID) {
          console.log("Using backup settings from localStorage");
          try {
            // Validate and insert the localStorage settings
            if (localSettings.defaultModel && typeof localSettings.defaultModel !== 'string') {
              localSettings.defaultModel = 'qwen-qwq-32b';
            }

            await this.db!.settings.insert(localSettings);
            this.cachedSettings = localSettings;
            return localSettings;
          } catch (insertLocalError) {
            console.warn("Failed to insert localStorage settings:", insertLocalError);
            // Continue to default settings
          }
        }

        // Create default settings if none exist
        // Get the model IDs from the MODELS array to set reasonable defaults
        const visibleModelIds: string[] = [];
        try {
          // Try to access top 5 models
          // We can't import dynamically here to avoid TypeScript errors
          // So we'll use a fixed set of default models
          const defaultModelIds = [
            'anthropic/claude-3.7-sonnet',
            'anthropic/claude-3.5-sonnet',
            'openai/gpt-4o-mini',
            'openai/gpt-4o-2024-11-20',
            'google/gemini-2.0-flash-001'
          ] as string[];
          // Use our fixed set of default models
          for (const id of defaultModelIds) {
            visibleModelIds.push(id);
          }
        } catch (e) {
          console.warn("Could not load MODELS for default visibleModelIds", e);
        }

        // Default selected model (previously defaultModel)
        const defaultSelectedModel = 'anthropic/claude-3.7-sonnet';

        const defaultSettings: Settings = {
          id: GLOBAL_SETTINGS_ID,
          theme: 'system',
          apiKeys: {},
          defaultModel: defaultSelectedModel, // Keep for backward compatibility
          selectedModelId: defaultSelectedModel, // New field
          visibleModelIds: visibleModelIds.length > 0 ? visibleModelIds : [defaultSelectedModel], // New field
          preferences: {}
        };

        try {
          // Try to insert, but this might fail if another instance already inserted
          await this.db!.settings.insert(defaultSettings);
          this.cachedSettings = defaultSettings;

          // Backup to localStorage
          try {
            if (typeof window !== 'undefined' && window.localStorage) {
              window.localStorage.setItem('openagents_settings_backup', JSON.stringify(defaultSettings));
            }
          } catch (backupError) {
            console.warn("Error backing up settings to localStorage:", backupError);
          }

          return defaultSettings;
        } catch (error) {
          // If we get an error (likely a conflict error), try to fetch again
          console.log('Settings insert conflict, retrying fetch...');

          // Add a small delay to let other operations complete
          await new Promise(resolve => setTimeout(resolve, 100));

          // Try again after the delay
          try {
            const existingSettings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
            if (existingSettings) {
              const existingData = existingSettings.toJSON();
              const mutableData = toMutableSettings(existingData);
              this.cachedSettings = mutableData;

              // Backup to localStorage
              try {
                if (typeof window !== 'undefined' && window.localStorage) {
                  window.localStorage.setItem('openagents_settings_backup', JSON.stringify(existingData));
                }
              } catch (backupError) {
                console.warn("Error backing up settings to localStorage:", backupError);
              }

              return mutableData;
            }
          } catch (retryError) {
            console.error("Error on retry fetch:", retryError);
          }

          // If we have localStorage settings, use those as a fallback
          if (localSettings && localSettings.id === GLOBAL_SETTINGS_ID) {
            console.log("Using localStorage settings as fallback after fetch errors");
            this.cachedSettings = localSettings;
            return localSettings;
          }

          // Last resort - return in-memory defaults
          const lastResortDefaults = {
            id: GLOBAL_SETTINGS_ID,
            theme: 'system',
            apiKeys: {},
            defaultModel: 'qwen-qwq-32b',
            preferences: {}
          };
          this.cachedSettings = lastResortDefaults;
          return lastResortDefaults;
        }
      } catch (error) {
        // If any error occurs during fetching (including database not available),
        // fall back to returning default settings without persisting
        console.warn('Error fetching settings, using defaults:', error);
        const fallbackSettings = {
          id: GLOBAL_SETTINGS_ID,
          theme: 'system',
          apiKeys: {},
          defaultModel: 'qwen-qwq-32b',
          preferences: {}
        };
        this.cachedSettings = fallbackSettings;
        return fallbackSettings;
      } finally {
        // Always clear the initialization flags
        this.settingsInitInProgress = false;
        this.settingsInitPromise = null;
      }
    })();

    return this.settingsInitPromise;
  }

  /**
   * Update settings
   */
  async updateSettings(updates: Partial<Settings>): Promise<Settings> {
    try {
      await this.initialize();

      // Handle model selection updates
      if (updates.defaultModel || updates.selectedModelId) {
        const modelId = updates.selectedModelId || updates.defaultModel;
        if (modelId) {
          console.log(`Setting pending model update: ${modelId}`);
          this._pendingModelUpdate = modelId;

          // If updates only contains defaultModel, also update selectedModelId for new schema
          if (updates.defaultModel && !updates.selectedModelId) {
            updates = {
              ...updates,
              selectedModelId: updates.defaultModel
            };
          }

          // If updates only contains selectedModelId, also update defaultModel for backward compatibility
          if (updates.selectedModelId && !updates.defaultModel) {
            updates = {
              ...updates,
              defaultModel: updates.selectedModelId
            };
          }

          // Store the pending update in localStorage for persistence across page reloads
          try {
            if (typeof window !== 'undefined' && window.localStorage) {
              window.localStorage.setItem('openagents_pending_model', modelId);
            }
          } catch (localStorageError) {
            console.warn("Error saving pending model to localStorage:", localStorageError);
          }
        }
      }

      // Handle model visibility updates
      if (updates.visibleModelIds) {
        console.log(`Setting pending visibility update: ${updates.visibleModelIds.length} models`);
        this._pendingVisibilityUpdates = updates.visibleModelIds;

        // Store the pending update in localStorage for persistence across page reloads
        try {
          if (typeof window !== 'undefined' && window.localStorage) {
            window.localStorage.setItem('openagents_pending_visibility', JSON.stringify(updates.visibleModelIds));
          }
        } catch (localStorageError) {
          console.warn("Error saving pending visibility to localStorage:", localStorageError);
        }
      }

      // Initial merge with cached settings if available
      // This helps maintain values not included in the updates
      let combinedUpdates = { ...updates };
      if (this.cachedSettings) {
        console.log("Merging updates with cached settings");
        combinedUpdates = {
          ...this.cachedSettings,
          ...updates
        };
      }

      // Use a mutex approach with optimistic locking
      let retries = 0;
      const maxRetries = 5; // Increased retry count for better resilience

      // Start a retry loop to handle potential conflicts
      while (retries < maxRetries) {
        try {
          // First get fresh settings - needed for the latest revision
          let currentDoc = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();

          if (currentDoc) {
            // Document exists, use patch with the current revision
            try {
              // Log the schema to help diagnose issues
              try {
                const collection = this.db!.settings;
                console.log("Settings schema:", collection.schema.jsonSchema);
              } catch (e) {
                console.warn("Could not log schema:", e);
              }

              // Use patch instead of atomicUpdate
              console.log("Using patch with only schema fields:", Object.keys(updates));
              await currentDoc.patch({
                ...updates
              });

              // Fetch the updated document
              const updatedDoc = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
              if (updatedDoc) {
                const updatedData = updatedDoc.toJSON();
                const mutableData = toMutableSettings(updatedData);
                this.cachedSettings = mutableData;
                return mutableData;
              }

              // If for some reason we can't find the document after update, use the cached settings
              if (this.cachedSettings) {
                return toMutableSettings(this.cachedSettings);
              }

              // Last resort - create a default settings object
              const fallbackSettings: Settings = {
                id: GLOBAL_SETTINGS_ID,
                theme: 'system',
                apiKeys: {},
                defaultModel: updates.defaultModel || 'anthropic/claude-3.7-sonnet',
                selectedModelId: updates.selectedModelId || updates.defaultModel || 'anthropic/claude-3.7-sonnet',
                visibleModelIds: updates.visibleModelIds || [],
                preferences: {}
              };

              return fallbackSettings;
            } catch (updateError) {
              console.warn(`Update attempt ${retries + 1} failed:`, updateError);
              retries++;

              // Add an increased delay between retries to avoid race conditions
              await new Promise(resolve => setTimeout(resolve, 100 * retries));
              continue; // Try again
            }
          } else {
            // Document doesn't exist, create it
            const defaultSettings: Settings = {
              id: GLOBAL_SETTINGS_ID,
              theme: 'system',
              apiKeys: {},
              defaultModel: 'qwen-qwq-32b',
              preferences: {},
              ...updates
            };

            try {
              await this.db!.settings.insert(defaultSettings);

              // Double-check the document was inserted
              const checkDoc = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
              if (checkDoc) {
                const settingsData = checkDoc.toJSON();
                this.cachedSettings = toMutableSettings(settingsData);
                console.log("New settings document created and verified");
                return toMutableSettings(settingsData);
              } else {
                // Document not found after insert - this is unexpected
                console.error("Document not found after insert - will retry");
                retries++;
                await new Promise(resolve => setTimeout(resolve, 100 * retries));
                continue;
              }
            } catch (insertError) {
              // This likely means the document was created by another process
              console.warn(`Insert attempt ${retries + 1} failed:`, insertError);
              retries++;

              // Add a delay between retries
              await new Promise(resolve => setTimeout(resolve, 100 * retries));
              continue; // Try again
            }
          }
        } catch (dbError) {
          console.error(`Database operation failed on attempt ${retries + 1}:`, dbError);
          retries++;

          // More substantial delay for DB errors
          await new Promise(resolve => setTimeout(resolve, 150 * retries));
          continue;
        }
      }

      // If we've exhausted retries, use the recovery approach
      // But don't use the nuclear option since it's causing page refreshes
      console.warn("Exhausted retry attempts, using stable recovery method");

      try {
        // Create a safe recovery document based on what we know
        // Only include schema-compliant fields
        const recoverySettings: Settings = {
          id: GLOBAL_SETTINGS_ID,
          theme: updates.theme || 'system',
          apiKeys: updates.apiKeys || {},
          defaultModel: updates.defaultModel || 'qwen-qwq-32b',
          preferences: updates.preferences || {}
        };

        // Update our cache with what we intended to save
        this.cachedSettings = recoverySettings;
        console.log("Settings recovery successful with in-memory update");

        // Try one last time to read the actual data
        try {
          const lastDoc = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
          if (lastDoc) {
            const lastSettings = lastDoc.toJSON();
            // Only update our cache if the defaultModel in the database
            // doesn't match what the user was trying to set
            if (updates.defaultModel && lastSettings.defaultModel !== updates.defaultModel) {
              console.log("Database has defaultModel:", lastSettings.defaultModel,
                "but user wanted:", updates.defaultModel);
              // Keep our in-memory version with the user's preference
            } else {
              // Otherwise use what's in the database
              this.cachedSettings = toMutableSettings(lastSettings);
            }
          }
        } catch (finalReadError) {
          console.warn("Final read attempt failed, using recovery settings");
        }

        return this.cachedSettings;
      } catch (finalCatchAllError) {
        // Absolute last resort - return a memory object
        console.error("All database operations failed:", finalCatchAllError);
        const fallbackSettings = {
          id: GLOBAL_SETTINGS_ID,
          theme: 'system',
          apiKeys: {},
          defaultModel: updates.defaultModel || 'qwen-qwq-32b',
          preferences: {}
        };
        this.cachedSettings = fallbackSettings;
        return fallbackSettings;
      }
    } catch (error) {
      console.error('Fatal error updating settings:', error);

      // If we have cached settings, merge with updates
      if (this.cachedSettings) {
        const mergedSettings = {
          ...this.cachedSettings,
          ...updates
        };
        this.cachedSettings = mergedSettings;
        return mergedSettings;
      }

      // Last resort fallback - return a new settings object with the updates
      const fallbackSettings = {
        id: GLOBAL_SETTINGS_ID,
        theme: 'system',
        apiKeys: {},
        defaultModel: updates.defaultModel || 'qwen-qwq-32b',
        preferences: {},
        ...updates
      };

      // Update cache with fallback settings
      this.cachedSettings = fallbackSettings;
      return fallbackSettings;
    }
  }

  /**
   * Set an API key for a provider
   */
  async setApiKey(provider: string, key: string): Promise<void> {
    await this.initialize();

    // Use cached settings if available
    const settings = this.cachedSettings || await this.getSettings();
    const apiKeys = settings.apiKeys || {};

    console.log(`Setting API key for provider: ${provider}`);

    await this.updateSettings({
      apiKeys: {
        ...apiKeys,
        [provider]: key
      }
    });
  }

  /**
   * Get an API key for a provider
   */
  async getApiKey(provider: string): Promise<string | null> {
    await this.initialize();

    // Use cached settings if available
    const settings = this.cachedSettings || await this.getSettings();

    if (settings.apiKeys && provider in settings.apiKeys) {
      return settings.apiKeys[provider];
    }

    return null;
  }

  /**
   * Delete an API key for a provider
   */
  async deleteApiKey(provider: string): Promise<void> {
    await this.initialize();

    // Use cached settings if available
    const settings = this.cachedSettings || await this.getSettings();
    const apiKeys = { ...(settings.apiKeys || {}) };

    if (provider in apiKeys) {
      delete apiKeys[provider];

      console.log(`Deleting API key for provider: ${provider}`);

      await this.updateSettings({
        apiKeys
      });
    }
  }

  /**
   * Get a specific preference value
   */
  async getPreference<T>(key: string, defaultValue: T): Promise<T> {
    await this.initialize();

    // Use cached settings if available
    const settings = this.cachedSettings || await this.getSettings();

    if (settings.preferences && key in settings.preferences) {
      return settings.preferences[key] as T;
    }

    return defaultValue;
  }

  /**
   * Set a specific preference value
   */
  async setPreference<T>(key: string, value: T): Promise<void> {
    await this.initialize();

    // Use cached settings if available
    const settings = this.cachedSettings || await this.getSettings();
    const preferences = settings.preferences || {};

    console.log(`Setting preference: ${key}`);

    await this.updateSettings({
      preferences: {
        ...preferences,
        [key]: value
      }
    });
  }

  /**
   * Reset settings to defaults
   */
  async resetSettings(): Promise<Settings> {
    await this.initialize();

    console.log("Resetting settings to defaults");

    try {
      // Try to remove existing settings
      // Use bulkWrite to remove any document corruption issues
      if (this.db?.settings) {
        try {
          await (this.db.settings as any)._collection.bulkWrite({
            bulkWrites: [{
              type: 'DELETE',
              documentId: GLOBAL_SETTINGS_ID
            }]
          });
          console.log("Successfully removed settings via bulkWrite during reset");
        } catch (bulkError) {
          console.error("Error removing settings via bulkWrite:", bulkError);

          // Try the normal approach
          let settings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
          if (settings) {
            await settings.remove();
            console.log("Successfully removed existing settings document during reset");
          }
        }
      }
    } catch (removeError) {
      console.error("Error removing settings during reset:", removeError);
    }

    // Clear cache
    this.cachedSettings = null;

    // Get default visible model IDs
    const visibleModelIds: string[] = [];
    try {
      // Use fixed default models instead of dynamic import
      const defaultModelIds = [
        'anthropic/claude-3.7-sonnet',
        'anthropic/claude-3.5-sonnet',
        'openai/gpt-4o-mini',
        'openai/gpt-4o-2024-11-20',
        'google/gemini-2.0-flash-001'
      ] as string[];
      // Use our fixed set of default models
      for (const id of defaultModelIds) {
        visibleModelIds.push(id);
      }
    } catch (e) {
      console.warn("Could not load MODELS for default visibleModelIds during reset", e);
    }

    // Default selected model
    const defaultSelectedModel = 'anthropic/claude-3.7-sonnet';

    // Create default settings
    const defaultSettings: Settings = {
      id: GLOBAL_SETTINGS_ID,
      theme: 'system',
      apiKeys: {},
      defaultModel: defaultSelectedModel,
      selectedModelId: defaultSelectedModel,
      visibleModelIds: visibleModelIds.length > 0 ? visibleModelIds : [defaultSelectedModel],
      preferences: {}
    };

    // Wait for things to settle
    await new Promise(resolve => setTimeout(resolve, 100));

    try {
      await this.db!.settings.insert(defaultSettings);
      this.cachedSettings = defaultSettings;
      console.log("Settings reset successfully");
      return defaultSettings;
    } catch (insertError) {
      console.error("Error inserting default settings during reset:", insertError);
      // Return in-memory version
      this.cachedSettings = defaultSettings;
      return defaultSettings;
    }
  }

  /**
   * Clear the settings cache - useful if we need to force a refresh
   */
  clearCache(): void {
    // console.log("Clearing settings cache");
    this.cachedSettings = null;
  }

  /**
   * Select a model as the active model
   * This replaces the old defaultModel concept
   */
  async selectModel(modelId: string): Promise<Settings> {
    console.log(`Selecting model: ${modelId}`);
    try {
      const result = await this.updateSettings({
        selectedModelId: modelId,
        defaultModel: modelId // Keep for backward compatibility
      });
      return result;
    } catch (error) {
      console.error("Error selecting model:", error);
      // Return fallback settings instead of null
      return {
        id: GLOBAL_SETTINGS_ID,
        theme: 'system',
        apiKeys: {},
        defaultModel: modelId,
        selectedModelId: modelId,
        visibleModelIds: [],
        preferences: {}
      };
    }
  }

  /**
   * Show a model in the selector
   */
  async showModel(modelId: string): Promise<Settings> {
    console.log(`Showing model: ${modelId}`);
    const settings = await this.getSettings();
    const visibleModelIds = settings.visibleModelIds || [];

    // Only add if not already visible
    if (!visibleModelIds.includes(modelId)) {
      return this.updateSettings({
        visibleModelIds: [...visibleModelIds, modelId]
      });
    }

    return toMutableSettings(settings);
  }

  /**
   * Hide a model from the selector
   */
  async hideModel(modelId: string): Promise<Settings> {
    console.log(`Hiding model: ${modelId}`);
    const settings = await this.getSettings();
    const visibleModelIds = settings.visibleModelIds || [];

    // Only remove if visible
    if (visibleModelIds.includes(modelId)) {
      return this.updateSettings({
        visibleModelIds: visibleModelIds.filter(id => id !== modelId)
      });
    }

    return toMutableSettings(settings);
  }

  /**
   * Toggle a model's visibility
   */
  async toggleModelVisibility(modelId: string): Promise<Settings> {
    const settings = await this.getSettings();
    const visibleModelIds = settings.visibleModelIds || [];

    if (visibleModelIds.includes(modelId)) {
      return this.hideModel(modelId);
    } else {
      return this.showModel(modelId);
    }
  }

  /**
   * Get currently visible model IDs
   */
  async getVisibleModelIds(): Promise<string[]> {
    try {
      const settings = await this.getSettings();
      if (settings && settings.visibleModelIds) {
        return Array.isArray(settings.visibleModelIds)
          ? [...settings.visibleModelIds]
          : [];
      }
      return [];
    } catch (error) {
      console.error("Error getting visible model IDs:", error);
      return [];
    }
  }
}

// Singleton instance
export const settingsRepository = new SettingsRepository();
