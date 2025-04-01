import { getDatabase } from '../database';
import { Settings, Database } from '../types';

// Global settings ID
const GLOBAL_SETTINGS_ID = 'global';

/**
 * Repository for settings operations
 */
export class SettingsRepository {
  private db: Database | null = null;
  
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
    await this.initialize();
    
    try {
      // Try to find existing settings
      const settings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
      
      if (settings) {
        return settings.toJSON();
      }
      
      // Create default settings if none exist
      const defaultSettings: Settings = {
        id: GLOBAL_SETTINGS_ID,
        theme: 'system',
        apiKeys: {},
        defaultModel: 'claude-3-sonnet-20240229',
        preferences: {}
      };
      
      try {
        // Try to insert, but this might fail if another instance already inserted
        await this.db!.settings.insert(defaultSettings);
        return defaultSettings;
      } catch (error) {
        // If we get an error (likely a conflict error), try to fetch again
        console.log('Settings insert conflict, retrying fetch...');
        const existingSettings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
        
        if (existingSettings) {
          return existingSettings.toJSON();
        }
        
        // If still no settings (very unlikely), return the default
        return defaultSettings;
      }
    } catch (error) {
      // If any error occurs during fetching (including database not available),
      // fall back to returning default settings without persisting
      console.warn('Error fetching settings, using defaults:', error);
      return {
        id: GLOBAL_SETTINGS_ID,
        theme: 'system',
        apiKeys: {},
        defaultModel: 'claude-3-sonnet-20240229',
        preferences: {}
      };
    }
  }
  
  /**
   * Update settings
   */
  async updateSettings(updates: Partial<Settings>): Promise<Settings> {
    try {
      await this.initialize();
      
      try {
        // Get existing settings
        let settings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
        
        if (settings) {
          // Update existing settings
          try {
            await settings.update({
              $set: updates
            });
            
            return settings.toJSON();
          } catch (error) {
            console.log('Settings update conflict, retrying...');
            // If update fails, get settings again and retry
            settings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
            
            if (settings) {
              await settings.update({
                $set: updates
              });
              
              return settings.toJSON();
            }
          }
        }
        
        // If no settings found or update failed, create new settings with updates
        const newSettings: Settings = {
          id: GLOBAL_SETTINGS_ID,
          theme: 'system',
          apiKeys: {},
          defaultModel: 'claude-3-sonnet-20240229',
          preferences: {},
          ...updates
        };
        
        try {
          await this.db!.settings.insert(newSettings);
          return newSettings;
        } catch (error) {
          // If insert fails (likely conflict), get latest settings
          console.log('Settings insert conflict during update, fetching latest...');
          const latestSettings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
          
          if (latestSettings) {
            return latestSettings.toJSON();
          }
          
          return newSettings;
        }
      } catch (error) {
        console.error('Database error during settings update:', error);
        
        // Return current settings merged with updates as fallback
        const currentSettings = await this.getSettings();
        return {
          ...currentSettings,
          ...updates
        };
      }
    } catch (error) {
      console.error('Fatal error updating settings:', error);
      
      // Last resort fallback - return a new settings object with the updates
      return {
        id: GLOBAL_SETTINGS_ID,
        theme: 'system',
        apiKeys: {},
        defaultModel: 'claude-3-sonnet-20240229',
        preferences: {},
        ...updates
      };
    }
  }
  
  /**
   * Set an API key for a provider
   */
  async setApiKey(provider: string, key: string): Promise<void> {
    await this.initialize();
    
    const settings = await this.getSettings();
    const apiKeys = settings.apiKeys || {};
    
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
    
    const settings = await this.getSettings();
    
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
    
    const settings = await this.getSettings();
    const apiKeys = { ...(settings.apiKeys || {}) };
    
    if (provider in apiKeys) {
      delete apiKeys[provider];
      
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
    
    const settings = await this.getSettings();
    
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
    
    const settings = await this.getSettings();
    const preferences = settings.preferences || {};
    
    await this.updateSettings({
      preferences: {
        ...preferences,
        [key]: value
      }
    });
  }
}

// Singleton instance
export const settingsRepository = new SettingsRepository();