import React, { useState } from 'react';
import { Chat } from '@openagents/ui';
import { useCommand } from '../helpers/ipc/command/command-context';
import { useChat } from '@openagents/core';

/**
 * Chat component that adds local command execution support
 */
export const ChatWithCommandSupport: React.FC = () => {
  const [commandStatus, setCommandStatus] = useState<{
    isExecuting: boolean;
    currentCommand: string | null;
  }>({
    isExecuting: false,
    currentCommand: null
  });
  
  // Get the command execution context
  const { executeCommand } = useCommand();
  
  // Create a chat hook with command execution enabled
  const chat = useChat({
    localCommandExecution: true,
    commandOptions: {
      timeout: 30000 // 30 second timeout
    },
    onCommandStart: (command) => {
      console.log(`Executing command: ${command}`);
      setCommandStatus({
        isExecuting: true,
        currentCommand: command
      });
    },
    onCommandComplete: (command, result) => {
      console.log(`Command completed: ${command}`, result);
      setCommandStatus({
        isExecuting: false,
        currentCommand: null
      });
    }
  });
  
  return (
    <div className="flex flex-col h-full relative">
      {/* Command execution status indicator */}
      {commandStatus.isExecuting && commandStatus.currentCommand && (
        <div className="absolute top-0 left-0 right-0 bg-yellow-500 text-black p-2 z-10">
          Executing: {commandStatus.currentCommand}
        </div>
      )}
      
      {/* Standard chat UI */}
      <Chat
        messages={chat.messages}
        onSendMessage={chat.append}
        isLoading={chat.isLoading}
      />
    </div>
  );
};