import type { AssistantRuntime } from '@openagentsinc/assistant-ui-runtime';

// Mock ACP runtime for testing/Storybook
export function useAcpRuntime(): AssistantRuntime {
  // Return a minimal mock runtime for development
  return {
    // Add minimal runtime implementation here if needed for Storybook
  } as AssistantRuntime;
}

