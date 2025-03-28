// Global declarations for command execution
import type { CommandExecutionOptions, CommandExecutionResult } from '@openagents/core';

declare global {
  // Command execution function exposed by @openagents/core
  var executeCommandFromCore: (
    command: string, 
    options?: CommandExecutionOptions
  ) => Promise<CommandExecutionResult | { error: string }>;
}

export {};