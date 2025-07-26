import { useState, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useAppStore } from '@/stores/appStore';

interface CommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export const useAppInitialization = () => {
  const { isAppInitialized, setAppInitialized, claudeStatus, setClaudeStatus } = useAppStore();
  const initializationTimeoutRef = useRef<NodeJS.Timeout>();

  const initializeApp = useCallback(async () => {
    console.log('🏁 [APP-INIT] Starting app initialization...');
    try {
      console.log('🔍 [APP-INIT] Step 1: Discovering Claude Code...');
      const result = await invoke<CommandResult<string>>("discover_claude");
      
      if (result.success && result.data) {
        setClaudeStatus(`Claude found at: ${result.data}`);
        console.log('✅ [APP-INIT] Claude Code discovered successfully');
      } else {
        setClaudeStatus(`Error: ${result.error || "Unknown error"}`);
        console.error('❌ [APP-INIT] Claude Code discovery failed:', result.error);
      }
      
      console.log('✅ [APP-INIT] App initialization complete!');
      console.log('🔄 [APP-INIT] Setting global isAppInitialized to true');
      setAppInitialized(true);
    } catch (error) {
      console.error('💥 [APP-INIT] Fatal error during initialization:', error);
      setClaudeStatus(`Fatal error: ${error}`);
      setAppInitialized(true);
    }
  }, [setAppInitialized, setClaudeStatus]);

  useEffect(() => {
    initializationTimeoutRef.current = setTimeout(() => {
      if (!isAppInitialized) {
        console.log('🚀 [APP-INIT] Initial timeout reached, starting initialization');
        initializeApp();
      }
    }, 100);

    return () => {
      if (initializationTimeoutRef.current) {
        clearTimeout(initializationTimeoutRef.current);
      }
    };
  }, [isAppInitialized, initializeApp]);

  return {
    isAppInitialized,
    claudeStatus,
    initializeApp,
  };
};