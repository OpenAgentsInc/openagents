import React, { useState } from 'react';
import { View, StyleSheet, SafeAreaView, KeyboardAvoidingView, Platform, Text, TextInput } from 'react-native';
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
  
  const handleSubmit = (message: string) => {
    console.log('Message submitted:', message);
    chat.append({
      content: message,
      role: 'user'
    });
  };
  
  return (
    <View style={styles.container}>
      {/* Command execution status indicator */}
      {commandStatus.isExecuting && commandStatus.currentCommand && (
        <View style={styles.commandStatus}>
          <Text style={styles.commandText}>
            Executing: {commandStatus.currentCommand}
          </Text>
        </View>
      )}
      
      {/* Use the Chat component directly */}
      <View style={styles.chatContainer}>
        <Chat />
      </View>
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
    position: 'relative',
  },
  chatContainer: {
    flex: 1,
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