/**
 * Tests for Autonomous Chat Agent schemas
 * Tests core validation logic without AI dependencies
 */

import { Effect, Schema } from "effect"
import { describe, expect, it } from "vitest"
import { AgentPersonality, ChatDecision, ChatDecisionContext } from "../src/browser/schemas.js"

describe("Autonomous Chat Agent Schemas", () => {
  describe("AgentPersonality Schema", () => {
    it("should validate a complete valid personality", () => {
      const validPersonality = {
        name: "TestBot",
        role: "teacher" as const,
        traits: ["helpful", "patient", "knowledgeable"],
        responseStyle: "formal" as const,
        topics: ["technology", "education"],
        chattiness: 0.7,
        temperature: 0.8
      }

      const decoded = Schema.decodeUnknown(AgentPersonality)(validPersonality)
      const result = Effect.runSync(Effect.either(decoded))

      expect(result._tag).toBe("Right")
      if (result._tag === "Right") {
        expect(result.right).toEqual(validPersonality)
      }
    })

    it("should validate all supported roles", () => {
      const roles = [
        "teacher",
        "analyst",
        "student",
        "entrepreneur",
        "artist",
        "skeptic",
        "helper",
        "comedian"
      ] as const

      roles.forEach((role) => {
        const personality = {
          name: "TestBot",
          role,
          traits: ["test"],
          responseStyle: "casual" as const,
          topics: ["general"],
          chattiness: 0.5,
          temperature: 0.7
        }

        const decoded = Schema.decodeUnknown(AgentPersonality)(personality)
        const result = Effect.runSync(Effect.either(decoded))

        expect(result._tag).toBe("Right")
      })
    })

    it("should validate all supported response styles", () => {
      const styles = ["formal", "casual", "enthusiastic", "analytical", "humorous", "concise"] as const

      styles.forEach((responseStyle) => {
        const personality = {
          name: "TestBot",
          role: "helper" as const,
          traits: ["test"],
          responseStyle,
          topics: ["general"],
          chattiness: 0.5,
          temperature: 0.7
        }

        const decoded = Schema.decodeUnknown(AgentPersonality)(personality)
        const result = Effect.runSync(Effect.either(decoded))

        expect(result._tag).toBe("Right")
      })
    })

    it("should reject invalid role", () => {
      const invalidPersonality = {
        name: "TestBot",
        role: "invalid-role",
        traits: ["helpful"],
        responseStyle: "formal" as const,
        topics: ["technology"],
        chattiness: 0.7,
        temperature: 0.8
      }

      const decoded = Schema.decodeUnknown(AgentPersonality)(invalidPersonality)
      const result = Effect.runSync(Effect.either(decoded))

      expect(result._tag).toBe("Left")
    })

    it("should reject chattiness values outside 0-1 range", () => {
      const invalidPersonalities = [
        {
          name: "TestBot",
          role: "teacher" as const,
          traits: ["helpful"],
          responseStyle: "formal" as const,
          topics: ["technology"],
          chattiness: -0.1,
          temperature: 0.8
        },
        {
          name: "TestBot",
          role: "teacher" as const,
          traits: ["helpful"],
          responseStyle: "formal" as const,
          topics: ["technology"],
          chattiness: 1.1,
          temperature: 0.8
        }
      ]

      invalidPersonalities.forEach((personality) => {
        const decoded = Schema.decodeUnknown(AgentPersonality)(personality)
        const result = Effect.runSync(Effect.either(decoded))

        expect(result._tag).toBe("Left")
      })
    })

    it("should reject temperature values outside 0-1 range", () => {
      const invalidPersonalities = [
        {
          name: "TestBot",
          role: "teacher" as const,
          traits: ["helpful"],
          responseStyle: "formal" as const,
          topics: ["technology"],
          chattiness: 0.7,
          temperature: -0.1
        },
        {
          name: "TestBot",
          role: "teacher" as const,
          traits: ["helpful"],
          responseStyle: "formal" as const,
          topics: ["technology"],
          chattiness: 0.7,
          temperature: 1.1
        }
      ]

      invalidPersonalities.forEach((personality) => {
        const decoded = Schema.decodeUnknown(AgentPersonality)(personality)
        const result = Effect.runSync(Effect.either(decoded))

        expect(result._tag).toBe("Left")
      })
    })
  })

  describe("ChatDecisionContext Schema", () => {
    it("should validate valid decision context", () => {
      const validContext = {
        recentMessages: [
          {
            content: "Hello, how are you?",
            author: "user123",
            timestamp: Date.now()
          }
        ],
        channelTopic: "General Discussion",
        agentLastResponse: Date.now() - 5000,
        messagesSinceLastResponse: 3
      }

      const decoded = Schema.decodeUnknown(ChatDecisionContext)(validContext)
      const result = Effect.runSync(Effect.either(decoded))

      expect(result._tag).toBe("Right")
      if (result._tag === "Right") {
        expect(result.right.recentMessages).toHaveLength(1)
        expect(result.right.messagesSinceLastResponse).toBe(3)
      }
    })

    it("should handle optional fields", () => {
      const minimalContext = {
        recentMessages: [],
        messagesSinceLastResponse: 0
      }

      const decoded = Schema.decodeUnknown(ChatDecisionContext)(minimalContext)
      const result = Effect.runSync(Effect.either(decoded))

      expect(result._tag).toBe("Right")
      if (result._tag === "Right") {
        expect(result.right.channelTopic).toBeUndefined()
        expect(result.right.agentLastResponse).toBeUndefined()
      }
    })
  })

  describe("ChatDecision Schema", () => {
    it("should validate valid decision", () => {
      const validDecision = {
        shouldRespond: true,
        response: "I think that's a great point!",
        reasoning: "The user asked a thoughtful question",
        confidence: 0.85
      }

      const decoded = Schema.decodeUnknown(ChatDecision)(validDecision)
      const result = Effect.runSync(Effect.either(decoded))

      expect(result._tag).toBe("Right")
      if (result._tag === "Right") {
        expect(result.right.shouldRespond).toBe(true)
        expect(result.right.confidence).toBe(0.85)
      }
    })

    it("should handle optional response field", () => {
      const decisionWithoutResponse = {
        shouldRespond: false,
        reasoning: "Not relevant to my expertise",
        confidence: 0.6
      }

      const decoded = Schema.decodeUnknown(ChatDecision)(decisionWithoutResponse)
      const result = Effect.runSync(Effect.either(decoded))

      expect(result._tag).toBe("Right")
      if (result._tag === "Right") {
        expect(result.right.response).toBeUndefined()
      }
    })
  })

  describe("Personality Validation Edge Cases", () => {
    it("should handle boundary values correctly", () => {
      const boundaryPersonality = {
        name: "BoundaryBot",
        role: "teacher" as const,
        traits: [],
        responseStyle: "formal" as const,
        topics: [],
        chattiness: 0.0, // Minimum
        temperature: 1.0 // Maximum
      }

      const decoded = Schema.decodeUnknown(AgentPersonality)(boundaryPersonality)
      const result = Effect.runSync(Effect.either(decoded))

      expect(result._tag).toBe("Right")
      if (result._tag === "Right") {
        expect(result.right.chattiness).toBe(0.0)
        expect(result.right.temperature).toBe(1.0)
      }
    })

    it("should validate complex personality combinations", () => {
      const complexPersonalities = [
        {
          name: "TechGuru",
          role: "teacher" as const,
          traits: ["patient", "knowledgeable", "encouraging", "detail-oriented"],
          responseStyle: "formal" as const,
          topics: ["programming", "software-engineering", "best-practices", "mentoring"],
          chattiness: 0.8,
          temperature: 0.6
        },
        {
          name: "MarketAnalyst",
          role: "analyst" as const,
          traits: ["analytical", "data-driven", "precise", "skeptical"],
          responseStyle: "analytical" as const,
          topics: ["finance", "markets", "economics", "trading", "risk-management"],
          chattiness: 0.4,
          temperature: 0.3
        },
        {
          name: "StartupFounder",
          role: "entrepreneur" as const,
          traits: ["ambitious", "innovative", "resilient", "visionary"],
          responseStyle: "enthusiastic" as const,
          topics: ["startups", "innovation", "funding", "scaling", "product-market-fit"],
          chattiness: 0.9,
          temperature: 0.8
        }
      ]

      complexPersonalities.forEach((personality) => {
        const decoded = Schema.decodeUnknown(AgentPersonality)(personality)
        const result = Effect.runSync(Effect.either(decoded))

        expect(result._tag).toBe("Right")
        if (result._tag === "Right") {
          expect(result.right.name).toBe(personality.name)
          expect(result.right.traits.length).toBeGreaterThan(0)
          expect(result.right.topics.length).toBeGreaterThan(0)
        }
      })
    })
  })
})
