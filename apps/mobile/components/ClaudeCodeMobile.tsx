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
} from "react-native";
import { useQuery, useMutation } from "convex/react";
import { api } from "../convex/_generated/api";

interface ClaudeSession {
  _id: string;
  sessionId: string;
  projectPath: string;
  title?: string;
  status: "active" | "inactive" | "error";
  createdBy: "desktop" | "mobile";
  lastActivity: number;
  metadata?: any;
}

interface ClaudeMessage {
  _id: string;
  sessionId: string;
  messageId: string;
  messageType: "user" | "assistant" | "tool_use" | "tool_result";
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
  const [activeTab, setActiveTab] = useState<"sessions" | "create">("sessions");
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  
  // Session creation state
  const [newProjectPath, setNewProjectPath] = useState("/Users/christopherdavid/code/openagents");
  const [newSessionTitle, setNewSessionTitle] = useState("");
  const [initialMessage, setInitialMessage] = useState("");
  
  // Message input state (moved from renderSessionDetail to fix hooks violation)
  const [newMessage, setNewMessage] = useState("");
  
  // Convex hooks
  const sessions = useQuery(api.claude.getSessions, { limit: 50 }) || [];
  const selectedSessionMessages = useQuery(
    api.claude.getSessionMessages, 
    selectedSessionId ? { sessionId: selectedSessionId } : "skip"
  ) || [];
  
  const requestDesktopSession = useMutation(api.claude.requestDesktopSession);
  const addMessage = useMutation(api.claude.addClaudeMessage);
  const updateSyncStatus = useMutation(api.claude.updateSyncStatus);

  // Clear message input when switching sessions
  useEffect(() => {
    setNewMessage("");
  }, [selectedSessionId]);

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
        projectPath: newProjectPath.trim(),
        initialMessage: initialMessage.trim() || undefined,
        title: newSessionTitle.trim() || undefined,
      });

      console.log('✅ [MOBILE] Session created successfully with ID:', sessionId);

      Alert.alert(
        "Session Created",
        `New Claude Code session created! Session ID: ${sessionId}\n\nThe desktop app will automatically start this session.`,
        [
          {
            text: "View Session",
            onPress: () => {
              console.log('📱 [MOBILE] User selected to view session:', sessionId);
              setSelectedSessionId(sessionId);
              setActiveTab("sessions");
            },
          },
          { text: "OK" },
        ]
      );

      // Clear form
      setNewProjectPath("");
      setNewSessionTitle("");
      setInitialMessage("");
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

      // Update mobile last seen
      await updateSyncStatus({
        sessionId,
        mobileLastSeen: Date.now(),
      });

      console.log('🔄 [MOBILE] Updated sync status for session');
    } catch (error) {
      console.error("❌ [MOBILE] Failed to send message:", error);
      Alert.alert("Error", "Failed to send message. Please try again.");
    }
  };

  const renderSessionItem = ({ item }: { item: ClaudeSession }) => (
    <TouchableOpacity
      style={[
        styles.sessionItem,
        selectedSessionId === item.sessionId && styles.selectedSessionItem,
      ]}
      onPress={() => setSelectedSessionId(item.sessionId)}
    >
      <View style={styles.sessionHeader}>
        <Text style={styles.sessionTitle}>
          {item.title || `Session - ${item.projectPath.split('/').pop()}`}
        </Text>
        <View style={[
          styles.statusBadge,
          item.status === "active" && styles.statusActive,
          item.status === "inactive" && styles.statusInactive,
          item.status === "error" && styles.statusError,
        ]}>
          <Text style={styles.statusText}>{item.status}</Text>
        </View>
      </View>
      
      <Text style={styles.sessionPath}>{item.projectPath}</Text>
      
      <View style={styles.sessionMeta}>
        <Text style={styles.sessionMetaText}>
          Created by: {item.createdBy}
        </Text>
        <Text style={styles.sessionMetaText}>
          Last activity: {new Date(item.lastActivity).toLocaleString()}
        </Text>
      </View>
    </TouchableOpacity>
  );

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

  const renderSessionDetail = () => {
    if (!selectedSessionId) {
      return (
        <View style={styles.emptyState}>
          <Text style={styles.emptyStateText}>Select a session to view messages</Text>
        </View>
      );
    }

    const session = sessions.find((s: ClaudeSession) => s.sessionId === selectedSessionId);

    return (
      <View style={styles.sessionDetail}>
        <View style={styles.sessionDetailHeader}>
          <TouchableOpacity
            style={styles.backButton}
            onPress={() => setSelectedSessionId(null)}
          >
            <Text style={styles.backButtonText}>← Back</Text>
          </TouchableOpacity>
          <Text style={styles.sessionDetailTitle}>
            {session?.title || "Session"}
          </Text>
        </View>

        <FlatList
          data={selectedSessionMessages}
          keyExtractor={(item) => item._id}
          renderItem={renderMessageItem}
          style={styles.messagesList}
          showsVerticalScrollIndicator={false}
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
            <Text style={styles.sendButtonText}>Send</Text>
          </TouchableOpacity>
        </View>
      </View>
    );
  };

  const renderCreateSession = () => (
    <ScrollView style={styles.createSession} showsVerticalScrollIndicator={false}>
      <Text style={styles.createTitle}>Create New Claude Code Session</Text>
      <Text style={styles.createDescription}>
        Start a new Claude Code session that will run on the desktop app
      </Text>

      <View style={styles.formGroup}>
        <Text style={styles.label}>Project Path *</Text>
        <TextInput
          style={styles.input}
          value={newProjectPath}
          onChangeText={setNewProjectPath}
          placeholder="/Users/username/my-project"
          placeholderTextColor="#666"
        />
        <Text style={styles.helpText}>
          Full path to your project directory
        </Text>
      </View>

      <View style={styles.formGroup}>
        <Text style={styles.label}>Session Title</Text>
        <TextInput
          style={styles.input}
          value={newSessionTitle}
          onChangeText={setNewSessionTitle}
          placeholder="My Mobile Session"
          placeholderTextColor="#666"
        />
        <Text style={styles.helpText}>
          Optional title for this session
        </Text>
      </View>

      <View style={styles.formGroup}>
        <Text style={styles.label}>Initial Message</Text>
        <TextInput
          style={[styles.input, styles.textArea]}
          value={initialMessage}
          onChangeText={setInitialMessage}
          placeholder="Hi Claude, can you help me with..."
          placeholderTextColor="#666"
          multiline
          numberOfLines={4}
        />
        <Text style={styles.helpText}>
          Optional first message to send to Claude
        </Text>
      </View>

      <TouchableOpacity
        style={[
          styles.createButton,
          !newProjectPath.trim() && styles.createButtonDisabled,
        ]}
        onPress={handleCreateSession}
        disabled={!newProjectPath.trim()}
      >
        <Text style={styles.createButtonText}>
          Create Desktop Session
        </Text>
      </TouchableOpacity>
    </ScrollView>
  );

  return (
    <View style={styles.container}>
      <View style={styles.tabBar}>
        <TouchableOpacity
          style={[styles.tab, activeTab === "sessions" && styles.activeTab]}
          onPress={() => setActiveTab("sessions")}
        >
          <Text style={[styles.tabText, activeTab === "sessions" && styles.activeTabText]}>
            Sessions ({sessions.length})
          </Text>
        </TouchableOpacity>
        
        <TouchableOpacity
          style={[styles.tab, activeTab === "create" && styles.activeTab]}
          onPress={() => setActiveTab("create")}
        >
          <Text style={[styles.tabText, activeTab === "create" && styles.activeTabText]}>
            Create New
          </Text>
        </TouchableOpacity>
      </View>

      {activeTab === "sessions" && (
        selectedSessionId ? renderSessionDetail() : (
          <FlatList
            data={sessions}
            keyExtractor={(item) => item._id}
            renderItem={renderSessionItem}
            style={styles.sessionsList}
            showsVerticalScrollIndicator={false}
            ListEmptyComponent={
              <View style={styles.emptyState}>
                <Text style={styles.emptyStateText}>
                  No Claude Code sessions yet
                </Text>
                <Text style={styles.emptyStateSubtext}>
                  Create a new session to get started
                </Text>
              </View>
            }
          />
        )
      )}

      {activeTab === "create" && renderCreateSession()}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#1a1a1a',
  },
  tabBar: {
    flexDirection: 'row',
    backgroundColor: '#111',
    borderBottomWidth: 1,
    borderBottomColor: '#333',
  },
  tab: {
    flex: 1,
    paddingVertical: 12,
    paddingHorizontal: 16,
    alignItems: 'center',
  },
  activeTab: {
    backgroundColor: '#333',
  },
  tabText: {
    color: '#999',
    fontSize: 14,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  activeTabText: {
    color: '#fff',
  },
  sessionsList: {
    flex: 1,
    padding: 16,
  },
  sessionItem: {
    backgroundColor: '#111',
    borderRadius: 8,
    padding: 16,
    marginBottom: 12,
    borderWidth: 1,
    borderColor: '#333',
  },
  selectedSessionItem: {
    borderColor: '#60a5fa',
    backgroundColor: '#1e3a8a20',
  },
  sessionHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 8,
  },
  sessionTitle: {
    flex: 1,
    color: '#fff',
    fontSize: 16,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  statusBadge: {
    paddingHorizontal: 8,
    paddingVertical: 4,
    borderRadius: 4,
    backgroundColor: '#333',
  },
  statusActive: {
    backgroundColor: '#22c55e',
  },
  statusInactive: {
    backgroundColor: '#6b7280',
  },
  statusError: {
    backgroundColor: '#ef4444',
  },
  statusText: {
    color: '#fff',
    fontSize: 10,
    fontWeight: 'bold',
  },
  sessionPath: {
    color: '#999',
    fontSize: 12,
    marginBottom: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  sessionMeta: {
    flexDirection: 'column',
  },
  sessionMetaText: {
    color: '#666',
    fontSize: 10,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  sessionDetail: {
    flex: 1,
  },
  sessionDetailHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    padding: 16,
    borderBottomWidth: 1,
    borderBottomColor: '#333',
  },
  backButton: {
    marginRight: 16,
  },
  backButtonText: {
    color: '#60a5fa',
    fontSize: 16,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  sessionDetailTitle: {
    flex: 1,
    color: '#fff',
    fontSize: 18,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  messagesList: {
    flex: 1,
    padding: 16,
  },
  messageItem: {
    backgroundColor: '#111',
    borderRadius: 8,
    padding: 12,
    marginBottom: 8,
    borderLeftWidth: 4,
    borderLeftColor: '#333',
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
    color: '#999',
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
    color: '#666',
    fontSize: 10,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  messageContent: {
    color: '#fff',
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
    backgroundColor: '#333',
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
    borderTopColor: '#333',
    gap: 8,
  },
  messageTextInput: {
    flex: 1,
    backgroundColor: '#111',
    color: '#fff',
    padding: 12,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#333',
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
    color: '#fff',
    fontSize: 14,
    fontWeight: 'bold',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  createSession: {
    flex: 1,
    padding: 16,
  },
  createTitle: {
    color: '#fff',
    fontSize: 20,
    fontWeight: 'bold',
    marginBottom: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  createDescription: {
    color: '#999',
    fontSize: 14,
    marginBottom: 24,
    lineHeight: 20,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  formGroup: {
    marginBottom: 20,
  },
  label: {
    color: '#fff',
    fontSize: 14,
    fontWeight: 'bold',
    marginBottom: 8,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
  input: {
    backgroundColor: '#111',
    color: '#fff',
    padding: 12,
    borderRadius: 8,
    borderWidth: 1,
    borderColor: '#333',
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
    color: '#666',
    fontSize: 12,
    marginTop: 4,
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
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
    color: '#fff',
    fontSize: 16,
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
  },
  emptyStateText: {
    color: '#999',
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
    color: '#666',
    fontSize: 14,
    textAlign: 'center',
    fontFamily: Platform.select({
      ios: 'Berkeley Mono',
      android: 'Berkeley Mono',
      default: 'monospace'
    }),
  },
});