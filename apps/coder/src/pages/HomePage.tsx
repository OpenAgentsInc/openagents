import React, { useState, useEffect } from 'react';
import { useSettings, DEFAULT_SYSTEM_PROMPT } from '@openagents/core';
import { ModelProvider } from '@/providers/ModelProvider';
import { ApiKeyProvider } from '@/providers/ApiKeyProvider';
import { ChatStateProvider } from '@/providers/ChatStateProvider';
import { MainLayout } from '@/components/layout/MainLayout';

export default function HomePage() {
  // Get settings
  const { clearSettingsCache } = useSettings();
  
  // State for system prompt
  const [systemPrompt, setSystemPrompt] = useState<string>("");
  
  // Initialize database and load system prompt
  useEffect(() => {
    clearSettingsCache();
    
    // Initialize database early
    (async () => {
      try {
        // Import directly here to avoid circular dependencies
        const db = await import('@openagents/core/src/db/database');
        await db.getDatabase();
        console.log("Database initialized on startup");
        
        // Load system prompt
        const { settingsRepository } = await import('@openagents/core/src/db/repositories');
        const savedPrompt = await settingsRepository.getPreference("defaultSystemPrompt", DEFAULT_SYSTEM_PROMPT);
        setSystemPrompt(savedPrompt);
        console.log("Loaded system prompt:", savedPrompt === DEFAULT_SYSTEM_PROMPT ? "Using default prompt" : "Custom prompt loaded");
      } catch (error) {
        console.error("Failed to initialize database or load system prompt:", error);
        setSystemPrompt(DEFAULT_SYSTEM_PROMPT);
      }
    })();
  }, [clearSettingsCache]);
  
  return (
    <ModelProvider>
      <ApiKeyProvider>
        <ChatStateProvider systemPrompt={systemPrompt}>
          <MainLayout />
        </ChatStateProvider>
      </ApiKeyProvider>
    </ModelProvider>
  );
}