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
    
    await this.db!.settings.insert(defaultSettings);
    return defaultSettings;
  }
  
  /**
   * Update settings
   */
  async updateSettings(updates: Partial<Settings>): Promise<Settings> {
    await this.initialize();
    
    // Get existing settings
    let settings = await this.db!.settings.findOne(GLOBAL_SETTINGS_ID).exec();
    
    if (settings) {
      // Update existing settings
      await settings.update({
        $set: updates
      });
      
      return settings.toJSON();
    } else {
      // Create new settings with updates
      const newSettings: Settings = {
        id: GLOBAL_SETTINGS_ID,
        ...updates
      };
      
      await this.db!.settings.insert(newSettings);
      return newSettings;
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