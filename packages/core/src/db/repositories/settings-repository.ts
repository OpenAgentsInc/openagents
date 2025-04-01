import { getDatabase } from '../database';
import { Settings, Database } from '../types';

// Global settings ID
const GLOBAL_SETTINGS_ID = 'global';

/**
 * Repository for settings operations
 */
export class SettingsRepository {
  private db: Database | null = null;
  private cachedSettings: Settings | null = null;
  private settingsInitInProgress = false;
  private settingsInitPromise: Promise<Settings> | null = null;

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
    // Return cached settings if available to prevent repeated DB access
    if (this.cachedSettings) {
      return { ...this.cachedSettings };
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
        // Try to find existing settings
        const settings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();

        if (settings) {
          const settingsData = settings.toJSON();
          this.cachedSettings = settingsData;
          return settingsData;
        }

        // Create default settings if none exist
        const defaultSettings: Settings = {
          id: GLOBAL_SETTINGS_ID,
          theme: 'system',
          apiKeys: {},
          defaultModel: 'claude-3-5-sonnet-20240620',
          preferences: {}
        };

        try {
          // Try to insert, but this might fail if another instance already inserted
          await this.db!.settings.insert(defaultSettings);
          this.cachedSettings = defaultSettings;
          return defaultSettings;
        } catch (error) {
          // If we get an error (likely a conflict error), try to fetch again
          console.log('Settings insert conflict, retrying fetch...');
          
          // Add a small delay to let other operations complete
          await new Promise(resolve => setTimeout(resolve, 50));
          
          const existingSettings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();

          if (existingSettings) {
            const existingData = existingSettings.toJSON();
            this.cachedSettings = existingData;
            return existingData;
          }

          // If still no settings (very unlikely), return the default
          this.cachedSettings = defaultSettings;
          return defaultSettings;
        }
      } catch (error) {
        // If any error occurs during fetching (including database not available),
        // fall back to returning default settings without persisting
        console.warn('Error fetching settings, using defaults:', error);
        const fallbackSettings = {
          id: GLOBAL_SETTINGS_ID,
          theme: 'system',
          apiKeys: {},
          defaultModel: 'claude-3-5-sonnet-20240620',
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

      try {
        // Get existing settings - use cached version if available
        let currentSettings = this.cachedSettings;
        if (!currentSettings) {
          currentSettings = await this.getSettings();
        }

        // Get settings from database
        let settings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();

        if (settings) {
          // Update existing settings
          try {
            await settings.update({
              $set: updates
            });

            const updatedSettings = settings.toJSON();
            // Update cache with the new settings
            this.cachedSettings = updatedSettings;
            console.log("Settings updated successfully:", updates);
            return updatedSettings;
          } catch (error) {
            console.log('Settings update conflict, retrying...');
            // Add a small delay before retry
            await new Promise(resolve => setTimeout(resolve, 50));
            
            // If update fails, get settings again and retry
            settings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();

            if (settings) {
              await settings.update({
                $set: updates
              });

              const updatedSettings = settings.toJSON();
              // Update cache with the new settings
              this.cachedSettings = updatedSettings;
              console.log("Settings updated successfully on retry:", updates);
              return updatedSettings;
            }
          }
        }

        // If no settings found or update failed, create new settings with updates
        const newSettings: Settings = {
          id: GLOBAL_SETTINGS_ID,
          theme: 'system',
          apiKeys: {},
          defaultModel: 'claude-3-5-sonnet-20240620',
          preferences: {},
          ...updates
        };

        try {
          await this.db!.settings.insert(newSettings);
          // Update cache with the new settings
          this.cachedSettings = newSettings;
          console.log("New settings created:", newSettings);
          return newSettings;
        } catch (error) {
          // If insert fails (likely conflict), get latest settings
          console.log('Settings insert conflict during update, fetching latest...');
          
          // Add a small delay before retry
          await new Promise(resolve => setTimeout(resolve, 50));
          
          const latestSettings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();

          if (latestSettings) {
            const latestData = latestSettings.toJSON();
            // Update cache with latest settings
            this.cachedSettings = latestData;
            return latestData;
          }

          // Last resort - update cache with new settings even if save failed
          this.cachedSettings = newSettings;
          return newSettings;
        }
      } catch (error) {
        console.error('Database error during settings update:', error);

        // Return current settings merged with updates as fallback
        const currentSettings = await this.getSettings();
        const mergedSettings = {
          ...currentSettings,
          ...updates
        };
        // Update cache with merged settings
        this.cachedSettings = mergedSettings;
        return mergedSettings;
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
        defaultModel: 'claude-3-5-sonnet-20240620',
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
   * Clear the settings cache - useful if we need to force a refresh
   */
  clearCache(): void {
    console.log("Clearing settings cache");
    this.cachedSettings = null;
  }
}

// Singleton instance
export const settingsRepository = new SettingsRepository();
