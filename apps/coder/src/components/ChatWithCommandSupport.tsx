import React, { useState, useEffect, useMemo } from 'react';
import { Platform } from 'react-native';
import { MessageList, MessageInput } from '@openagents/ui';
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
  const { executeCommand, isAvailable } = useCommand();

  // Create a chat hook with command execution enabled
  const chat = useChat({
    localCommandExecution: true,
    commandOptions: {
      timeout: 30000 // 30 second timeout
    },
    onCommandStart: (command) => {
      console.log(`🚀 CODER: Executing command: ${command}`);
      setCommandStatus({
        isExecuting: true,
        currentCommand: command
      });
    },
    onCommandComplete: (command, result) => {
      console.log(`✅ CODER: Command completed: ${command}`, result);
      setCommandStatus({
        isExecuting: false,
        currentCommand: null
      });
    }
  });

  // Create a ref outside the effect to ensure we only run the test once
  const hasRunTest = React.useRef(false);

  // Test command execution on component mount
  useEffect(() => {
    const testCommandExecution = async () => {
      // Skip if already run
      if (hasRunTest.current) {
        console.log('🧪 CODER: Command execution test already run, skipping');
        return;
      }

      // Mark as run immediately to prevent multiple executions
      hasRunTest.current = true;

      console.log('🧪 CODER: Testing command execution...');
      console.log('🧪 CODER: Command execution available via context:', isAvailable);

      try {
        // Log window.commandExecution availability
        if (typeof window !== 'undefined') {
          console.log('🧪 CODER: window.commandExecution available:', !!window.commandExecution);
        }

        // For now, just log that we're skipping test execution
        // The testCommandExecution method in useChat.ts exists but isn't exported in the type
        console.log('🧪 CODER: Skipping command execution test - method not available in type');

        // We'll skip testing via command context to avoid infinite loops
        console.log('🧪 CODER: Skipping command context test to avoid potential loops');
      } catch (error) {
        console.error('🧪 CODER: Command test error:', error);
      }
    };

    testCommandExecution();
  }, [chat, isAvailable]); // Removed executeCommand dependency

  const handleSubmit = (message: string) => {
    console.log('📝 CODER: Message submitted:', message);
    chat.append({
      content: message,
      role: 'user'
    });
  };

  // Process messages to ensure command results are rendered properly
  const processedMessages = useMemo(() => {
    // Only log on changes, not every render
    if (process.env.NODE_ENV !== 'production') {
      // console.log(`📊 CODER: Chat messages count: ${chat.messages.length}`);
    }

    return chat.messages;
  }, [chat.messages]);

  return (
    <div className="flex flex-col h-full bg-black">
      {/* Command execution status indicator */}
      {commandStatus.isExecuting && commandStatus.currentCommand && (
        <div className="bg-yellow-500 text-black p-2 z-10">
          Executing: {commandStatus.currentCommand}
        </div>
      )}

      {/* Main content with absolute positioned input */}
      <div className="flex-1 relative h-full overflow-hidden">
        {/* Message list with bottom padding to ensure messages don't go behind input */}
        <div className="absolute top-0 left-0 right-0 bottom-16 overflow-auto pb-4">
          <div className="h-px bg-gray-800 w-full" />
          <MessageList messages={processedMessages} />
        </div>
        
        {/* Input fixed at the bottom */}
        <div className="absolute bottom-0 left-0 right-0 bg-black border-t border-gray-800 py-2 h-16">
          <MessageInput maxRows={8} onSubmit={handleSubmit} />
        </div>
      </div>
    </div>
  );
};

// Styles now use Tailwind classes directly in the JSX
