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
  isAvailable: boolean;
}

// Create a default implementation that checks for the window.commandExecution object
const createDefaultExecuteCommand = () => {
  const isAvailable = typeof window !== 'undefined' && !!window.commandExecution;
  
  const executeCommand = async (command: string, options?: any) => {
    if (typeof window !== 'undefined' && window.commandExecution) {
      try {
        return await window.commandExecution.executeCommand(command, options);
      } catch (error) {
        return { 
          error: error instanceof Error ? error.message : String(error),
          command
        };
      }
    }
    return { 
      error: 'Command execution not available in this environment', 
      command 
    };
  };
  
  return { executeCommand, isAvailable };
};

const defaultContext = createDefaultExecuteCommand();

export const CommandContext = createContext<CommandContextValue>({
  executeCommand: defaultContext.executeCommand,
  isAvailable: defaultContext.isAvailable
});

export const useCommand = () => useContext(CommandContext);