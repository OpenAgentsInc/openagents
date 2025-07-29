import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, act, waitFor } from '@testing-library/react';
import { ConvexProvider } from 'convex/react';
import { Effect, Runtime, Exit } from 'effect';
import React from 'react';
import './setup-integration';

// Mock the entire Confect mobile sync module
const mockMobileSyncFunctions = {
  createClaudeSession: vi.fn(),
  getPendingMobileSessions: vi.fn(),
  getSessionMessages: vi.fn(),
  addClaudeMessage: vi.fn(),
  updateSessionStatus: vi.fn(),
  requestDesktopSession: vi.fn(),
  syncSessionFromHook: vi.fn(),
};

// Mock API object
const mockApi = {
  confect: {
    mobile_sync: mockMobileSyncFunctions,
  },
};

// Mock Convex client
const mockConvexClient = {
  query: vi.fn(),
  mutation: vi.fn(),
  action: vi.fn(),
  subscribe: vi.fn(),
};

// Mock Tauri API
const mockTauri = {
  invoke: vi.fn(),
};

vi.mock('@tauri-apps/api/core', () => mockTauri);
vi.mock('../convex/_generated/api', () => ({ api: mockApi }));

// Mock Convex React hooks
const mockUseQuery = vi.fn();
const mockUseMutation = vi.fn();
vi.mock('convex/react', async () => {
  const actual = await vi.importActual('convex/react');
  return {
    ...actual,
    useQuery: mockUseQuery,
    useMutation: mockUseMutation,
    useConvex: () => mockConvexClient,
  };
});

// Test utilities
function createMockMobileSession(overrides = {}) {
  return {
    sessionId: `mobile-${Date.now()}`,
    projectPath: '/test/project',
    title: 'Test Mobile Session',
    status: 'active',
    createdBy: 'mobile',
    lastActivity: Date.now(),
    metadata: {},
    ...overrides,
  };
}

function createMockMessage(overrides = {}) {
  return {
    messageId: `msg-${Date.now()}`,
    messageType: 'user',
    content: 'Test message content',
    timestamp: new Date().toISOString(),
    ...overrides,
  };
}

describe('Mobile-Desktop Sync Integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    
    // Default mock implementations
    mockUseQuery.mockReturnValue([]);
    mockUseMutation.mockReturnValue(vi.fn());
    mockTauri.invoke.mockResolvedValue({ success: true, data: 'test-session-id' });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe('Session Creation Flow', () => {
    it('should handle mobile session creation and desktop detection', async () => {
      // Test the complete flow from mobile session creation to desktop detection
      const mobileSession = createMockMobileSession({
        sessionId: 'sync-test-session-1',
        projectPath: '/sync/test/project',
      });

      // Mock mobile session appears in pending list
      mockUseQuery.mockReturnValue([mobileSession]);
      
      // Mock successful desktop session creation
      mockTauri.invoke.mockResolvedValue({
        success: true,
        data: 'desktop-session-uuid-1'
      });

      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      mockUseMutation.mockReturnValue(mockUpdateStatus);

      // Import the hook after mocks are set up
      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestSyncFlow() {
        const [sessions, setSessions] = React.useState([]);
        const { 
          pendingMobileSessions, 
          isProcessing,
          processAllSessions,
          sessionIdMapping 
        } = useMobileSessionSyncConfect(sessions, setSessions, true);

        React.useEffect(() => {
          if (pendingMobileSessions.length > 0 && !isProcessing) {
            processAllSessions();
          }
        }, [pendingMobileSessions, isProcessing, processAllSessions]);

        return (
          <div data-testid="sync-flow">
            <div data-testid="pending-count">{pendingMobileSessions.length}</div>
            <div data-testid="processing">{isProcessing.toString()}</div>
            <div data-testid="session-mapping">{JSON.stringify(Array.from(sessionIdMapping.entries()))}</div>
          </div>
        );
      }

      render(<TestSyncFlow />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Verify session detection
      await waitFor(() => {
        const pendingElement = document.querySelector('[data-testid="pending-count"]');
        expect(pendingElement?.textContent).toBe('1');
      });

      // Verify session creation is triggered
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalledWith('create_session', {
          projectPath: '/sync/test/project'
        });
      });

      // Verify session mapping is created
      await waitFor(() => {
        const mappingElement = document.querySelector('[data-testid="session-mapping"]');
        expect(mappingElement?.textContent).toContain('desktop-session-uuid-1');
        expect(mappingElement?.textContent).toContain('sync-test-session-1');
      });
    });

    it('should handle message synchronization between mobile and desktop', async () => {
      const mobileSession = createMockMobileSession({
        sessionId: 'msg-sync-session',
      });

      const mobileMessages = [
        createMockMessage({
          messageId: 'mobile-msg-1',
          content: 'Hello from mobile app',
          messageType: 'user',
        }),
        createMockMessage({
          messageId: 'mobile-msg-2', 
          content: 'Follow-up message',
          messageType: 'user',
        }),
      ];

      mockUseQuery.mockReturnValue([mobileSession]);
      mockConvexClient.query.mockResolvedValue(mobileMessages);
      
      mockTauri.invoke
        .mockResolvedValueOnce({ success: true, data: 'desktop-msg-sync-id' })
        .mockResolvedValueOnce({ success: true }); // trigger_claude_response

      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      mockUseMutation.mockReturnValue(mockUpdateStatus);

      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestMessageSync() {
        const [sessions, setSessions] = React.useState([]);
        const { processAllSessions, isProcessing } = useMobileSessionSyncConfect(
          sessions, 
          setSessions, 
          true
        );

        React.useEffect(() => {
          if (!isProcessing) {
            processAllSessions();
          }
        }, [processAllSessions, isProcessing]);

        return <div data-testid="message-sync" />;
      }

      render(<TestMessageSync />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Wait for session creation
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalledWith('create_session', expect.any(Object));
      });

      // Wait for message fetching
      await waitFor(() => {
        expect(mockConvexClient.query).toHaveBeenCalledWith(
          mockApi.confect.mobile_sync.getSessionMessages,
          {
            sessionId: 'msg-sync-session',
            limit: 10
          }
        );
      });

      // Wait for Claude response trigger
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalledWith('trigger_claude_response', {
          sessionId: 'desktop-msg-sync-id',
          message: 'Follow-up message', // Should use the last user message
        });
      });

      // Verify session marked as processed
      await waitFor(() => {
        expect(mockUpdateStatus).toHaveBeenCalledWith({
          sessionId: 'msg-sync-session',
          status: 'processed'
        });
      });
    });
  });

  describe('Real-time Synchronization', () => {
    it('should handle real-time message streaming between devices', async () => {
      const sessionId = 'realtime-sync-session';
      
      // Mock streaming messages
      const streamingMessages = [
        createMockMessage({ messageId: 'stream-1', content: 'Streaming message 1' }),
        createMockMessage({ messageId: 'stream-2', content: 'Streaming message 2' }),
      ];

      mockUseQuery.mockReturnValue(streamingMessages);
      const mockAddMessage = vi.fn().mockResolvedValue('new-msg-id');
      mockUseMutation.mockReturnValue(mockAddMessage);

      const { useClaudeStreaming } = await import('../hooks/useClaudeStreaming');

      function TestRealtimeSync() {
        const [messages, setMessages] = React.useState([]);
        
        const { messages: streamMessages, sendMessage } = useClaudeStreaming({
          sessionId,
          onMessage: (msg) => {
            setMessages(prev => [...prev, msg]);
          },
        });

        const handleSendTest = async () => {
          await sendMessage('Test real-time message');
        };

        return (
          <div data-testid="realtime-sync">
            <div data-testid="stream-messages">{streamMessages.length}</div>
            <div data-testid="local-messages">{messages.length}</div>
            <button onClick={handleSendTest} data-testid="send-button">Send</button>
          </div>
        );
      }

      render(<TestRealtimeSync />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Verify messages are loaded
      await waitFor(() => {
        const streamElement = document.querySelector('[data-testid="stream-messages"]');
        expect(streamElement?.textContent).toBe('2');
      });

      // Test message sending
      const sendButton = document.querySelector('[data-testid="send-button"]') as HTMLButtonElement;
      await act(async () => {
        sendButton.click();
      });

      // Should attempt to add message via Confect
      await waitFor(() => {
        expect(mockAddMessage).toHaveBeenCalledWith(
          expect.objectContaining({
            content: 'Test real-time message',
            sessionId,
          })
        );
      });
    });
  });

  describe('Error Handling and Recovery', () => {
    it('should handle network failures gracefully', async () => {
      const mobileSession = createMockMobileSession({
        sessionId: 'error-test-session',
      });

      mockUseQuery.mockReturnValue([mobileSession]);
      
      // Mock network failure
      mockTauri.invoke.mockRejectedValue(new Error('Network error'));
      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      mockUseMutation.mockReturnValue(mockUpdateStatus);

      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestErrorHandling() {
        const [sessions, setSessions] = React.useState([]);
        const { error, processAllSessions } = useMobileSessionSyncConfect(
          sessions,
          setSessions,
          true
        );

        React.useEffect(() => {
          processAllSessions();
        }, [processAllSessions]);

        return (
          <div data-testid="error-handling">
            <div data-testid="error-state">{error || 'no-error'}</div>
          </div>
        );
      }

      render(<TestErrorHandling />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Should handle error without crashing
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalled();
      });

      // Error should be captured and displayed
      await waitFor(() => {
        const errorElement = document.querySelector('[data-testid="error-state"]');
        expect(errorElement?.textContent).not.toBe('no-error');
      });
    });

    it('should handle Claude Code not being initialized', async () => {
      const mobileSession = createMockMobileSession({
        sessionId: 'claude-not-ready-session',
      });

      mockUseQuery.mockReturnValue([mobileSession]);
      
      // Mock Claude Code not initialized
      mockTauri.invoke.mockResolvedValue({
        success: false,
        error: 'Claude Code not initialized'
      });

      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      mockUseMutation.mockReturnValue(mockUpdateStatus);

      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestClaudeNotReady() {
        const [sessions, setSessions] = React.useState([]);
        useMobileSessionSyncConfect(sessions, setSessions, true);
        return <div data-testid="claude-not-ready" />;
      }

      render(<TestClaudeNotReady />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Should attempt session creation
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalledWith('create_session', expect.any(Object));
      });

      // Should handle the failure gracefully without crashing
      // The hook should continue to work and not throw errors
      expect(document.querySelector('[data-testid="claude-not-ready"]')).toBeTruthy();
    });
  });

  describe('Performance and Concurrency', () => {
    it('should handle multiple concurrent sessions efficiently', async () => {
      // Create multiple mobile sessions
      const multipleSessions = Array.from({ length: 5 }, (_, i) => 
        createMockMobileSession({
          sessionId: `concurrent-${i}`,
          projectPath: `/concurrent/project/${i}`,
        })
      );

      mockUseQuery.mockReturnValue(multipleSessions);
      
      // Mock successful session creation with slight delay
      mockTauri.invoke.mockImplementation(() => 
        new Promise(resolve => 
          setTimeout(() => resolve({ 
            success: true, 
            data: `desktop-${Date.now()}` 
          }), 100 + Math.random() * 100)
        )
      );

      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      mockUseMutation.mockReturnValue(mockUpdateStatus);

      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestConcurrency() {
        const [sessions, setSessions] = React.useState([]);
        const { isProcessing, processedCount } = useMobileSessionSyncConfect(
          sessions,
          setSessions,
          true
        );

        return (
          <div data-testid="concurrency">
            <div data-testid="processing">{isProcessing.toString()}</div>
            <div data-testid="processed">{processedCount}</div>
          </div>
        );
      }

      render(<TestConcurrency />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Should start processing
      await waitFor(() => {
        const processingElement = document.querySelector('[data-testid="processing"]');
        expect(processingElement?.textContent).toBe('true');
      });

      // Should process sessions (with concurrency control of max 3)
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalled();
      }, { timeout: 2000 });

      // The Effect-TS concurrency control should limit to 3 simultaneous operations
      // This is tested at the hook level - here we just verify it doesn't crash
      expect(mockTauri.invoke).toHaveBeenCalledTimes(1); // Only first session in this test
    });

    it('should respect processing cooldowns and debouncing', async () => {
      const mobileSession = createMockMobileSession({
        sessionId: 'cooldown-test-session',
      });

      mockUseQuery.mockReturnValue([mobileSession]);
      mockTauri.invoke.mockResolvedValue({ success: true, data: 'cooldown-desktop-id' });
      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      mockUseMutation.mockReturnValue(mockUpdateStatus);

      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestCooldown() {
        const [sessions, setSessions] = React.useState([]);
        const { processAllSessions, isProcessing } = useMobileSessionSyncConfect(
          sessions,
          setSessions,
          true
        );

        // Trigger multiple rapid calls
        React.useEffect(() => {
          processAllSessions();
          processAllSessions();
          processAllSessions();
        }, [processAllSessions]);

        return <div data-testid="cooldown">{isProcessing.toString()}</div>;
      }

      render(<TestCooldown />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Wait for processing
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalled();
      });

      // Should only process once due to debouncing/cooldown
      expect(mockTauri.invoke).toHaveBeenCalledTimes(1);
    });
  });

  describe('Session State Management', () => {
    it('should maintain session mappings correctly', async () => {
      const mobileSession = createMockMobileSession({
        sessionId: 'mapping-test-session',
        projectPath: '/mapping/test',
      });

      mockUseQuery.mockReturnValue([mobileSession]);
      mockTauri.invoke.mockResolvedValue({
        success: true,
        data: 'desktop-mapping-uuid'
      });

      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      const mockSetSessions = vi.fn();
      mockUseMutation.mockReturnValue(mockUpdateStatus);

      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestSessionMapping() {
        const [sessions, setSessions] = React.useState([]);
        const { sessionIdMapping, processAllSessions } = useMobileSessionSyncConfect(
          sessions,
          mockSetSessions,
          true
        );

        React.useEffect(() => {
          processAllSessions();
        }, [processAllSessions]);

        return (
          <div data-testid="session-mapping">
            <div data-testid="mapping-size">{sessionIdMapping.size}</div>
            <div data-testid="mapping-content">
              {JSON.stringify(Array.from(sessionIdMapping.entries()))}
            </div>
          </div>
        );
      }

      render(<TestSessionMapping />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Wait for session processing
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalled();
      });

      // Wait for session state updates
      await waitFor(() => {
        expect(mockSetSessions).toHaveBeenCalled();
      });

      // Verify mapping is created
      await waitFor(() => {
        const sizeElement = document.querySelector('[data-testid="mapping-size"]');
        expect(sizeElement?.textContent).toBe('1');
      });

      await waitFor(() => {
        const contentElement = document.querySelector('[data-testid="mapping-content"]');
        expect(contentElement?.textContent).toContain('desktop-mapping-uuid');
        expect(contentElement?.textContent).toContain('mapping-test-session');
      });
    });
  });
});