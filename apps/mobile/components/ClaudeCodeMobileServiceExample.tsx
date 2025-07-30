/**
 * Example of ClaudeCodeMobile component using the new Effect-TS Service Layer
 * This demonstrates how to integrate the new dedicated service layer
 */

import React, { useState, useEffect, useCallback } from "react";
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
import { ScreenWithSidebar, Text as CustomText, ThinkingAnimation, ErrorBoundary } from "./index";
import { AuthButton } from "./auth/AuthButton";
import { useConfectAuth } from "../contexts/SimpleConfectAuthContext";
import { IconPlus } from "./icons/IconPlus";
import type { ChatSession } from "../types/chat";
import { useAPMTracking } from "../src/hooks/useAPMTracking";
import { useUserSync } from "../hooks/useUserSync";

// Import the new service layer hooks
import { useClaudeSessionService } from "../src/hooks/useClaudeSessionService";
import {
  CreateSessionParams,
  SessionData,
  SessionStatus,
} from "@openagentsinc/shared";

// Constants with defaults from environment
const DEFAULT_PROJECT_PATH = "/Users/testuser/projects/openagents";
const DEFAULT_INITIAL_MESSAGE = "Create a new feature for user authentication";

// Generate random string for unique session titles
const generateRandomString = () => Math.random().toString(36).substring(2, 8);

export function ClaudeCodeMobileServiceExample() {
  const { isAuthenticated, user } = useConfectAuth();
  const { isSynced } = useUserSync();
  
  // Authentication is ready when user is authenticated AND synced to Convex
  const authReady = isAuthenticated && isSynced;
  
  // APM tracking
  const { trackMessageSent, trackSessionCreated } = useAPMTracking();
  
  // New service layer integration
  const sessionService = useClaudeSessionService({
    onError: (error) => {
      console.error("❌ [MOBILE_SERVICE] Session service error:", error);
      Alert.alert("Error", error);
    },
    onSuccess: (message) => {
      console.log("✅ [MOBILE_SERVICE] Session service success:", message);
    }
  });
  
  // State for session management
  const [sessions, setSessions] = useState<SessionData[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [selectedSessionMessages, setSelectedSessionMessages] = useState<any[]>([]);
  const [newMessage, setNewMessage] = useState("");
  
  // Modal state for session creation
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [newProjectPath, setNewProjectPath] = useState(DEFAULT_PROJECT_PATH);
  const [newSessionTitle, setNewSessionTitle] = useState(`Testing ${generateRandomString()}`);
  const [initialMessage, setInitialMessage] = useState(DEFAULT_INITIAL_MESSAGE);
  
  // Load sessions using the new service layer
  const loadSessions = useCallback(async () => {
    if (!authReady) return;
    
    try {
      console.log("🔄 [MOBILE_SERVICE] Loading sessions with new service layer");
      
      const result = await sessionService.querySessionsAdvanced({
        createdBy: "mobile",
        status: "active",
        limit: 10,
        sortBy: "lastActivity",
        sortOrder: "desc"
      });
      
      setSessions(result.sessions);
      console.log(`📋 [MOBILE_SERVICE] Loaded ${result.sessions.length} sessions`);
    } catch (error) {
      console.error("❌ [MOBILE_SERVICE] Failed to load sessions:", error);
    }
  }, [authReady, sessionService]);
  
  // Load sessions on mount and when auth is ready
  useEffect(() => {
    loadSessions();
  }, [loadSessions]);
  
  // Handle session creation with new service layer
  const handleCreateSession = async () => {
    if (!newProjectPath.trim()) {
      Alert.alert("Error", "Please enter a project path");
      return;
    }
    
    try {
      console.log('📱 [MOBILE_SERVICE] Creating new Claude Code session with:', {
        projectPath: newProjectPath.trim(),
        title: newSessionTitle.trim() || undefined,
        hasInitialMessage: !!initialMessage.trim()
      });
      
      // Create session using resilient service layer
      const params: CreateSessionParams = {
        sessionId: `mobile-${Date.now()}`,
        projectPath: newProjectPath.trim(),
        createdBy: "mobile" as const,
        title: newSessionTitle.trim() || undefined,
        metadata: {
          workingDirectory: newProjectPath.trim(),
          originalMobileSessionId: `mobile-${Date.now()}`,
        },
      };
      
      const sessionData = await sessionService.createSessionResilient(params);
      
      // Track session creation for APM
      trackSessionCreated();
      
      // Show success alert with session details
      Alert.alert(
        "Session Created",
        `New Claude Code session created! Session ID: ${sessionData.sessionId}\n\nThe desktop app will automatically start this session.`,
        [
          {
            text: "View Session",
            onPress: () => {
              console.log('📱 [MOBILE_SERVICE] User selected to view session:', sessionData.sessionId);
              setSelectedSessionId(sessionData.sessionId);
              setShowCreateModal(false);
            },
          },
          { 
            text: "OK",
            onPress: () => setShowCreateModal(false),
          },
        ]
      );
      
      // Reload sessions to show the new one
      await loadSessions();
      
    } catch (error) {
      console.error("❌ [MOBILE_SERVICE] Failed to create session:", error);
      Alert.alert("Error", "Failed to create session. Please try again.");
    }
  };
  
  // Handle message sending with new service layer
  const handleSendMessage = async (sessionId: string, content: string) => {
    if (!content.trim()) return;
    
    try {
      console.log('💬 [MOBILE_SERVICE] Sending message to session:', {
        sessionId,
        contentLength: content.trim().length,
      });
      
      // In a full implementation, this would call a message service
      // For now, just simulate the behavior
      const messageId = `mobile-${Date.now()}-${Math.random().toString(36).substring(2)}`;
      
      // Track message sent for APM
      trackMessageSent();
      
      // Update session status to ensure desktop processing
      await sessionService.updateSessionStatus(sessionId, "active");
      
      // Clear message input
      setNewMessage("");
      
    } catch (error) {
      console.error("❌ [MOBILE_SERVICE] Failed to send message:", error);
      Alert.alert("Error", "Failed to send message. Please try again.");
    }
  };
  
  // Handle session selection
  const handleSessionSelect = (sessionId: string) => {
    setSelectedSessionId(sessionId);
  };
  
  // Handle new chat
  const handleNewChat = () => {
    setShowCreateModal(true);
  };
  
  // Get resilience health for debugging
  const handleGetHealth = async () => {
    try {
      const health = await sessionService.getResilienceHealth();
      console.log("🏥 [MOBILE_SERVICE] Resilience health:", health);
      Alert.alert("Resilience Health", JSON.stringify(health, null, 2));
    } catch (error) {
      console.error("❌ [MOBILE_SERVICE] Failed to get health:", error);
    }
  };
  
  // Render session item
  const renderSessionItem = ({ item }: { item: SessionData }) => (
    <TouchableOpacity
      style={[
        styles.sessionItem,
        selectedSessionId === item.sessionId && styles.selectedSessionItem
      ]}
      onPress={() => handleSessionSelect(item.sessionId)}
    >
      <View style={styles.sessionHeader}>
        <Text style={styles.sessionTitle} numberOfLines={1}>
          {item.title}
        </Text>
        <Text style={styles.sessionStatus}>
          {item.status}
        </Text>
      </View>
      <Text style={styles.sessionPath} numberOfLines={1}>
        {item.projectPath}
      </Text>
      <Text style={styles.sessionTime}>
        {new Date(item.lastActivity).toLocaleString()}
      </Text>
    </TouchableOpacity>
  );
  
  if (!isAuthenticated) {
    return (
      <ScreenWithSidebar title="Claude Code" sessions={[]}>
        <View style={styles.container}>
          <View style={styles.authContainer}>
            <CustomText style={styles.title}>Welcome to Claude Code Mobile</CustomText>
            <CustomText style={styles.subtitle}>Sign in to manage your coding sessions</CustomText>
            <AuthButton />
          </View>
        </View>
      </ScreenWithSidebar>
    );
  }
  
  if (!isSynced) {
    return (
      <ScreenWithSidebar title="Claude Code" sessions={[]}>
        <View style={styles.container}>
          <View style={styles.loadingContainer}>
            <ThinkingAnimation />
            <CustomText style={styles.loadingText}>Syncing your account...</CustomText>
          </View>
        </View>
      </ScreenWithSidebar>
    );
  }
  
  return (
    <ErrorBoundary
      onError={(error, errorInfo) => {
        const timestamp = new Date().toISOString();
        console.error(`❌ [CLAUDE_CODE_MOBILE_SERVICE] ${timestamp} Component error:`, {
          error: error.message,
          stack: error.stack,
          componentStack: errorInfo.componentStack,
          user: user?.githubUsername,
          isAuthenticated,
          selectedSessionId,
          serviceLoading: sessionService.isLoading,
          serviceError: sessionService.error,
        });
      }}
    >
      <ScreenWithSidebar 
        title="Claude Code (Service Layer)"
        onNewChat={handleNewChat}
        sessions={sessions.map(s => ({
          id: s.sessionId,
          title: s.title,
          messages: [], // Would be loaded separately
          createdAt: s.createdAt,
          updatedAt: s.lastActivity,
        }))}
        onSessionSelect={(id) => handleSessionSelect(id)}
        currentSessionId={selectedSessionId}
      >
        <View style={styles.container}>
          {/* Service Status */}
          <View style={styles.serviceStatus}>
            <Text style={styles.serviceStatusText}>
              Service: {sessionService.isLoading ? "Loading..." : "Ready"}
            </Text>
            {sessionService.error && (
              <Text style={styles.errorText}>Error: {sessionService.error}</Text>
            )}
            <TouchableOpacity onPress={handleGetHealth} style={styles.healthButton}>
              <Text style={styles.buttonText}>Check Health</Text>
            </TouchableOpacity>
          </View>
          
          {/* Sessions List */}
          <View style={styles.sessionsContainer}>
            <CustomText style={styles.sectionTitle}>
              Active Sessions ({sessions.length})
            </CustomText>
            
            {sessionService.isLoading ? (
              <View style={styles.loadingContainer}>
                <ThinkingAnimation />
                <CustomText style={styles.loadingText}>Loading sessions...</CustomText>
              </View>
            ) : sessions.length === 0 ? (
              <View style={styles.emptyContainer}>
                <CustomText style={styles.emptyText}>
                  No active sessions. Create one to get started!
                </CustomText>
                <TouchableOpacity style={styles.createButton} onPress={handleNewChat}>
                  <IconPlus />
                  <CustomText style={styles.createButtonText}>Create Session</CustomText>
                </TouchableOpacity>
              </View>
            ) : (
              <FlatList
                data={sessions}
                renderItem={renderSessionItem}
                keyExtractor={(item) => item.sessionId}
                contentContainerStyle={styles.sessionsList}
                showsVerticalScrollIndicator={false}
              />
            )}
          </View>
          
          {/* Selected Session Details */}
          {selectedSessionId && (
            <View style={styles.messageContainer}>
              <CustomText style={styles.sectionTitle}>
                Session: {sessions.find(s => s.sessionId === selectedSessionId)?.title}
              </CustomText>
              
              <View style={styles.messageInputContainer}>
                <TextInput
                  style={styles.messageInput}
                  value={newMessage}
                  onChangeText={setNewMessage}
                  placeholder="Type your message..."
                  placeholderTextColor="#666"
                  multiline
                />
                <TouchableOpacity
                  style={styles.sendButton}
                  onPress={() => handleSendMessage(selectedSessionId, newMessage)}
                  disabled={!newMessage.trim() || sessionService.isLoading}
                >
                  <CustomText style={styles.sendButtonText}>Send</CustomText>
                </TouchableOpacity>
              </View>
            </View>
          )}
        </View>
        
        {/* Create Session Modal */}
        {showCreateModal && (
          <View style={styles.modalOverlay}>
            <View style={styles.modalContent}>
              <CustomText style={styles.modalTitle}>Create New Session</CustomText>
              
              <View style={styles.inputGroup}>
                <CustomText style={styles.inputLabel}>Project Path *</CustomText>
                <TextInput
                  style={styles.modalInput}
                  value={newProjectPath}
                  onChangeText={setNewProjectPath}
                  placeholder="Enter project path"
                  placeholderTextColor="#666"
                />
              </View>
              
              <View style={styles.inputGroup}>
                <CustomText style={styles.inputLabel}>Session Title</CustomText>
                <TextInput
                  style={styles.modalInput}
                  value={newSessionTitle}
                  onChangeText={setNewSessionTitle}
                  placeholder="Enter session title (optional)"
                  placeholderTextColor="#666"
                />
              </View>
              
              <View style={styles.inputGroup}>
                <CustomText style={styles.inputLabel}>Initial Message</CustomText>
                <TextInput
                  style={[styles.modalInput, styles.multilineInput]}
                  value={initialMessage}
                  onChangeText={setInitialMessage}
                  placeholder="Enter initial message (optional)"
                  placeholderTextColor="#666"
                  multiline
                  numberOfLines={3}
                />
              </View>
              
              <View style={styles.modalButtons}>
                <TouchableOpacity
                  style={[styles.modalButton, styles.cancelButton]}
                  onPress={() => setShowCreateModal(false)}
                >
                  <CustomText style={styles.cancelButtonText}>Cancel</CustomText>
                </TouchableOpacity>
                
                <TouchableOpacity
                  style={[styles.modalButton, styles.createButtonModal]}
                  onPress={handleCreateSession}
                  disabled={sessionService.isLoading}
                >
                  <CustomText style={styles.createButtonText}>
                    {sessionService.isLoading ? "Creating..." : "Create Session"}
                  </CustomText>
                </TouchableOpacity>
              </View>
            </View>
          </View>
        )}
      </ScreenWithSidebar>
    </ErrorBoundary>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#000',
    padding: 16,
  },
  authContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 20,
  },
  title: {
    fontSize: 24,
    fontWeight: 'bold',
    color: '#fff',
    marginBottom: 8,
    textAlign: 'center',
  },
  subtitle: {
    fontSize: 16,
    color: '#888',
    marginBottom: 32,
    textAlign: 'center',
  },
  loadingContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
  },
  loadingText: {
    marginTop: 16,
    fontSize: 16,
    color: '#888',
  },
  serviceStatus: {
    backgroundColor: '#111',
    padding: 12,
    borderRadius: 8,
    marginBottom: 16,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
  },
  serviceStatusText: {
    color: '#fff',
    fontSize: 14,
  },
  errorText: {
    color: '#ff6b6b',
    fontSize: 12,
    marginTop: 4,
  },
  healthButton: {
    backgroundColor: '#333',
    paddingHorizontal: 12,
    paddingVertical: 6,
    borderRadius: 4,
  },
  buttonText: {
    color: '#fff',
    fontSize: 12,
  },
  sessionsContainer: {
    flex: 1,
    marginBottom: 16,
  },
  sectionTitle: {
    fontSize: 18,
    fontWeight: 'bold',
    color: '#fff',
    marginBottom: 12,
  },
  sessionsList: {
    flexGrow: 1,
  },
  sessionItem: {
    backgroundColor: '#111',
    padding: 16,
    borderRadius: 8,
    marginBottom: 8,
    borderWidth: 1,
    borderColor: '#333',
  },
  selectedSessionItem: {
    borderColor: '#007AFF',
    backgroundColor: '#001122',
  },
  sessionHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: 4,
  },
  sessionTitle: {
    color: '#fff',
    fontSize: 16,
    fontWeight: 'bold',
    flex: 1,
  },
  sessionStatus: {
    color: '#007AFF',
    fontSize: 12,
    textTransform: 'uppercase',
  },
  sessionPath: {
    color: '#888',
    fontSize: 14,
    marginBottom: 4,
  },
  sessionTime: {
    color: '#666',
    fontSize: 12,
  },
  emptyContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    padding: 32,
  },
  emptyText: {
    color: '#888',
    fontSize: 16,
    textAlign: 'center',
    marginBottom: 20,
  },
  createButton: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: '#007AFF',
    paddingHorizontal: 20,
    paddingVertical: 12,
    borderRadius: 8,
  },
  createButtonText: {
    color: '#fff',
    fontSize: 16,
    fontWeight: 'bold',
    marginLeft: 8,
  },
  messageContainer: {
    backgroundColor: '#111',
    padding: 16,
    borderRadius: 8,
  },
  messageInputContainer: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    marginTop: 12,
  },
  messageInput: {
    flex: 1,
    backgroundColor: '#222',
    color: '#fff',
    padding: 12,
    borderRadius: 8,
    marginRight: 8,
    minHeight: 40,
    maxHeight: 100,
  },
  sendButton: {
    backgroundColor: '#007AFF',
    paddingHorizontal: 16,
    paddingVertical: 12,
    borderRadius: 8,
  },
  sendButtonText: {
    color: '#fff',
    fontWeight: 'bold',
  },
  // Modal styles
  modalOverlay: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: 'rgba(0, 0, 0, 0.8)',
    justifyContent: 'center',
    alignItems: 'center',
    padding: 16,
  },
  modalContent: {
    backgroundColor: '#111',
    borderRadius: 12,
    padding: 20,
    width: '100%',
    maxWidth: 400,
  },
  modalTitle: {
    fontSize: 20,
    fontWeight: 'bold',
    color: '#fff',
    marginBottom: 20,
    textAlign: 'center',
  },
  inputGroup: {
    marginBottom: 16,
  },
  inputLabel: {
    color: '#fff',
    fontSize: 14,
    fontWeight: 'bold',
    marginBottom: 6,
  },
  modalInput: {
    backgroundColor: '#222',
    color: '#fff',
    padding: 12,
    borderRadius: 8,
    fontSize: 16,
  },
  multilineInput: {
    minHeight: 80,
    textAlignVertical: 'top',
  },
  modalButtons: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    marginTop: 20,
  },
  modalButton: {
    flex: 1,
    paddingVertical: 12,
    borderRadius: 8,
    alignItems: 'center',
  },
  cancelButton: {
    backgroundColor: '#333',
    marginRight: 8,
  },
  cancelButtonText: {
    color: '#fff',
    fontSize: 16,
  },
  createButtonModal: {
    backgroundColor: '#007AFF',
    marginLeft: 8,
  },
});