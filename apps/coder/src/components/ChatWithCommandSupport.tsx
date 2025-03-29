import React, { useState, useEffect, useMemo } from 'react';
import { View, StyleSheet, SafeAreaView, KeyboardAvoidingView, Platform, Text } from 'react-native';
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
    <SafeAreaView style={styles.container}>
      {/* Command execution status indicator */}
      {commandStatus.isExecuting && commandStatus.currentCommand && (
        <View style={styles.commandStatus}>
          <Text style={styles.commandText}>
            Executing: {commandStatus.currentCommand}
          </Text>
        </View>
      )}

      {/* Use MessageList and MessageInput directly with our configured chat instance */}
      <KeyboardAvoidingView
        behavior={Platform.OS === 'ios' ? 'padding' : undefined}
        style={styles.keyboardAvoidingView}
      >
        <View style={styles.inner}>
          <View style={styles.topBorder} />
          {/* Use our processed messages with command execution results */}
          <MessageList messages={processedMessages} />
          <MessageInput maxRows={8} onSubmit={handleSubmit} />
        </View>
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
    position: 'relative',
  },
  keyboardAvoidingView: {
    flex: 1,
  },
  inner: {
    flex: 1,
    position: 'relative',
  },
  topBorder: {
    height: 1,
    backgroundColor: '#333',
    width: '100%',
  },
  commandStatus: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    backgroundColor: '#f0ad4e',
    zIndex: 10,
    padding: 8,
  },
  commandText: {
    color: '#000',
  }
});
