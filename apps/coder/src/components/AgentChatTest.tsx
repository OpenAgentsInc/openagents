import React, { useState, useEffect } from 'react';
import { View, Text, StyleSheet, Button, TextInput, ScrollView, Platform } from 'react-native';
import { useChat } from '@openagents/core';

/**
 * Test component for Cloudflare Agent integration
 */
export const AgentChatTest: React.FC = () => {
  // State for input message
  const [message, setMessage] = useState('');
  
  // State for agent configuration
  const [agentConfig, setAgentConfig] = useState({
    agentId: 'coder-agent',
    agentName: 'test-instance',
    serverUrl: 'https://agents.openagents.com'
  });
  
  // Toggle between agent and local chat
  const [useAgent, setUseAgent] = useState(false);
  
  // Command execution test results
  const [testResults, setTestResults] = useState<any>(null);
  
  // Create the chat instance with agent support when enabled
  const chat = useChat(useAgent ? {
    // Agent configuration
    agentId: agentConfig.agentId,
    agentName: agentConfig.agentName,
    agentServerUrl: agentConfig.serverUrl,
    
    // Project context for the Coder Agent
    agentOptions: {
      projectContext: {
        repoOwner: 'OpenAgentsInc',
        repoName: 'openagents',
        branch: 'main'
      }
    },
    
    // Callbacks for connection status
    onAgentConnectionChange: (connected) => {
      console.log(`🔌 AGENT-TEST: Connection status changed: ${connected ? 'connected' : 'disconnected'}`);
    },
    
    // Enable local fallback
    localCommandExecution: true,
    
    // Command handlers
    onCommandStart: (cmd) => console.log(`⚙️ AGENT-TEST: Command started: ${cmd}`),
    onCommandComplete: (cmd, result) => console.log(`✅ AGENT-TEST: Command completed: ${cmd}`, result)
  } : {
    // Standard local chat when agent is disabled
    localCommandExecution: true
  });
  
  // Test command execution capabilities
  const runCommandTest = async () => {
    try {
      // Use type assertion to access the method (it exists at runtime)
      const testFn = (chat as any).testCommandExecution;
      if (typeof testFn === 'function') {
        const results = await testFn();
        setTestResults(results);
        // console.log('🧪 AGENT-TEST: Command test results:', results);
      } else {
        // console.error('🧪 AGENT-TEST: testCommandExecution not available');
        setTestResults({ error: 'testCommandExecution method not available' });
      }
    } catch (error) {
      // console.error('🧪 AGENT-TEST: Error testing commands:', error);
      setTestResults({ error: String(error) });
    }
  };
  
  // Handle message submission
  const handleSendMessage = () => {
    if (!message.trim()) return;
    
    // console.log(`📤 AGENT-TEST: Sending message: ${message}`);
    chat.append({
      role: 'user',
      content: message
    });
    
    setMessage('');
  };
  
  // Execute a test command
  const executeTestCommand = async () => {
    try {
      // Use the unified command execution method that automatically routes
      // to either agent or local execution based on connection state
      const executeCommand = (chat as any).executeCommand;
      if (typeof executeCommand === 'function') {
        console.log('⚙️ AGENT-TEST: Executing command through unified executor');
        const result = await executeCommand('ls -la');
        console.log('✅ AGENT-TEST: Command execution result:', result);
      } else {
        console.log('⚙️ AGENT-TEST: Unified executor not available, using message-based command');
        // Fall back to adding a command in the message that will be executed locally
        chat.append({
          role: 'user',
          content: 'Run this command: <<ls -la>>'
        });
      }
    } catch (error) {
      console.error('❌ AGENT-TEST: Command execution error:', error);
    }
  };
  
  return (
    <View style={styles.container}>
      <View style={styles.header}>
        <Text style={styles.title}>Agent Chat Test</Text>
        <View style={styles.toggleContainer}>
          <Text style={styles.toggleLabel}>Use Agent:</Text>
          <Button
            title={useAgent ? 'Enabled' : 'Disabled'}
            onPress={() => setUseAgent(!useAgent)}
            color={useAgent ? '#4caf50' : '#f44336'}
          />
        </View>
      </View>
      
      {useAgent && (
        <View style={styles.configPanel}>
          <Text style={styles.sectionTitle}>Agent Configuration</Text>
          <View style={styles.inputRow}>
            <Text style={styles.inputLabel}>Agent ID:</Text>
            <TextInput
              style={styles.configInput}
              value={agentConfig.agentId}
              onChangeText={(text) => setAgentConfig({...agentConfig, agentId: text})}
              placeholder="coder-agent"
            />
          </View>
          <View style={styles.inputRow}>
            <Text style={styles.inputLabel}>Agent Name:</Text>
            <TextInput
              style={styles.configInput}
              value={agentConfig.agentName}
              onChangeText={(text) => setAgentConfig({...agentConfig, agentName: text})}
              placeholder="default"
            />
          </View>
          <View style={styles.inputRow}>
            <Text style={styles.inputLabel}>Server URL:</Text>
            <TextInput
              style={styles.configInput}
              value={agentConfig.serverUrl}
              onChangeText={(text) => setAgentConfig({...agentConfig, serverUrl: text})}
              placeholder="https://agents.openagents.com"
            />
          </View>
        </View>
      )}
      
      <View style={styles.statusPanel}>
        <Text style={styles.sectionTitle}>Connection Status</Text>
        <View style={styles.statusRow}>
          <Text style={styles.statusLabel}>Using Agent:</Text>
          <Text style={[styles.statusValue, {color: useAgent ? '#4caf50' : '#f44336'}]}>
            {useAgent ? 'Yes' : 'No'}
          </Text>
        </View>
        {useAgent && (
          <>
            <View style={styles.statusRow}>
              <Text style={styles.statusLabel}>Agent Connected:</Text>
              <Text style={[styles.statusValue, {color: chat.agentConnection?.isConnected ? '#4caf50' : '#f44336'}]}>
                {chat.agentConnection?.isConnected ? 'Yes' : 'No'}
              </Text>
            </View>
            <View style={styles.statusRow}>
              <Text style={styles.statusLabel}>Agent Type:</Text>
              <Text style={styles.statusValue}>
                {agentConfig.agentId}
              </Text>
            </View>
            <View style={styles.statusRow}>
              <Text style={styles.statusLabel}>Instance Name:</Text>
              <Text style={styles.statusValue}>
                {agentConfig.agentName}
              </Text>
            </View>
          </>
        )}
        <View style={styles.statusRow}>
          <Text style={styles.statusLabel}>Local Commands:</Text>
          <Text style={[styles.statusValue, {color: (chat as any).isCommandExecutionEnabled ? '#4caf50' : '#f44336'}]}>
            {(chat as any).isCommandExecutionEnabled ? 'Enabled' : 'Disabled'}
          </Text>
        </View>
        <View style={styles.buttonRow}>
          <Button 
            title="Test Commands" 
            onPress={runCommandTest} 
          />
          <Button 
            title="Run Test Command" 
            onPress={executeTestCommand}
          />
        </View>
      </View>
      
      {testResults && (
        <View style={styles.resultsPanel}>
          <Text style={styles.sectionTitle}>Test Results</Text>
          <ScrollView style={styles.resultsScroll}>
            <Text style={styles.resultText}>
              {JSON.stringify(testResults, null, 2)}
            </Text>
          </ScrollView>
        </View>
      )}
      
      <View style={styles.chatPanel}>
        <Text style={styles.sectionTitle}>Chat Messages ({chat.messages.length})</Text>
        <ScrollView style={styles.messagesContainer}>
          {chat.messages.map((msg) => (
            <View 
              key={msg.id} 
              style={[
                styles.message, 
                msg.role === 'user' ? styles.userMessage : styles.assistantMessage
              ]}
            >
              <Text style={styles.messageRole}>{msg.role}</Text>
              <Text style={styles.messageContent}>{msg.content}</Text>
            </View>
          ))}
        </ScrollView>
        <View style={styles.inputContainer}>
          <TextInput
            style={styles.input}
            value={message}
            onChangeText={setMessage}
            placeholder="Type a message..."
            placeholderTextColor="#999"
            multiline
          />
          <Button title="Send" onPress={handleSendMessage} />
        </View>
      </View>
    </View>
  );
};

const styles = StyleSheet.create({
  container: {
    flex: 1,
    padding: 16,
    backgroundColor: '#000',
    color: '#fff',
    fontFamily: 'Berkeley Mono, monospace',
  },
  header: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 16,
  },
  title: {
    fontSize: 20,
    fontWeight: 'bold',
    color: '#fff',
    fontFamily: 'Berkeley Mono, monospace',
  },
  toggleContainer: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  toggleLabel: {
    marginRight: 8,
    color: '#fff',
    fontFamily: 'Berkeley Mono, monospace',
  },
  configPanel: {
    backgroundColor: '#111',
    padding: 16,
    borderRadius: 8,
    marginBottom: 16,
    borderWidth: 1,
    borderColor: '#333',
    ...Platform.select({
      web: {
        boxShadow: '0 2px 4px rgba(0,0,0,0.3)',
      },
    }),
  },
  sectionTitle: {
    fontWeight: 'bold',
    fontSize: 16,
    marginBottom: 8,
    color: '#fff',
    fontFamily: 'Berkeley Mono, monospace',
  },
  inputRow: {
    flexDirection: 'row',
    alignItems: 'center',
    marginBottom: 8,
  },
  inputLabel: {
    width: 100,
    color: '#ccc',
    fontFamily: 'Berkeley Mono, monospace',
  },
  configInput: {
    flex: 1,
    borderWidth: 1,
    borderColor: '#333',
    padding: 8,
    borderRadius: 4,
    backgroundColor: '#222',
    color: '#fff',
    fontFamily: 'Berkeley Mono, monospace',
  },
  statusPanel: {
    backgroundColor: '#111',
    padding: 16,
    borderRadius: 8,
    marginBottom: 16,
    borderWidth: 1,
    borderColor: '#333',
    ...Platform.select({
      web: {
        boxShadow: '0 2px 4px rgba(0,0,0,0.3)',
      },
    }),
  },
  statusRow: {
    flexDirection: 'row',
    alignItems: 'center',
    marginBottom: 8,
  },
  statusLabel: {
    width: 120,
    color: '#ccc',
    fontFamily: 'Berkeley Mono, monospace',
  },
  statusValue: {
    fontWeight: 'bold',
    color: '#fff',
    fontFamily: 'Berkeley Mono, monospace',
  },
  buttonRow: {
    flexDirection: 'row',
    justifyContent: 'space-around',
    marginTop: 8,
  },
  resultsPanel: {
    backgroundColor: '#111',
    padding: 16,
    borderRadius: 8,
    marginBottom: 16,
    maxHeight: 200,
    borderWidth: 1,
    borderColor: '#333',
    ...Platform.select({
      web: {
        boxShadow: '0 2px 4px rgba(0,0,0,0.3)',
      },
    }),
  },
  resultsScroll: {
    backgroundColor: '#222',
    padding: 8,
    borderRadius: 4,
  },
  resultText: {
    fontFamily: 'Berkeley Mono, monospace',
    fontSize: 12,
    color: '#aaa',
  },
  chatPanel: {
    flex: 1,
    backgroundColor: '#111',
    padding: 16,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#333',
    display: 'flex',
    flexDirection: 'column',
    ...Platform.select({
      web: {
        boxShadow: '0 2px 4px rgba(0,0,0,0.3)',
      },
    }),
  },
  messagesContainer: {
    flex: 1,
    marginBottom: 16,
    overflow: 'auto',
    maxHeight: 'calc(100% - 60px)', // Reserve space for input
  },
  message: {
    marginBottom: 12,
    padding: 12,
    borderRadius: 8,
  },
  userMessage: {
    backgroundColor: '#1a1a2e',
    alignSelf: 'flex-end',
    marginLeft: 50,
    borderWidth: 1,
    borderColor: '#333',
  },
  assistantMessage: {
    backgroundColor: '#1a2c39',
    alignSelf: 'flex-start',
    marginRight: 50,
    borderWidth: 1,
    borderColor: '#333',
  },
  messageRole: {
    fontWeight: 'bold',
    marginBottom: 4,
    color: '#ccc',
    fontFamily: 'Berkeley Mono, monospace',
  },
  messageContent: {
    fontSize: 14,
    color: '#fff',
    fontFamily: 'Berkeley Mono, monospace',
  },
  inputContainer: {
    flexDirection: 'row',
    alignItems: 'center',
    minHeight: 60,
    borderTopWidth: 1,
    borderTopColor: '#333',
    paddingTop: 8,
    position: 'sticky',
    bottom: 0,
    backgroundColor: '#111',
  },
  input: {
    flex: 1,
    borderWidth: 1,
    borderColor: '#333',
    borderRadius: 8,
    padding: 8,
    marginRight: 8,
    minHeight: 40,
    maxHeight: 100,
    backgroundColor: '#222',
    color: '#fff',
    fontFamily: 'Berkeley Mono, monospace',
  },
});