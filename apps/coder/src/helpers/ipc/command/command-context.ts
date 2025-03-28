import { createContext, useContext } from 'react';

export interface CommandExecutionResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  command: string;
}

export interface CommandExecutionError {
  error: string;
  command: string;
}

export type CommandResult = CommandExecutionResult | CommandExecutionError;

export interface CommandContextValue {
  executeCommand: (
    command: string, 
    options?: { 
      cwd?: string; 
      timeout?: number; 
      env?: Record<string, string>;
    }
  ) => Promise<CommandResult>;
}

export const CommandContext = createContext<CommandContextValue>({
  executeCommand: async () => ({ error: 'Command context not initialized', command: '' }),
});

export const useCommand = () => useContext(CommandContext);