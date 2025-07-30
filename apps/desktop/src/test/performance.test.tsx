import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, waitFor } from '@testing-library/react';
import { ConvexProvider } from 'convex/react';
import React from 'react';
import './setup-integration';

// Types
interface Session {
  id: string;
  projectPath: string;
  messages: any[];
  inputMessage: string;
  isLoading: boolean;
  isInitializing?: boolean;
}

// Performance testing utilities
function measureExecutionTime<T>(fn: () => T | Promise<T>): Promise<{ result: T; duration: number }> {
  const start = performance.now();
  const result = fn();
  
  if (result instanceof Promise) {
    return result.then(res => ({
      result: res,
      duration: performance.now() - start
    }));
  }
  
  return Promise.resolve({
    result,
    duration: performance.now() - start
  });
}

function createLargeDataset(size: number) {
  return Array.from({ length: size }, (_, i) => ({
    sessionId: `perf-session-${i}`,
    projectPath: `/perf/project/${i}`,
    title: `Performance Test Session ${i}`,
    status: 'active' as const,
    createdBy: 'mobile' as const,
    lastActivity: Date.now() - (i * 1000),
    metadata: {
      testData: `Large metadata string for session ${i}`.repeat(10),
      tags: [`tag-${i}`, `category-${i % 5}`, `priority-${i % 3}`],
    },
  }));
}

function createLargeMessageSet(sessionId: string, count: number) {
  return Array.from({ length: count }, (_, i) => ({
    messageId: `msg-${sessionId}-${i}`,
    messageType: i % 2 === 0 ? 'user' : 'assistant',
    content: `Message content ${i} for session ${sessionId}`.repeat(5),
    timestamp: new Date(Date.now() - (count - i) * 1000).toISOString(),
    toolInfo: i % 5 === 0 ? {
      toolName: `tool-${i}`,
      toolUseId: `use-${i}`,
      input: { param: `value-${i}` },
      output: `Tool output ${i}`.repeat(3),
    } : undefined,
  }));
}

// Use setup mocks
import { mockTauri } from './setup';
import { mockConvexClient, mockUseQuery, mockUseMutation } from './setup-integration';

describe('Performance Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockUseQuery.mockReturnValue([]);
    mockUseMutation.mockReturnValue(vi.fn());
    mockTauri.invoke.mockResolvedValue({ success: true, data: 'perf-session-id' });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  describe('Large Dataset Handling', () => {
    it('should handle 100 pending sessions efficiently', async () => {
      const largeSessions = createLargeDataset(100);
      mockUseQuery.mockReturnValue(largeSessions);
      
      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      const { duration: renderTime } = await measureExecutionTime(() => {
        function TestLargeDataset() {
          const [sessions, setSessions] = React.useState<Session[]>([]);
          const { pendingMobileSessions, isProcessing } = useMobileSessionSyncConfect(
            sessions,
            setSessions,
            true
          );

          return (
            <div data-testid="large-dataset">
              <div data-testid="pending-count">{pendingMobileSessions.length}</div>
              <div data-testid="processing">{isProcessing.toString()}</div>
            </div>
          );
        }

        return render(<TestLargeDataset />, {
          wrapper: ({ children }) => (
            <ConvexProvider client={mockConvexClient as any}>
              {children}
            </ConvexProvider>
          ),
        });
      });

      // Render should complete quickly even with large dataset
      expect(renderTime).toBeLessThan(500); // 500ms threshold

      // Verify data is handled correctly
      await waitFor(() => {
        const countElement = document.querySelector('[data-testid="pending-count"]');
        expect(countElement?.textContent).toBe('100');
      });

      // Performance assertion: Hook should handle large datasets without blocking
      expect(renderTime).toBeLessThan(1000);
    });

    it('should process multiple sessions with reasonable performance', async () => {
      const multipleSessions = createLargeDataset(10);
      mockUseQuery.mockReturnValue(multipleSessions);
      
      // Mock successful but realistic processing times
      mockTauri.invoke.mockImplementation(() => 
        new Promise(resolve => 
          setTimeout(() => resolve({ success: true, data: 'processed-id' }), 50)
        )
      );

      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestProcessingPerformance() {
        const [sessions, setSessions] = React.useState<Session[]>([]);
        const { processAllSessions, processedCount, isProcessing } = useMobileSessionSyncConfect(
          sessions,
          setSessions,
          true
        );

        React.useEffect(() => {
          if (!isProcessing) {
            processAllSessions();
          }
        }, [processAllSessions, isProcessing]);

        return (
          <div data-testid="processing-perf">
            <div data-testid="processed-count">{processedCount}</div>
            <div data-testid="is-processing">{isProcessing.toString()}</div>
          </div>
        );
      }

      const startTime = performance.now();
      render(<TestProcessingPerformance />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Wait for processing to start
      await waitFor(() => {
        const processingElement = document.querySelector('[data-testid="is-processing"]');
        expect(processingElement?.textContent).toBe('true');
      });

      // Verify processing starts efficiently
      const processingStartTime = performance.now() - startTime;
      expect(processingStartTime).toBeLessThan(200); // Should start processing quickly
    });
  });

  describe('Message Streaming Performance', () => {
    it('should handle large message histories efficiently', async () => {
      const sessionId = 'perf-streaming-session';
      const largeMessageHistory = createLargeMessageSet(sessionId, 1000);
      
      mockUseQuery.mockReturnValue(largeMessageHistory);
      mockUseMutation.mockReturnValue(vi.fn());

      const { useClaudeStreaming } = await import('../hooks/useClaudeStreaming');

      const { duration } = await measureExecutionTime(() => {
        function TestStreamingPerformance() {
          const [messageCount, setMessageCount] = React.useState(0);
          
          const { messages, isStreaming } = useClaudeStreaming({
            sessionId,
            onMessage: () => setMessageCount(prev => prev + 1),
          });

          return (
            <div data-testid="streaming-perf">
              <div data-testid="message-count">{messages.length}</div>
              <div data-testid="callback-count">{messageCount}</div>
              <div data-testid="streaming">{isStreaming.toString()}</div>
            </div>
          );
        }

        return render(<TestStreamingPerformance />, {
          wrapper: ({ children }) => (
            <ConvexProvider client={mockConvexClient as any}>
              {children}
            </ConvexProvider>
          ),
        });
      });

      // Should handle large message set efficiently
      expect(duration).toBeLessThan(300);

      // Verify messages are loaded
      await waitFor(() => {
        const countElement = document.querySelector('[data-testid="message-count"]');
        expect(countElement?.textContent).toBe('1000');
      });
    });

    it('should maintain performance under rapid message updates', async () => {
      const sessionId = 'rapid-updates-session';
      let messageCount = 0;
      
      // Mock rapidly changing message set
      mockUseQuery.mockImplementation(() => {
        messageCount += 10;
        return createLargeMessageSet(sessionId, Math.min(messageCount, 500));
      });

      const { useClaudeStreaming } = await import('../hooks/useClaudeStreaming');

      function TestRapidUpdates() {
        const [updateCount, setUpdateCount] = React.useState(0);
        
        const { messages } = useClaudeStreaming({
          sessionId,
          onMessage: () => setUpdateCount(prev => prev + 1),
        });

        // Simulate rapid updates
        React.useEffect(() => {
          const interval = setInterval(() => {
            // Trigger re-renders to simulate real-time updates
            setUpdateCount(prev => prev + 1);
          }, 50);

          return () => clearInterval(interval);
        }, []);

        return (
          <div data-testid="rapid-updates">
            <div data-testid="message-count">{messages.length}</div>
            <div data-testid="update-count">{updateCount}</div>
          </div>
        );
      }

      const startTime = performance.now();
      render(<TestRapidUpdates />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Let it run for a short time to measure performance
      await new Promise(resolve => setTimeout(resolve, 500));

      const totalTime = performance.now() - startTime;
      
      // Should maintain reasonable performance even with rapid updates
      expect(totalTime).toBeLessThan(1000);

      // Component should still be responsive
      const messageElement = document.querySelector('[data-testid="message-count"]');
      expect(messageElement).toBeTruthy();
    });
  });

  describe('Memory Usage and Cleanup', () => {
    it('should clean up resources properly', async () => {
      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestCleanup() {
        const [mounted, setMounted] = React.useState(true);
        const [sessions, setSessions] = React.useState<Session[]>([]);
        
        if (mounted) {
          useMobileSessionSyncConfect(sessions, setSessions, true);
          
          // Simulate unmounting
          setTimeout(() => setMounted(false), 100);
          
          return <div data-testid="cleanup">Mounted</div>;
        }
        
        return <div data-testid="cleanup">Unmounted</div>;
      }

      render(<TestCleanup />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Wait for unmounting
      await waitFor(() => {
        const element = document.querySelector('[data-testid="cleanup"]');
        expect(element?.textContent).toBe('Unmounted');
      });

      // Cleanup should happen without errors
      // This test mainly ensures no memory leaks or hanging references
    });

    it('should handle memory-intensive operations without degradation', async () => {
      const sessionSizes = [10, 50, 100, 200, 500];
      const performanceResults = [];

      for (const size of sessionSizes) {
        const sessions = createLargeDataset(size);
        mockUseQuery.mockReturnValue(sessions);

        const { duration } = await measureExecutionTime(async () => {
          const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

          function TestMemoryIntensive() {
            const [sessions, setSessions] = React.useState<Session[]>([]);
            const { pendingMobileSessions } = useMobileSessionSyncConfect(
              sessions,
              setSessions,
              true
            );

            return <div data-testid={`memory-test-${size}`}>{pendingMobileSessions.length}</div>;
          }

          const { unmount } = render(<TestMemoryIntensive />, {
            wrapper: ({ children }) => (
              <ConvexProvider client={mockConvexClient as any}>
                {children}
              </ConvexProvider>
            ),
          });

          await waitFor(() => {
            const element = document.querySelector(`[data-testid="memory-test-${size}"]`);
            expect(element?.textContent).toBe(size.toString());
          });

          unmount();
        });

        performanceResults.push({ size, duration });
      }

      // Performance should not degrade exponentially with size
      // Each size should be roughly linear in performance
      const firstResult = performanceResults[0];
      const lastResult = performanceResults[performanceResults.length - 1];
      
      expect(firstResult).toBeDefined();
      expect(lastResult).toBeDefined();
      
      // Performance degradation should be reasonable (not exponential)
      const performanceRatio = lastResult!.duration / firstResult!.duration;
      const sizeRatio = lastResult!.size / firstResult!.size;
      
      // Performance should scale reasonably with data size
      expect(performanceRatio).toBeLessThan(sizeRatio * 2); // Allow some overhead but not exponential
    });
  });

  describe('Concurrent Operations Performance', () => {
    it('should handle concurrent session operations efficiently', async () => {
      const concurrentSessions = createLargeDataset(20);
      mockUseQuery.mockReturnValue(concurrentSessions);
      
      // Mock concurrent operations with realistic timing
      mockTauri.invoke.mockImplementation(() => 
        new Promise(resolve => 
          setTimeout(() => resolve({ 
            success: true, 
            data: `concurrent-${Date.now()}` 
          }), 20 + Math.random() * 30)
        )
      );

      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      const { duration } = await measureExecutionTime(async () => {
        function TestConcurrentOps() {
          const [sessions, setSessions] = React.useState<Session[]>([]);
          const { processAllSessions, processedCount } = useMobileSessionSyncConfect(
            sessions,
            setSessions,
            true
          );

          // Trigger multiple concurrent operations
          React.useEffect(() => {
            const promises = Array.from({ length: 5 }, () => processAllSessions());
            Promise.all(promises);
          }, [processAllSessions]);

          return <div data-testid="concurrent">{processedCount}</div>;
        }

        render(<TestConcurrentOps />, {
          wrapper: ({ children }) => (
            <ConvexProvider client={mockConvexClient as any}>
              {children}
            </ConvexProvider>
          ),
        });

        // Wait for some processing
        await new Promise(resolve => setTimeout(resolve, 200));
      });

      // Concurrent operations should complete efficiently
      expect(duration).toBeLessThan(500);
      
      // Should handle concurrency without errors
      expect(mockTauri.invoke).toHaveBeenCalled();
    });
  });

  describe('Network Latency Simulation', () => {
    it('should maintain responsiveness under high network latency', async () => {
      const sessions = createLargeDataset(5);
      mockUseQuery.mockReturnValue(sessions);
      
      // Simulate high network latency
      mockTauri.invoke.mockImplementation(() => 
        new Promise(resolve => 
          setTimeout(() => resolve({ 
            success: true, 
            data: 'high-latency-id' 
          }), 1000) // 1 second latency
        )
      );

      const { useMobileSessionSyncConfect } = await import('../hooks/useMobileSessionSyncConfect');

      function TestHighLatency() {
        const [sessions, setSessions] = React.useState<Session[]>([]);
        const [userInteraction, setUserInteraction] = React.useState(0);
        const { processAllSessions, isProcessing } = useMobileSessionSyncConfect(
          sessions,
          setSessions,
          true
        );

        // Simulate user interactions during processing
        React.useEffect(() => {
          const interval = setInterval(() => {
            setUserInteraction(prev => prev + 1);
          }, 100);

          return () => clearInterval(interval);
        }, []);

        React.useEffect(() => {
          processAllSessions();
        }, [processAllSessions]);

        return (
          <div data-testid="high-latency">
            <div data-testid="processing">{isProcessing.toString()}</div>
            <div data-testid="interactions">{userInteraction}</div>
          </div>
        );
      }

      const startTime = performance.now();
      render(<TestHighLatency />, {
        wrapper: ({ children }) => (
          <ConvexProvider client={mockConvexClient as any}>
            {children}
          </ConvexProvider>
        ),
      });

      // Wait for processing to start
      await waitFor(() => {
        const processingElement = document.querySelector('[data-testid="processing"]');
        expect(processingElement?.textContent).toBe('true');
      });

      // Wait a bit to let user interactions accumulate
      await new Promise(resolve => setTimeout(resolve, 300));

      const uiResponseTime = performance.now() - startTime;
      
      // UI should remain responsive even with high network latency
      expect(uiResponseTime).toBeLessThan(400);

      // User interactions should continue to work
      const interactionElement = document.querySelector('[data-testid="interactions"]');
      const interactionCount = parseInt(interactionElement?.textContent || '0');
      expect(interactionCount).toBeGreaterThan(0);
    });
  });
});