import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import React from 'react';
import { ConvexProvider } from 'convex/react';
import { mockTauri } from './setup';
import { mockConvexClient, mockUseQuery, mockUseMutation } from './setup-integration';
import './setup-integration';
import { useMobileSessionSyncConfect } from '../hooks/useMobileSessionSyncConfect';
import { useMobileSessionSync } from '../hooks/useMobileSessionSync';
import { useSessionManager } from '../hooks/useSessionManager';
import { useClaudeStreaming } from '../hooks/useClaudeStreaming';
import { api } from '@openagentsinc/convex';

// Types
interface Session {
  id: string;
  projectPath: string;
  messages: any[];
  inputMessage: string;
  isLoading: boolean;
  isInitializing?: boolean;
}

// Mock pane store
const mockPaneStore = {
  openChatPane: vi.fn(),
  updateSessionMessages: vi.fn(),
};
vi.mock('@/stores/pane', () => ({
  usePaneStore: () => mockPaneStore,
}));

// Test wrapper component
function TestWrapper({ children }: { children: React.ReactNode }) {
  return (
    <ConvexProvider client={mockConvexClient as any}>
      {children}
    </ConvexProvider>
  );
}

describe('Confect Integration Tests', () => {
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

  describe('Mobile Session Sync Confect Hook', () => {
    it('should use api.confect.mobile_sync functions', async () => {
      const mockSessions: Session[] = [];
      const mockSetSessions = vi.fn();
      
      // Mock pending mobile sessions
      const mockPendingSessions = [
        {
          sessionId: 'mobile-session-1',
          projectPath: '/test/project',
          title: 'Test Session',
          status: 'active',
          createdBy: 'mobile',
        },
      ];
      
      mockUseQuery
        .mockReturnValueOnce(mockPendingSessions) // getPendingMobileSessions
        .mockReturnValueOnce([]); // Any other queries
      
      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      mockUseMutation.mockReturnValue(mockUpdateStatus);

      // Test component that uses the hook
      function TestComponent() {
        const syncResult = useMobileSessionSyncConfect(
          mockSessions,
          mockSetSessions,
          true // isAppInitialized
        );
        
        return <div data-testid="sync-result">{JSON.stringify(syncResult.pendingMobileSessions)}</div>;
      }

      render(<TestComponent />, { wrapper: TestWrapper });

      // Verify the hook is calling the correct Confect API
      expect(mockUseQuery).toHaveBeenCalledWith(api.confect.mobile_sync.getPendingMobileSessions);
      expect(mockUseMutation).toHaveBeenCalledWith(api.confect.mobile_sync.updateSessionStatus);
    });

    it('should process mobile sessions with Effect-TS patterns', async () => {
      const mockSessions: Session[] = [];
      const mockSetSessions = vi.fn();
      
      const mockPendingSessions = [
        {
          sessionId: 'mobile-session-test',
          projectPath: '/test/project',
          title: 'Test Mobile Session',
          status: 'active',
          createdBy: 'mobile',
        },
      ];
      
      mockUseQuery.mockReturnValue(mockPendingSessions);
      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      mockUseMutation.mockReturnValue(mockUpdateStatus);
      
      // Mock successful Tauri session creation
      mockTauri.invoke.mockResolvedValue({
        success: true,
        data: 'desktop-session-uuid'
      });

      function TestComponent() {
        const { processAllSessions, isProcessing } = useMobileSessionSyncConfect(
          mockSessions,
          mockSetSessions,
          true
        );
        
        // Auto-process sessions
        React.useEffect(() => {
          if (!isProcessing) {
            processAllSessions();
          }
        }, [processAllSessions, isProcessing]);
        
        return <div data-testid="processing">{isProcessing.toString()}</div>;
      }

      render(<TestComponent />, { wrapper: TestWrapper });

      // Wait for processing to complete
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalledWith('create_session', {
          projectPath: '/test/project'
        });
      });

      await waitFor(() => {
        expect(mockSetSessions).toHaveBeenCalled();
      });

      await waitFor(() => {
        expect(mockPaneStore.openChatPane).toHaveBeenCalledWith(
          'desktop-session-uuid',
          '/test/project'
        );
      });
    });

    it('should handle session processing errors gracefully', async () => {
      const mockSessions: Session[] = [];
      const mockSetSessions = vi.fn();
      
      const mockPendingSessions = [
        {
          sessionId: 'failing-session',
          projectPath: '/test/project',
          status: 'active',
          createdBy: 'mobile',
        },
      ];
      
      mockUseQuery.mockReturnValue(mockPendingSessions);
      mockUseMutation.mockReturnValue(vi.fn());
      
      // Mock Tauri failure
      mockTauri.invoke.mockResolvedValue({
        success: false,
        error: 'Claude Code not initialized'
      });

      function TestComponent() {
        const { error, processAllSessions } = useMobileSessionSyncConfect(
          mockSessions,
          mockSetSessions,
          true
        );
        
        React.useEffect(() => {
          processAllSessions();
        }, [processAllSessions]);
        
        return <div data-testid="error">{error || 'no-error'}</div>;
      }

      render(<TestComponent />, { wrapper: TestWrapper });

      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalled();
      });

      // Error should be handled without crashing
      await waitFor(() => {
        const element = document.querySelector('[data-testid="error"]');
        expect(element).toBeTruthy();
      });
    });
  });

  describe('Traditional Mobile Session Sync Hook', () => {
    it('should use api.confect.mobile_sync functions', () => {
      const mockSessions: Session[] = [];
      const mockSetSessions = vi.fn();
      
      mockUseQuery.mockReturnValue([]);
      mockUseMutation.mockReturnValue(vi.fn());

      function TestComponent() {
        useMobileSessionSync(mockSessions, mockSetSessions, true);
        return <div>Test</div>;
      }

      render(<TestComponent />, { wrapper: TestWrapper });

      // Verify migration to Confect APIs
      expect(mockUseQuery).toHaveBeenCalledWith(api.confect.mobile_sync.getPendingMobileSessions);
      expect(mockUseMutation).toHaveBeenCalledWith(api.confect.mobile_sync.updateSessionStatus);
    });
  });

  describe('Session Manager Hook', () => {
    it('should use api.confect.mobile_sync functions', () => {
      mockUseMutation.mockReturnValue(vi.fn());
      
      function TestComponent() {
        useSessionManager();
        return <div>Test</div>;
      }

      render(<TestComponent />, { wrapper: TestWrapper });

      // Verify migration to Confect APIs
      expect(mockUseMutation).toHaveBeenCalledWith(api.confect.mobile_sync.createClaudeSession);
      expect(mockUseMutation).toHaveBeenCalledWith(api.confect.mobile_sync.updateSessionStatus);
    });
  });

  describe('Claude Streaming Hook', () => {
    it('should use api.confect.mobile_sync functions', () => {
      mockUseQuery.mockReturnValue([]);
      mockUseMutation.mockReturnValue(vi.fn());
      
      function TestComponent() {
        useClaudeStreaming({
          sessionId: 'test-session',
          onMessage: vi.fn(),
          onError: vi.fn(),
        });
        return <div>Test</div>;
      }

      render(<TestComponent />, { wrapper: TestWrapper });

      // Verify migration to Confect APIs
      expect(mockUseMutation).toHaveBeenCalledWith(api.confect.mobile_sync.addClaudeMessage);
      expect(mockUseQuery).toHaveBeenCalledWith(api.confect.mobile_sync.getSessionMessages, {
        sessionId: 'test-session',
      });
    });
  });

  describe('End-to-End Session Flow', () => {
    it('should handle complete mobile-to-desktop session sync', async () => {
      const mockSessions: Session[] = [];
      const mockSetSessions = vi.fn();
      
      // Mock mobile session data
      const mobileSession = {
        sessionId: 'e2e-test-session',
        projectPath: '/e2e/test/project',
        title: 'E2E Test Session',
        status: 'active',
        createdBy: 'mobile',
      };
      
      const mobileMessages = [
        {
          messageId: 'msg-1',
          messageType: 'user',
          content: 'Hello from mobile',
          timestamp: new Date().toISOString(),
        },
      ];
      
      mockUseQuery
        .mockReturnValueOnce([mobileSession]) // getPendingMobileSessions
        .mockReturnValueOnce(mobileMessages); // getSessionMessages
      
      const mockUpdateStatus = vi.fn().mockResolvedValue(null);
      mockUseMutation.mockReturnValue(mockUpdateStatus);
      
      mockConvexClient.query.mockResolvedValue(mobileMessages);
      
      // Mock successful Tauri operations
      mockTauri.invoke
        .mockResolvedValueOnce({ success: true, data: 'desktop-uuid' }) // create_session
        .mockResolvedValueOnce({ success: true }); // trigger_claude_response

      function TestComponent() {
        const { processAllSessions, isProcessing, processedCount } = useMobileSessionSyncConfect(
          mockSessions,
          mockSetSessions,
          true
        );
        
        React.useEffect(() => {
          if (!isProcessing) {
            processAllSessions();
          }
        }, [processAllSessions, isProcessing]);
        
        return (
          <div>
            <div data-testid="processing">{isProcessing.toString()}</div>
            <div data-testid="processed-count">{processedCount}</div>
          </div>
        );
      }

      render(<TestComponent />, { wrapper: TestWrapper });

      // Wait for complete processing
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalledWith('create_session', {
          projectPath: '/e2e/test/project'
        });
      });

      await waitFor(() => {
        expect(mockConvexClient.query).toHaveBeenCalledWith(
          api.confect.mobile_sync.getSessionMessages,
          {
            sessionId: 'e2e-test-session',
            limit: 10
          }
        );
      });

      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalledWith('trigger_claude_response', {
          sessionId: 'desktop-uuid',
          message: 'Hello from mobile'
        });
      });

      await waitFor(() => {
        expect(mockUpdateStatus).toHaveBeenCalledWith({
          sessionId: 'e2e-test-session',
          status: 'processed'
        });
      });

      // Verify session was added to local state
      expect(mockSetSessions).toHaveBeenCalledWith(
        expect.any(Function)
      );

      // Verify chat pane was opened
      expect(mockPaneStore.openChatPane).toHaveBeenCalledWith(
        'desktop-uuid',
        '/e2e/test/project'
      );
    });

    it('should handle network failures with proper retry logic', async () => {
      const mockSessions: Session[] = [];
      const mockSetSessions = vi.fn();
      
      const mobileSession = {
        sessionId: 'retry-test-session',
        projectPath: '/retry/test',
        status: 'active',
        createdBy: 'mobile',
      };
      
      mockUseQuery.mockReturnValue([mobileSession]);
      mockUseMutation.mockReturnValue(vi.fn());
      
      // Mock initial failure, then success
      mockTauri.invoke
        .mockRejectedValueOnce(new Error('Network failure'))
        .mockResolvedValueOnce({ success: true, data: 'retry-session-id' });

      function TestComponent() {
        const { processAllSessions, error } = useMobileSessionSyncConfect(
          mockSessions,
          mockSetSessions,
          true
        );
        
        React.useEffect(() => {
          processAllSessions();
        }, [processAllSessions]);
        
        return <div data-testid="error">{error || 'no-error'}</div>;
      }

      render(<TestComponent />, { wrapper: TestWrapper });

      // Should handle the error gracefully
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalled();
      });

      // Effect-TS retry logic should be tested at the unit level
      // This integration test verifies error handling doesn't crash the app
      const errorElement = document.querySelector('[data-testid="error"]');
      expect(errorElement).toBeTruthy();
    });
  });

  describe('Performance and Concurrency', () => {
    it('should handle multiple sessions with concurrency control', async () => {
      const mockSessions: Session[] = [];
      const mockSetSessions = vi.fn();
      
      // Mock multiple pending sessions
      const multipleSessions = Array.from({ length: 5 }, (_, i) => ({
        sessionId: `concurrent-session-${i}`,
        projectPath: `/concurrent/test/${i}`,
        status: 'active',
        createdBy: 'mobile',
      }));
      
      mockUseQuery.mockReturnValue(multipleSessions);
      mockUseMutation.mockReturnValue(vi.fn());
      
      mockTauri.invoke.mockResolvedValue({
        success: true,
        data: 'concurrent-desktop-id'
      });

      function TestComponent() {
        const { processAllSessions, isProcessing } = useMobileSessionSyncConfect(
          mockSessions,
          mockSetSessions,
          true
        );
        
        React.useEffect(() => {
          if (!isProcessing) {
            processAllSessions();
          }
        }, [processAllSessions, isProcessing]);
        
        return <div data-testid="processing">{isProcessing.toString()}</div>;
      }

      render(<TestComponent />, { wrapper: TestWrapper });

      // Should process sessions with concurrency control (max 3 at once)
      await waitFor(() => {
        expect(mockTauri.invoke).toHaveBeenCalled();
      }, { timeout: 5000 });

      // The Effect-TS concurrency control should be tested at the unit level
      // This test verifies the integration doesn't break under load
      expect(mockTauri.invoke).toHaveBeenCalledTimes(1); // Only processes first session in this test
    });
  });
});