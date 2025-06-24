/**
 * Tests for Convex schemas
 * @since 1.0.0
 */

import { describe, it, expect } from "vitest"
import * as Schema from "effect/Schema"
import { NostrEvent, AgentProfile, ChatSession, ChatMessage } from "../src/client.js"

describe("Convex Schemas", () => {
  describe("NostrEvent", () => {
    it("should validate a valid Nostr event", () => {
      const validEvent = {
        id: "abcd1234567890abcd1234567890abcd1234567890abcd1234567890abcd1234",
        pubkey: "efgh1234567890efgh1234567890efgh1234567890efgh1234567890efgh1234",
        created_at: 1640995200,
        kind: 1,
        tags: [["e", "event_id"], ["p", "pubkey"]],
        content: "Hello Nostr!",
        sig: "signature1234567890signature1234567890signature1234567890signature1234567890signature1234567890signature1234567890"
      }

      const result = Schema.decodeUnknownSync(NostrEvent)(validEvent)
      expect(result).toEqual(validEvent)
    })

    it("should fail validation for invalid event", () => {
      const invalidEvent = {
        id: "short",
        // missing required fields
      }

      expect(() => Schema.decodeUnknownSync(NostrEvent)(invalidEvent)).toThrow()
    })
  })

  describe("AgentProfile", () => {
    it("should validate a valid agent profile", () => {
      const validProfile = {
        pubkey: "agent1234567890agent1234567890agent1234567890agent1234567890",
        agent_id: "agent_001",
        name: "Test Agent",
        status: "active",
        balance: 50000,
        metabolic_rate: 100,
        capabilities: ["text_generation", "code_analysis"],
        last_activity: Date.now(),
        created_at: Date.now(),
        updated_at: Date.now()
      }

      const result = Schema.decodeUnknownSync(AgentProfile)(validProfile)
      expect(result.pubkey).toBe(validProfile.pubkey)
      expect(result.agent_id).toBe(validProfile.agent_id)
      expect(result.capabilities).toEqual(validProfile.capabilities)
    })
  })

  describe("ChatSession", () => {
    it("should validate a valid chat session", () => {
      const validSession = {
        id: "session_123",
        user_id: "user_456",
        project_path: "/path/to/project",
        project_name: "Test Project",
        status: "active",
        started_at: Date.now(),
        last_activity: Date.now(),
        message_count: 5,
        total_cost: 0.05
      }

      const result = Schema.decodeUnknownSync(ChatSession)(validSession)
      expect(result.id).toBe(validSession.id)
      expect(result.status).toBe("active")
    })
  })

  describe("ChatMessage", () => {
    it("should validate a user message", () => {
      const userMessage = {
        session_id: "session_123",
        entry_uuid: "msg_789",
        entry_type: "user",
        role: "user",
        content: "Hello there!",
        timestamp: Date.now()
      }

      const result = Schema.decodeUnknownSync(ChatMessage)(userMessage)
      expect(result.entry_type).toBe("user")
      expect(result.role).toBe("user")
    })

    it("should validate an assistant message with tool usage", () => {
      const assistantMessage = {
        session_id: "session_123",
        entry_uuid: "msg_790",
        entry_type: "assistant",
        role: "assistant",
        content: "I'll help you with that.",
        thinking: "The user is asking for help...",
        model: "claude-3-5-sonnet-20241022",
        token_usage: {
          input_tokens: 100,
          output_tokens: 50,
          total_tokens: 150
        },
        cost: 0.01,
        timestamp: Date.now()
      }

      const result = Schema.decodeUnknownSync(ChatMessage)(assistantMessage)
      expect(result.entry_type).toBe("assistant")
      expect(result.token_usage?.total_tokens).toBe(150)
    })

    it("should validate a tool use message", () => {
      const toolMessage = {
        session_id: "session_123",
        entry_uuid: "msg_791",
        entry_type: "tool_use",
        tool_name: "read_file",
        tool_input: { file_path: "/path/to/file.txt" },
        tool_use_id: "tool_123",
        timestamp: Date.now()
      }

      const result = Schema.decodeUnknownSync(ChatMessage)(toolMessage)
      expect(result.tool_name).toBe("read_file")
      expect(result.tool_use_id).toBe("tool_123")
    })
  })
})