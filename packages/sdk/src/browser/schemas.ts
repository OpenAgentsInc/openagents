/**
 * Schemas for Autonomous Chat Agent - extracted for testability
 */

import { Schema } from "effect"

// Agent personality and configuration
export const AgentPersonality = Schema.Struct({
  name: Schema.String,
  role: Schema.Union(
    Schema.Literal("teacher"),
    Schema.Literal("analyst"),
    Schema.Literal("student"),
    Schema.Literal("entrepreneur"),
    Schema.Literal("artist"),
    Schema.Literal("skeptic"),
    Schema.Literal("helper"),
    Schema.Literal("comedian")
  ),
  traits: Schema.Array(Schema.String), // ["helpful", "patient", "analytical"]
  responseStyle: Schema.Union(
    Schema.Literal("formal"),
    Schema.Literal("casual"),
    Schema.Literal("enthusiastic"),
    Schema.Literal("analytical"),
    Schema.Literal("humorous"),
    Schema.Literal("concise")
  ),
  topics: Schema.Array(Schema.String), // ["technology", "business", "art"]
  chattiness: Schema.Number.pipe(Schema.between(0, 1)), // 0 = rarely responds, 1 = very chatty
  temperature: Schema.Number.pipe(Schema.between(0, 1)) // AI temperature for responses
})
export type AgentPersonality = Schema.Schema.Type<typeof AgentPersonality>

// Chat decision context
export const ChatDecisionContext = Schema.Struct({
  recentMessages: Schema.Array(Schema.Struct({
    content: Schema.String,
    author: Schema.String,
    timestamp: Schema.Number
  })),
  channelTopic: Schema.optional(Schema.String),
  agentLastResponse: Schema.optional(Schema.Number), // timestamp of last response
  messagesSinceLastResponse: Schema.Number
})
export type ChatDecisionContext = Schema.Schema.Type<typeof ChatDecisionContext>

// AI decision response
export const ChatDecision = Schema.Struct({
  shouldRespond: Schema.Boolean,
  response: Schema.optional(Schema.String),
  reasoning: Schema.String,
  confidence: Schema.Number.pipe(Schema.between(0, 1))
})
export type ChatDecision = Schema.Schema.Type<typeof ChatDecision>
