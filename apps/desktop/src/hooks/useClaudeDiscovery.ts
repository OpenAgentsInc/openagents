import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface CommandResult<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export const useClaudeDiscovery = () => {
  const [claudeStatus, setClaudeStatus] = useState<string>("Not initialized");
  const [isDiscoveryLoading, setIsDiscoveryLoading] = useState(false);

  const discoverClaude = useCallback(async () => {
    setIsDiscoveryLoading(true);
    try {
      const result = await invoke<CommandResult<string>>("discover_claude");
      if (result.success && result.data) {
        setClaudeStatus(`Claude found at: ${result.data}`);
      } else {
        setClaudeStatus(`Error: ${result.error || "Unknown error"}`);
        console.error("Discovery failed:", result.error);
      }
    } catch (error) {
      setClaudeStatus(`Error: ${error}`);
      console.error("Discovery error:", error);
    }
    setIsDiscoveryLoading(false);
  }, []);

  return { claudeStatus, isDiscoveryLoading, discoverClaude };
};