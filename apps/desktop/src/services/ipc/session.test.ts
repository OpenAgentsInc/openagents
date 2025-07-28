import { describe, it, expect, vi, beforeEach } from 'vitest'
import { Effect, Option } from 'effect'
import { SessionCommands, Session, Message } from './session'
import { SessionError, SessionNotFoundError, IPCError } from './errors'
import { CommandResult } from './command'
import {
  expectEffect,
  expectEffectError,
  generateTestData
} from '@/test/effect-test-utils'
import { invoke } from '@tauri-apps/api/core'

// Mock Tauri API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}))

describe('Session IPC Commands', () => {
  beforeEach(() => {
    vi.clearAllMocks()
  })

  describe('discover', () => {
    it('should discover Claude session ID', async () => {
      const mockSessionId = 'claude-session-123'
      const mockResult: CommandResult<string> = {
        success: true,
        data: mockSessionId
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.discover(),
        (result) => {
          expect(result).toBe(mockSessionId)
          expect(invoke).toHaveBeenCalledWith('discover_claude', {})
        }
      )
    })

    it('should handle discovery failure', async () => {
      const mockResult: CommandResult<string> = {
        success: false,
        error: 'Claude not found'
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffectError(
        SessionCommands.discover(),
        (error) => {
          expect(error).toBeInstanceOf(IPCError)
          expect(error.command).toBe('discover_claude')
        }
      )
    })
  })

  describe('create', () => {
    it('should create session with project path', async () => {
      const projectPath = '/home/user/project'
      const sessionId = 'session-456'
      const mockResult: CommandResult<string> = {
        success: true,
        data: sessionId
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.create(projectPath),
        (result) => {
          expect(result).toBe(sessionId)
          expect(invoke).toHaveBeenCalledWith('create_session', {
            project_path: projectPath
          })
        }
      )
    })

    it('should handle session creation error', async () => {
      const projectPath = '/invalid/path'
      const mockResult: CommandResult<string> = {
        success: false,
        error: 'Invalid project path'
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffectError(
        SessionCommands.create(projectPath),
        (error) => {
          expect(error).toBeInstanceOf(SessionError)
          expect(error.operation).toBe('create')
          expect(error.message).toContain(`Failed to create session for project: ${projectPath}`)
        }
      )
    })

    it('should handle paths with special characters', async () => {
      const projectPath = '/home/user/my project (2024)/src'
      const sessionId = 'session-789'
      const mockResult: CommandResult<string> = {
        success: true,
        data: sessionId
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.create(projectPath),
        (result) => {
          expect(result).toBe(sessionId)
        }
      )
    })
  })

  describe('sendMessage', () => {
    it('should send message to session', async () => {
      const sessionId = 'session-123'
      const message = 'Hello, Claude!'
      const mockResult: CommandResult<void> = {
        success: true
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.sendMessage(sessionId, message),
        () => {
          expect(invoke).toHaveBeenCalledWith('send_message', {
            session_id: sessionId,
            message: message
          })
        }
      )
    })

    it('should handle send message error', async () => {
      const sessionId = 'invalid-session'
      const message = 'Test message'
      const mockResult: CommandResult<void> = {
        success: false,
        error: 'Session not found'
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffectError(
        SessionCommands.sendMessage(sessionId, message),
        (error) => {
          expect(error).toBeInstanceOf(SessionError)
          expect(error.operation).toBe('send')
          expect(error.sessionId).toBe(sessionId)
          expect(error.message).toContain(`Failed to send message to session ${sessionId}`)
        }
      )
    })

    it('should handle multi-line messages', async () => {
      const sessionId = 'session-123'
      const multiLineMessage = `Line 1
Line 2
Line 3 with "quotes" and 'apostrophes'`
      const mockResult: CommandResult<void> = {
        success: true
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.sendMessage(sessionId, multiLineMessage),
        () => {
          expect(invoke).toHaveBeenCalledWith('send_message', {
            session_id: sessionId,
            message: multiLineMessage
          })
        }
      )
    })
  })

  describe('stop', () => {
    it('should stop session', async () => {
      const sessionId = 'session-123'
      const mockResult: CommandResult<void> = {
        success: true
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.stop(sessionId),
        () => {
          expect(invoke).toHaveBeenCalledWith('stop_session', {
            session_id: sessionId
          })
        }
      )
    })

    it('should handle stop session error', async () => {
      const sessionId = 'session-123'
      const mockResult: CommandResult<void> = {
        success: false,
        error: 'Failed to stop session'
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffectError(
        SessionCommands.stop(sessionId),
        (error) => {
          expect(error).toBeInstanceOf(SessionError)
          expect(error.operation).toBe('stop')
          expect(error.sessionId).toBe(sessionId)
        }
      )
    })
  })

  describe('get', () => {
    it('should get session details', async () => {
      const sessionId = 'session-123'
      const mockSession: Session = {
        id: sessionId,
        title: 'Test Session',
        project_path: '/home/user/project',
        created_at: new Date().toISOString(),
        is_active: true,
        messages: [
          {
            id: 'msg-1',
            role: 'user',
            content: 'Hello',
            timestamp: new Date().toISOString()
          },
          {
            id: 'msg-2',
            role: 'assistant',
            content: 'Hi there!',
            timestamp: new Date().toISOString(),
            tool_uses: [{
              id: 'tool-1',
              name: 'calculator',
              input: { operation: 'add', a: 1, b: 2 }
            }]
          }
        ]
      }
      const mockResult: CommandResult<Session> = {
        success: true,
        data: mockSession
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.get(sessionId),
        (result) => {
          expect(Option.isSome(result)).toBe(true)
          if (Option.isSome(result)) {
            expect(result.value).toEqual(mockSession)
          }
        }
      )
    })

    it('should return None for non-existent session', async () => {
      const sessionId = 'non-existent'
      const mockResult: CommandResult<Session> = {
        success: false,
        error: 'Session not found'
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffectError(
        SessionCommands.get(sessionId),
        (error) => {
          expect(error).toBeInstanceOf(SessionNotFoundError)
          expect(error.sessionId).toBe(sessionId)
        }
      )
    })

    it('should handle empty message list', async () => {
      const sessionId = 'session-empty'
      const mockSession: Session = {
        id: sessionId,
        title: 'Empty Session',
        project_path: '/home/user/empty',
        created_at: new Date().toISOString(),
        is_active: false,
        messages: []
      }
      const mockResult: CommandResult<Session> = {
        success: true,
        data: mockSession
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.get(sessionId),
        (result) => {
          expect(Option.isSome(result)).toBe(true)
          if (Option.isSome(result)) {
            expect(result.value.messages).toHaveLength(0)
          }
        }
      )
    })
  })

  describe('handleEvent', () => {
    it('should handle session events', async () => {
      const sessionId = 'session-123'
      const event = {
        type: 'message_received',
        data: { content: 'New message' }
      }
      const mockResult: CommandResult<void> = {
        success: true
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.handleEvent(sessionId, event),
        () => {
          expect(invoke).toHaveBeenCalledWith('handle_session_event', {
            session_id: sessionId,
            event: event
          })
        }
      )
    })

    it('should handle event processing error', async () => {
      const sessionId = 'session-123'
      const event = {
        type: 'invalid_event',
        data: null
      }
      const mockResult: CommandResult<void> = {
        success: false,
        error: 'Unknown event type'
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffectError(
        SessionCommands.handleEvent(sessionId, event),
        (error) => {
          expect(error).toBeInstanceOf(SessionError)
          expect(error.operation).toBe('handle_event')
          expect(error.sessionId).toBe(sessionId)
        }
      )
    })

    it('should handle complex event data', async () => {
      const sessionId = 'session-123'
      const complexEvent = {
        type: 'tool_execution',
        data: {
          tool_id: 'tool-123',
          input: {
            nested: {
              array: [1, 2, 3],
              object: { key: 'value' }
            }
          },
          metadata: {
            timestamp: new Date().toISOString(),
            retries: 0,
            timeout: 30000
          }
        }
      }
      const mockResult: CommandResult<void> = {
        success: true
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.handleEvent(sessionId, complexEvent),
        () => {
          expect(invoke).toHaveBeenCalledWith('handle_session_event', {
            session_id: sessionId,
            event: complexEvent
          })
        }
      )
    })
  })

  describe('edge cases', () => {
    it('should handle sessions with large message history', async () => {
      const sessionId = 'session-large'
      const largeMessageCount = 1000
      const messages: Message[] = Array.from({ length: largeMessageCount }, (_, i) => ({
        id: `msg-${i}`,
        role: i % 2 === 0 ? 'user' : 'assistant',
        content: `Message ${i} with some content that makes it realistic`,
        timestamp: new Date(Date.now() - (largeMessageCount - i) * 60000).toISOString()
      }))

      const mockSession: Session = {
        id: sessionId,
        title: 'Large Session',
        project_path: '/home/user/large-project',
        created_at: new Date(Date.now() - 86400000).toISOString(), // 1 day ago
        is_active: true,
        messages
      }

      const mockResult: CommandResult<Session> = {
        success: true,
        data: mockSession
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.get(sessionId),
        (result) => {
          expect(Option.isSome(result)).toBe(true)
          if (Option.isSome(result)) {
            expect(result.value.messages).toHaveLength(largeMessageCount)
          }
        }
      )
    })

    it('should handle special characters in session operations', async () => {
      const specialChars = 'session-with-特殊字符-émojis-😀'
      const mockResult: CommandResult<void> = {
        success: true
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.stop(specialChars),
        () => {
          expect(invoke).toHaveBeenCalledWith('stop_session', {
            session_id: specialChars
          })
        }
      )
    })

    it('should handle very long messages', async () => {
      const sessionId = 'session-123'
      const longMessage = 'A'.repeat(10000) // 10KB message
      const mockResult: CommandResult<void> = {
        success: true
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.sendMessage(sessionId, longMessage),
        () => {
          expect(invoke).toHaveBeenCalledWith('send_message', {
            session_id: sessionId,
            message: longMessage
          })
        }
      )
    })

    it('should preserve message tool_uses structure', async () => {
      const sessionId = 'session-tools'
      const complexToolUse = {
        id: 'tool-complex',
        name: 'code_analyzer',
        input: {
          code: 'function test() { return 42; }',
          language: 'javascript',
          options: {
            includeMetrics: true,
            checkStyle: false,
            rules: ['no-unused-vars', 'semi']
          }
        }
      }

      const mockSession: Session = {
        id: sessionId,
        title: 'Tool Test Session',
        project_path: '/test',
        created_at: new Date().toISOString(),
        is_active: true,
        messages: [{
          id: 'msg-tool',
          role: 'assistant',
          content: 'Analyzing code...',
          timestamp: new Date().toISOString(),
          tool_uses: [complexToolUse]
        }]
      }

      const mockResult: CommandResult<Session> = {
        success: true,
        data: mockSession
      }
      vi.mocked(invoke).mockResolvedValue(mockResult)

      await expectEffect(
        SessionCommands.get(sessionId),
        (result) => {
          expect(Option.isSome(result)).toBe(true)
          if (Option.isSome(result)) {
            const toolUse = result.value.messages[0].tool_uses?.[0]
            expect(toolUse).toEqual(complexToolUse)
          }
        }
      )
    })
  })
})