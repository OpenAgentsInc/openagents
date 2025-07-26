import { useState, useRef, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface CommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export const useAppInitialization = () => {
  const [isAppInitialized, setIsAppInitialized] = useState(false);
  const [claudeStatus, setClaudeStatus] = useState<string>("Not initialized");
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
      setIsAppInitialized(true);
    } catch (error) {
      console.error('💥 [APP-INIT] Fatal error during initialization:', error);
      setClaudeStatus(`Fatal error: ${error}`);
      setIsAppInitialized(true);
    }
  }, []);

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