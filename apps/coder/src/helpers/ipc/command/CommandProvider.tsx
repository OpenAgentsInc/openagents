import React, { useCallback, ReactNode, useState, useEffect } from 'react';
import { CommandContext, CommandResult } from './command-context';

// The declaration for Window.commandExecution is now in types.d.ts

interface CommandProviderProps {
  children: ReactNode;
}

export const CommandProvider: React.FC<CommandProviderProps> = ({ children }) => {
  const [isElectron, setIsElectron] = useState(false);
  
  useEffect(() => {
    // Check if we're running in Electron with the command execution API
    setIsElectron(!!window.commandExecution);
  }, []);
  
  const executeCommand = useCallback(async (
    command: string, 
    options?: { 
      cwd?: string; 
      timeout?: number; 
      env?: Record<string, string>;
    }
  ): Promise<CommandResult> => {
    // If running in Electron, use the native API
    if (window.commandExecution) {
      return window.commandExecution.executeCommand(command, options);
    }
    
    // If not in Electron, return an error
    return {
      error: 'Command execution is only available in the Electron app',
      command
    };
  }, [isElectron]);
  
  const contextValue = {
    executeCommand,
    isAvailable: isElectron
  };
  
  return (
    <CommandContext.Provider value={contextValue}>
      {children}
    </CommandContext.Provider>
  );
};