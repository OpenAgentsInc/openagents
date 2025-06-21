/**
 * Autonomous Chat Agent - AI-powered agents that chat in channels
 * Uses Cloudflare inference for decision-making and response generation
 */

import { Context, Data, Effect, Fiber, Layer, Schema, Stream } from "effect"
import type { ChannelMessage } from "./ChannelService.js"
import { ChannelService } from "./ChannelService.js"

import * as AI from "@openagentsinc/ai"

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

// Errors
export class AutonomousChatError extends Data.TaggedError("AutonomousChatError")<{
  reason: "ai_failed" | "channel_error" | "decision_failed" | "response_failed"
  message: string
  cause?: unknown
}> {}

// Autonomous Chat Agent Service
export class AutonomousChatAgent extends Context.Tag("sdk/AutonomousChatAgent")<
  AutonomousChatAgent,
  {
    readonly startChatLoop: (
      channelId: string,
      personality: AgentPersonality,
      agentKeys: { privateKey: string; publicKey: string }
    ) => Effect.Effect<void, AutonomousChatError>

    readonly stopChatLoop: (channelId: string) => Effect.Effect<void, never>

    readonly makeDecision: (
      context: ChatDecisionContext,
      personality: AgentPersonality
    ) => Effect.Effect<ChatDecision, AutonomousChatError>

    readonly generateResponse: (
      messages: Array<ChannelMessage>,
      personality: AgentPersonality
    ) => Effect.Effect<string, AutonomousChatError>
  }
>() {}

// Cloudflare AI integration for decision making
const makeAIDecision = (
  context: ChatDecisionContext,
  personality: AgentPersonality,
  languageModel: AI.AiLanguageModel.AiLanguageModel.Service<never>
): Effect.Effect<ChatDecision, AutonomousChatError, never> =>
  Effect.gen(function*() {
    const recentMessagesText = context.recentMessages
      .map((m) => `${m.author}: ${m.content}`)
      .join("\n")

    const prompt = `You are an autonomous chat agent named "${personality.name}" with the following personality:
- Role: ${personality.role}
- Communication Style: ${personality.responseStyle}
- Interests: ${personality.topics.join(", ")}
- Traits: ${personality.traits.join(", ")}
- Chattiness Level: ${personality.chattiness} (0=rarely responds, 1=very chatty)

Context:
- Recent messages:
${recentMessagesText}
- Messages since your last response: ${context.messagesSinceLastResponse}
- Time since your last response: ${
      context.agentLastResponse
        ? `${Math.floor((Date.now() - context.agentLastResponse) / 1000)} seconds`
        : "Never responded"
    }

Based on your personality and the conversation context, should you respond to the recent messages?

Respond with a JSON object in this exact format:
{
  "shouldRespond": true/false,
  "reasoning": "Brief explanation of your decision",
  "confidence": 0.0-1.0
}`

    const response = yield* languageModel.generateText({
      prompt: AI.AiPrompt.make(prompt)
    }).pipe(
      Effect.mapError((error) =>
        new AutonomousChatError({
          reason: "ai_failed",
          message: `AI service error: ${error}`
        })
      )
    )

    // Extract and parse the JSON response
    const content = response.text
    const jsonMatch = content.match(/\{[\s\S]*\}/)

    if (!jsonMatch) {
      return yield* Effect.fail(
        new AutonomousChatError({
          reason: "ai_failed",
          message: "No valid JSON found in AI response"
        })
      )
    }

    try {
      const decision = JSON.parse(jsonMatch[0])
      return {
        shouldRespond: Boolean(decision.shouldRespond),
        reasoning: String(decision.reasoning || "AI decision"),
        confidence: Math.max(0, Math.min(1, Number(decision.confidence) || 0.5)),
        response: undefined
      }
    } catch (error) {
      return yield* Effect.fail(
        new AutonomousChatError({
          reason: "ai_failed",
          message: `Failed to parse AI decision: ${error}`
        })
      )
    }
  })

// Cloudflare AI integration for response generation
const generateAIResponse = (
  messages: Array<ChannelMessage>,
  personality: AgentPersonality,
  languageModel: AI.AiLanguageModel.AiLanguageModel.Service<never>
): Effect.Effect<string, AutonomousChatError, never> =>
  Effect.gen(function*() {
    const conversationHistory = messages
      .slice(-5) // Last 5 messages for context
      .map((m) => `${m.pubkey.slice(0, 8)}: ${m.content}`)
      .join("\n")

    const prompt = `You are an autonomous chat agent named "${personality.name}" with the following personality:
- Role: ${personality.role}
- Communication Style: ${personality.responseStyle}
- Interests: ${personality.topics.join(", ")}
- Traits: ${personality.traits.join(", ")}

Recent conversation:
${conversationHistory}

Generate a response that matches your personality and role. Keep it natural and conversational.
Be ${personality.responseStyle} in your communication style.
As a ${personality.role}, respond appropriately to the conversation.

Your response:`

    const response = yield* languageModel.generateText({
      prompt: AI.AiPrompt.make(prompt)
    }).pipe(
      Effect.mapError((error) =>
        new AutonomousChatError({
          reason: "ai_failed",
          message: `AI service error: ${error}`
        })
      )
    )

    const content = response.text
    const cleanResponse = content
      .replace(/^Your response:\s*/i, "")
      .replace(/^Response:\s*/i, "")
      .trim()

    if (!cleanResponse) {
      return yield* Effect.fail(
        new AutonomousChatError({
          reason: "ai_failed",
          message: "AI generated empty response"
        })
      )
    }

    return cleanResponse
  })

// Implementation
export const AutonomousChatAgentLive = Layer.effect(
  AutonomousChatAgent,
  Effect.gen(function*() {
    const channelService = yield* ChannelService
    const languageModel = yield* AI.AiLanguageModel.AiLanguageModel

    // Active chat loops
    const activeChatLoops = new Map<string, { stop: () => void }>()

    const makeDecision = (
      context: ChatDecisionContext,
      personality: AgentPersonality
    ): Effect.Effect<ChatDecision, AutonomousChatError> =>
      Effect.gen(function*() {
        // Use real Cloudflare AI for decision making
        const aiDecision = yield* makeAIDecision(context, personality, languageModel).pipe(
          Effect.catchAll((error: AutonomousChatError) =>
            Effect.succeed({
              shouldRespond: Math.random() < personality.chattiness,
              reasoning: `Rule-based fallback: ${error.message}`,
              confidence: 0.3,
              response: undefined
            } as ChatDecision)
          )
        )

        return aiDecision
      })

    const generateResponse = (
      messages: Array<ChannelMessage>,
      personality: AgentPersonality
    ): Effect.Effect<string, AutonomousChatError> =>
      Effect.gen(function*() {
        // Use real Cloudflare AI for response generation
        const aiResponse = yield* generateAIResponse(messages, personality, languageModel).pipe(
          Effect.catchAll((error: AutonomousChatError) => {
            console.error("AI response generation failed, using template fallback:", error)

            // Template fallback only if AI fails
            const responses = {
              teacher: [
                "That's an interesting point. Let me add some context...",
                "Great question! Here's how I think about it...",
                "I'd like to build on that idea..."
              ],
              analyst: [
                "Looking at this analytically...",
                "The data suggests...",
                "We should consider the implications of..."
              ],
              student: [
                "I'm curious about...",
                "Can someone explain...?",
                "That makes me wonder..."
              ],
              entrepreneur: [
                "There's an opportunity here...",
                "From a business perspective...",
                "How can we scale this...?"
              ],
              artist: [
                "This reminds me of...",
                "There's beauty in this concept...",
                "Creatively speaking..."
              ],
              skeptic: [
                "I have some doubts about...",
                "Where's the evidence for...?",
                "Playing devil's advocate..."
              ],
              helper: [
                "I can help with that...",
                "Have you tried...?",
                "Let me offer a suggestion..."
              ],
              comedian: [
                "This is like when...",
                "Ha! That reminds me...",
                "On a lighter note..."
              ]
            }

            const roleResponses = responses[personality.role]
            const fallbackResponse = roleResponses[Math.floor(Math.random() * roleResponses.length)]

            return Effect.succeed(`${fallbackResponse} (template fallback)`)
          })
        )

        return aiResponse
      })

    const startChatLoop = (
      channelId: string,
      personality: AgentPersonality,
      agentKeys: { privateKey: string; publicKey: string }
    ): Effect.Effect<void, AutonomousChatError> =>
      Effect.gen(function*() {
        console.log(`Starting autonomous chat loop for ${personality.name} in channel ${channelId}`)

        let lastResponseTime = 0
        let messageCount = 0

        // Subscribe to channel messages
        const messageStream = channelService.messages(channelId)

        const chatLoop = Effect.gen(function*() {
          yield* messageStream.pipe(
            Stream.tap((message) =>
              Effect.gen(function*() {
                messageCount++

                // Don't respond to own messages
                if (message.pubkey === agentKeys.publicKey) {
                  return
                }

                console.log(`${personality.name} sees message: ${message.content}`)

                // Build decision context
                const context: ChatDecisionContext = {
                  recentMessages: [{
                    content: message.content,
                    author: message.pubkey.slice(0, 8),
                    timestamp: message.created_at
                  }],
                  messagesSinceLastResponse: messageCount,
                  agentLastResponse: lastResponseTime || undefined
                }

                // Make decision
                const decision = yield* makeDecision(context, personality)
                console.log(`${personality.name} decision:`, decision)

                if (decision.shouldRespond) {
                  // Generate response
                  const response = yield* generateResponse([message], personality)

                  console.log(`${personality.name} responding: ${response}`)

                  // Send message
                  yield* channelService.sendMessage({
                    channelId,
                    content: response
                  })

                  lastResponseTime = Date.now()
                  messageCount = 0
                }
              }).pipe(
                Effect.catchAll((error) => Effect.log(`Chat loop error for ${personality.name}: ${error}`))
              )
            ),
            Stream.runDrain
          )
        })

        // Start the chat loop in background
        const fiber = yield* Effect.fork(chatLoop)

        // Store the loop so it can be stopped
        activeChatLoops.set(channelId, {
          stop: () => Effect.runSync(Fiber.interrupt(fiber))
        })
      })

    const stopChatLoop = (channelId: string): Effect.Effect<void, never> =>
      Effect.sync(() => {
        const loop = activeChatLoops.get(channelId)
        if (loop) {
          loop.stop()
          activeChatLoops.delete(channelId)
          console.log(`Stopped chat loop for channel ${channelId}`)
        }
      })

    return {
      startChatLoop,
      stopChatLoop,
      makeDecision,
      generateResponse
    }
  })
)
