import React, { useState, useEffect } from "react";
import {
  View,
  Text,
  TextInput,
  TouchableOpacity,
  FlatList,
  StyleSheet,
  Platform,
  Alert,
  ScrollView,
  Pressable,
} from "react-native";
import { useQuery, useMutation } from "convex/react";
import { api } from '@openagentsinc/convex';
import { ScreenWithSidebar, Text as CustomText, ThinkingAnimation, ErrorBoundary } from "./index";
import { AuthButton } from "./auth/AuthButton";
import { useConfectAuth } from "../contexts/SimpleConfectAuthContext";
import { IconPlus } from "./icons/IconPlus";
import type { ChatSession } from "../types/chat";
import { useAPMTracking } from "../src/hooks/useAPMTracking";
import { useUserSync } from "../hooks/useUserSync";

interface ClaudeSession {
  _id: string;
  sessionId: string;
  projectPath: string;
  title?: string;
  status: "active" | "inactive" | "error" | "processed";
  createdBy: "desktop" | "mobile";
  lastActivity: number;
  metadata?: any;
}

interface ClaudeMessage {
  _id: string;
  sessionId: string;
  messageId: string;
  messageType: "user" | "assistant" | "tool_use" | "tool_result" | "thinking";
  content: string;
  timestamp: string;
  toolInfo?: {
    toolName: string;
    toolUseId: string;
    input: any;
    output?: string;
  };
}

export function ClaudeCodeMobile() {
  const { isAuthenticated, user } = useConfectAuth();
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [showCreateModal, setShowCreateModal] = useState(false);
  
  // Automatically sync user to Convex when authenticated
  const { isSynced } = useUserSync();
  
  // Generate random 4-character string for session title
  const generateRandomString = () => {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789' as const;
    let result = '';
    for (let i = 0; i < 4; i++) {
      result += chars.charAt(Math.floor(Math.random() * chars.length));
    }
    return result;
  };
  
  // Session creation state - TODO: Move defaults to environment configuration
  const DEFAULT_PROJECT_PATH = process.env.EXPO_PUBLIC_DEFAULT_PROJECT_PATH || "/Users/christopherdavid/code/openagents";
  const DEFAULT_INITIAL_MESSAGE = process.env.EXPO_PUBLIC_DEFAULT_INITIAL_MESSAGE || "Introduce yourself, then use 3 readonly tools to explore the codebase and summarize what you learn.";
  
  const [newProjectPath, setNewProjectPath] = useState(DEFAULT_PROJECT_PATH);
  const [newSessionTitle, setNewSessionTitle] = useState(`Testing ${generateRandomString()}`);
  const [initialMessage, setInitialMessage] = useState(DEFAULT_INITIAL_MESSAGE);
  
  // Message input state
  const [newMessage, setNewMessage] = useState("");
  
  // Authentication is ready when user is authenticated AND synced to Convex
  // This prevents race conditions where queries execute before user sync completes
  const authReady = isAuthenticated && isSynced;

  // Convex hooks - only query data when authentication is ready
  const sessions = useQuery(
    api.confect.mobile_sync.getPendingMobileSessions, 
    authReady ? {} : "skip"
  ) || [];
  const selectedSessionMessages = useQuery(
    api.confect.mobile_sync.getSessionMessages, 
    authReady && selectedSessionId ? { sessionId: selectedSessionId } : "skip"
  ) || [];
  
  const requestDesktopSession = useMutation(api.confect.mobile_sync.createClaudeSession);
  const addMessage = useMutation(api.confect.mobile_sync.addClaudeMessage);
  const updateSyncStatus = useMutation(api.confect.mobile_sync.updateSessionStatus);

  // APM tracking - only enabled when authentication is ready
  const { trackMessageSent, trackSessionCreated } = useAPMTracking({
    enabled: authReady,
    trackMessages: authReady,
    trackSessions: authReady,
    trackAppState: authReady,
  });

  // Clear message input when switching sessions
  useEffect(() => {
    setNewMessage("");
  }, [selectedSessionId]);

  // Convert sessions to ChatSession format for the sidebar
  const convertedSessions: ChatSession[] = sessions.map((session: ClaudeSession) => ({
    _id: session._id,
    sessionId: session.sessionId,
    projectPath: session.projectPath,
    title: session.title,
    status: session.status,
    createdBy: session.createdBy,
    lastActivity: session.lastActivity,
    metadata: session.metadata,
    id: session.sessionId, // Use sessionId as id for compatibility
    updatedAt: new Date(session.lastActivity),
    isStarred: false, // Add default value, can be enhanced later
  }));

  const handleCreateSession = async () => {
    if (!newProjectPath.trim()) {
      Alert.alert("Error", "Please enter a project path");
      return;
    }

    console.log('📱 [MOBILE] Creating new Claude Code session with:', {
      projectPath: newProjectPath.trim(),
      title: newSessionTitle.trim() || undefined,
      hasInitialMessage: !!initialMessage.trim()
    });

    try {
      const sessionId = await requestDesktopSession({
        sessionId: `mobile-${Date.now()}`,
        projectPath: newProjectPath.trim(),
        createdBy: "mobile" as const,
        title: newSessionTitle.trim() || undefined,
        metadata: {
          workingDirectory: newProjectPath.trim(),
          originalMobileSessionId: `mobile-${Date.now()}`,
        },
      });

      console.log('✅ [MOBILE] Session created successfully with ID:', sessionId);

      // Track session creation for APM
      trackSessionCreated();

      Alert.alert(
        "Session Created",
        `New Claude Code session created! Session ID: ${sessionId}\n\nThe desktop app will automatically start this session.`,
        [
          {
            text: "View Session",
            onPress: () => {
              console.log('📱 [MOBILE] User selected to view session:', sessionId);
              setSelectedSessionId(sessionId);
              setShowCreateModal(false);
            },
          },
          { 
            text: "OK",
            onPress: () => setShowCreateModal(false),
          },
        ]
      );

      // Clear form and reset to defaults
      setNewProjectPath(DEFAULT_PROJECT_PATH);
      setNewSessionTitle(`Testing ${generateRandomString()}`);
      setInitialMessage(DEFAULT_INITIAL_MESSAGE);
    } catch (error) {
      console.error("❌ [MOBILE] Failed to create session:", error);
      Alert.alert("Error", "Failed to create session. Please try again.");
    }
  };

  const handleSendMessage = async (sessionId: string, content: string) => {
    if (!content.trim()) return;

    console.log('💬 [MOBILE] Sending message to session:', {
      sessionId,
      contentLength: content.trim().length,
      content: content.trim().substring(0, 100) + (content.trim().length > 100 ? '...' : '')
    });

    try {
      const messageId = `mobile-${Date.now()}-${Math.random().toString(36).substring(2)}`;
      console.log('📝 [MOBILE] Adding message with ID:', messageId);
      
      await addMessage({
        sessionId,
        messageId,
        messageType: "user",
        content: content.trim(),
        timestamp: new Date().toISOString(),
        metadata: { source: "mobile" },
      });

      console.log('✅ [MOBILE] Message added to Convex successfully');

      // Track message sent for APM
      trackMessageSent();

      // Update session status
      await updateSyncStatus({
        sessionId,
        status: "active",
      });

      console.log('🔄 [MOBILE] Updated sync status for session');
    } catch (error) {
      console.error("❌ [MOBILE] Failed to send message:", error);
      Alert.alert("Error", "Failed to send message. Please try again.");
    }
  };

  const handleNewChat = () => {
    setShowCreateModal(true);
  };

  const handleSessionSelect = (sessionId: string) => {
    setSelectedSessionId(sessionId);
  };

  const handleSessionDelete = (sessionId: string) => {
    // Session deletion not implemented - no-op
  };

  const handleSessionStar = (sessionId: string) => {
    // Session starring not implemented - no-op
  };

  const handleSessionRename = (sessionId: string, newTitle: string) => {
    // Session renaming not implemented - no-op
  };

  const renderMessageItem = ({ item }: { item: ClaudeMessage }) => (
    <View style={[
      styles.messageItem,
      item.messageType === "user" && styles.userMessage,
      item.messageType === "assistant" && styles.assistantMessage,
      item.messageType === "tool_use" && styles.toolMessage,
    ]}>
      <View style={styles.messageHeader}>
        <Text style={styles.messageType}>{item.messageType}</Text>
        <Text style={styles.messageTime}>
          {new Date(item.timestamp).toLocaleTimeString()}
        </Text>
      </View>
      
      <Text style={styles.messageContent}>{item.content}</Text>
      
      {item.toolInfo && (
        <View style={styles.toolInfo}>
          <Text style={styles.toolInfoText}>
            Tool: {item.toolInfo.toolName}
          </Text>
          {item.toolInfo.output && (
            <Text style={styles.toolInfoText}>
              Output: {item.toolInfo.output.substring(0, 100)}...
            </Text>
          )}
        </View>
      )}
    </View>
  );

  const renderCreateSessionModal = () => {
    if (!showCreateModal) return null;

    return (
      <View style={styles.modalOverlay}>
        <View style={styles.modalContent}>
          <View style={styles.modalHeader}>
            <CustomText style={styles.modalTitle}>Create New Session</CustomText>
            <TouchableOpacity 
              onPress={() => setShowCreateModal(false)}
              accessibilityRole="button"
              accessibilityLabel="Close create session dialog"
            >
              <CustomText style={styles.modalClose}>×</CustomText>
            </TouchableOpacity>
          </View>

          <ScrollView style={styles.modalBody} showsVerticalScrollIndicator={false}>
            <View style={styles.formGroup}>
              <CustomText style={styles.label}>Project Path *</CustomText>
              <TextInput
                style={styles.input}
                value={newProjectPath}
                onChangeText={setNewProjectPath}
                placeholder="/Users/username/my-project"
                placeholderTextColor="#71717a"
              />
              <CustomText style={styles.helpText}>
                Full path to your project directory
              </CustomText>
            </View>

            <View style={styles.formGroup}>
              <CustomText style={styles.label}>Session Title</CustomText>
              <TextInput
                style={styles.input}
                value={newSessionTitle}
                onChangeText={setNewSessionTitle}
                placeholder="My Mobile Session"
                placeholderTextColor="#71717a"
              />
              <CustomText style={styles.helpText}>
                Optional title for this session
              </CustomText>
            </View>

            <View style={styles.formGroup}>
              <CustomText style={styles.label}>Initial Message</CustomText>
              <TextInput
                style={[styles.input, styles.textArea]}
                value={initialMessage}
                onChangeText={setInitialMessage}
                placeholder="Hi Claude, can you help me with..."
                placeholderTextColor="#71717a"
                multiline
                numberOfLines={4}
              />
              <CustomText style={styles.helpText}>
                Optional first message to send to Claude
              </CustomText>
            </View>

            <TouchableOpacity
              style={[
                styles.createButton,
                !newProjectPath.trim() && styles.createButtonDisabled,
              ]}
              onPress={handleCreateSession}
              disabled={!newProjectPath.trim()}
              accessibilityRole="button"
              accessibilityLabel="Create new desktop session"
              accessibilityState={{ disabled: !newProjectPath.trim() }}
            >
              <CustomText style={styles.createButtonText}>
                Create Desktop Session
              </CustomText>
            </TouchableOpacity>
          </ScrollView>
        </View>
      </View>
    );
  };

  const renderHeaderTitle = () => (
    <View style={styles.headerTitle}>
      <CustomText style={styles.headerTitleText}>Claude Code</CustomText>
    </View>
  );

  const renderMainContent = () => {
    if (!authReady) {
      return (
        <View style={styles.emptyState}>
          <ThinkingAnimation size={38} style={styles.connectionAnimation} />
          <CustomText style={styles.emptyStateText}>
            Setting up secure connection...
          </CustomText>
          <CustomText style={styles.emptyStateSubtext}>
            Please wait while we authenticate with the server
          </CustomText>
        </View>
      );
    }

    if (!selectedSessionId) {
      return (
        <View style={styles.emptyState}>
          <CustomText style={styles.emptyStateText}>
            Select a session from the sidebar to view messages
          </CustomText>
          <CustomText style={styles.emptyStateSubtext}>
            Or create a new session to get started
          </CustomText>
        </View>
      );
    }

    const session = sessions.find((s: ClaudeSession) => s.sessionId === selectedSessionId);

    return (
      <View style={styles.sessionDetail}>
        <FlatList
          data={selectedSessionMessages}
          keyExtractor={(item) => item._id}
          renderItem={renderMessageItem}
          style={styles.messagesList}
          showsVerticalScrollIndicator={false}
          inverted
          ListEmptyComponent={
            <View style={styles.emptyState}>
              <CustomText style={styles.emptyStateText}>No messages yet</CustomText>
              <CustomText style={styles.emptyStateSubtext}>
                Start a conversation with Claude
              </CustomText>
            </View>
          }
        />

        <View style={styles.messageInput}>
          <TextInput
            style={styles.messageTextInput}
            value={newMessage}
            onChangeText={setNewMessage}
            placeholder="Type a message..."
            placeholderTextColor="#666"
            multiline
          />
          <TouchableOpacity
            style={[
              styles.sendButton,
              !newMessage.trim() && styles.sendButtonDisabled,
            ]}
            onPress={() => {
              handleSendMessage(selectedSessionId, newMessage);
              setNewMessage("");
            }}
            disabled={!newMessage.trim()}
          >
            <CustomText style={styles.sendButtonText}>Send</CustomText>
          </TouchableOpacity>
        </View>
      </View>
    );
  };

  // Show clean login screen without header/sidebar when not authenticated
  if (!isAuthenticated) {
    return (
      <ErrorBoundary
        onError={(error, errorInfo) => {
          const timestamp = new Date().toISOString();
          console.error(`❌ [CLAUDE_CODE_MOBILE] ${timestamp} Component error:`, {
            error: error.message,
            stack: error.stack,
            componentStack: errorInfo.componentStack,
            user: user?.githubUsername,
            isAuthenticated,
            selectedSessionId
          });
          // TODO: Report to crash analytics service
        }}
      >
        <View style={styles.loginContainer}>
          <View style={styles.loginContent}>
            <CustomText style={styles.loginTitle}>OpenAgents</CustomText>
            <View style={styles.authButtonContainer}>
              <AuthButton />
            </View>
          </View>
        </View>
      </ErrorBoundary>
    );
  }

  // Show main app with header/sidebar when authenticated
  return (
    <ErrorBoundary
      onError={(error, errorInfo) => {
        const timestamp = new Date().toISOString();
        console.error(`❌ [CLAUDE_CODE_MOBILE] ${timestamp} Component error:`, {
          error: error.message,
          stack: error.stack,
          componentStack: errorInfo.componentStack,
          user: user?.githubUsername,
          isAuthenticated,
          selectedSessionId
        });
        // TODO: Report to crash analytics service
      }}
    >
      <ScreenWithSidebar
        title={renderHeaderTitle()}
        onNewChat={authReady ? handleNewChat : undefined}
        onSessionSelect={handleSessionSelect}
        currentSessionId={selectedSessionId}
        sessions={authReady ? convertedSessions : []}
        onSessionDelete={handleSessionDelete}
        onSessionStar={handleSessionStar}
        onSessionRename={handleSessionRename}
        disableKeyboardAvoidance={true}
      >
        <View style={styles.container}>
          {renderMainContent()}
        </View>
      </ScreenWithSidebar>
      {authReady && renderCreateSessionModal()}
    </ErrorBoundary>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
  },
  headerTitle: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    flex: 1,
    paddingHorizontal: 16,
  },
  headerTitleText: {
    color: '#f4f4f5',
    fontSize: 16,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  sessionDetail: {
    flex: 1,
  },
  messagesList: {
    flex: 1,
    padding: 16,
  },
  messageItem: {
    backgroundColor: '#18181b',
    borderRadius: 8,
    padding: 12,
    marginBottom: 8,
    borderLeftWidth: 4,
    borderLeftColor: '#27272a',
  },
  userMessage: {
    borderLeftColor: '#60a5fa',
  },
  assistantMessage: {
    borderLeftColor: '#22c55e',
  },
  toolMessage: {
    borderLeftColor: '#f59e0b',
  },
  messageHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 8,
  },
  messageType: {
    color: '#a1a1aa',
    fontSize: 12,
    fontWeight: 'bold',
    textTransform: 'uppercase',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  messageTime: {
    color: '#71717a',
    fontSize: 10,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  messageContent: {
    color: '#f4f4f5',
    fontSize: 14,
    lineHeight: 20,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  toolInfo: {
    marginTop: 8,
    padding: 8,
    backgroundColor: '#27272a',
    borderRadius: 4,
  },
  toolInfoText: {
    color: '#f59e0b',
    fontSize: 12,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  messageInput: {
    flexDirection: 'row',
    padding: 16,
    borderTopWidth: 1,
    borderTopColor: '#27272a',
    gap: 8,
  },
  messageTextInput: {
    flex: 1,
    backgroundColor: '#18181b',
    color: '#f4f4f5',
    padding: 12,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#27272a',
    fontSize: 14,
    maxHeight: 100,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  sendButton: {
    backgroundColor: '#60a5fa',
    paddingHorizontal: 16,
    paddingVertical: 12,
    borderRadius: 8,
    justifyContent: 'center',
  },
  sendButtonDisabled: {
    backgroundColor: '#374151',
  },
  sendButtonText: {
    color: '#f4f4f5',
    fontSize: 14,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  emptyState: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 40,
    marginTop: -80,
  },
  emptyStateText: {
    color: '#a1a1aa',
    fontSize: 16,
    textAlign: 'center',
    marginBottom: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  emptyStateSubtext: {
    color: '#71717a',
    fontSize: 14,
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  authButtonContainer: {
    marginTop: -10,
    alignItems: 'center',
  },
  connectionAnimation: {
    marginBottom: 16,
  },
  // Modal styles
  modalOverlay: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: 'rgba(9, 9, 11, 0.8)',
    justifyContent: 'center',
    alignItems: 'center',
    zIndex: 1000,
  },
  modalContent: {
    backgroundColor: '#000',
    borderRadius: 12,
    width: '90%',
    maxWidth: 400,
    maxHeight: '80%',
    borderWidth: 1,
    borderColor: '#27272a',
  },
  modalHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    padding: 16,
    borderBottomWidth: 1,
    borderBottomColor: '#27272a',
  },
  modalTitle: {
    fontSize: 18,
    fontWeight: 'bold',
    color: '#f4f4f5',
  },
  modalClose: {
    fontSize: 20,
    color: '#a1a1aa',
    fontWeight: 'bold',
  },
  modalBody: {
    padding: 16,
    maxHeight: 400,
  },
  formGroup: {
    marginBottom: 20,
  },
  label: {
    color: '#f4f4f5',
    fontSize: 14,
    fontWeight: 'bold',
    marginBottom: 8,
  },
  input: {
    backgroundColor: '#18181b',
    color: '#f4f4f5',
    padding: 12,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#27272a',
    fontSize: 14,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  textArea: {
    height: 80,
    textAlignVertical: 'top',
  },
  helpText: {
    color: '#71717a',
    fontSize: 12,
    marginTop: 4,
  },
  createButton: {
    backgroundColor: '#22c55e',
    paddingVertical: 16,
    borderRadius: 8,
    alignItems: 'center',
    marginTop: 16,
  },
  createButtonDisabled: {
    backgroundColor: '#374151',
  },
  createButtonText: {
    color: '#f4f4f5',
    fontSize: 16,
    fontWeight: 'bold',
  },
  // Login screen styles
  loginContainer: {
    flex: 1,
    backgroundColor: '#000',
    justifyContent: 'center',
    alignItems: 'center',
    padding: 24,
  },
  loginContent: {
    alignItems: 'center',
    maxWidth: 400,
    width: '100%',
  },
  loginTitle: {
    color: '#f4f4f5',
    fontSize: 32,
    fontWeight: 'bold',
    marginBottom: 48,
    textAlign: 'center',
    lineHeight: 32,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  loginSubtitle: {
    color: '#a1a1aa',
    fontSize: 16,
    marginBottom: 48,
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
});