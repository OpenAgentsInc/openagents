import { Args, Command, Options } from "@effect/cli"
import * as Ai from "@openagentsinc/ai"
import { Console, Effect } from "effect"

// AI Commands for testing Claude Code integration
const promptArg = Args.text({ name: "prompt" }).pipe(
  Args.withDescription("The prompt to send to Claude")
)

const aiPrompt = Command.make("prompt", { prompt: promptArg }).pipe(
  Command.withDescription("Send a prompt to Claude Code"),
  Command.withHandler(({ prompt }) =>
    Effect.scoped(
      Effect.gen(function*() {
        yield* Console.log("🤖 Sending prompt to Claude Code...")

        const ai = yield* Ai.AiService.AiService
        const response = yield* ai.complete(prompt)

        yield* Console.log("\n📝 Response:")
        yield* Console.log(response.content)
        yield* Console.log(`\n📊 Model: ${response.model}`)

        if (response.usage.totalTokens > 0) {
          yield* Console.log(
            `📈 Tokens: ${response.usage.totalTokens} (input: ${response.usage.promptTokens}, output: ${response.usage.completionTokens})`
          )
        }

        if ("sessionId" in response && response.sessionId) {
          yield* Console.log(`🔗 Session ID: ${response.sessionId}`)
        }
      })
    ).pipe(
      Effect.provide(Ai.internal.ClaudeCodeProviderLive),
      Effect.catchAll((error) =>
        Effect.gen(function*() {
          yield* Console.error(`❌ Error: ${error}`)
          yield* Effect.fail(error)
        })
      )
    )
  )
)

const sessionIdOption = Options.text("session").pipe(
  Options.withDescription("Session ID to continue conversation"),
  Options.optional
)

const systemPromptOption = Options.text("system").pipe(
  Options.withDescription("System prompt to use"),
  Options.optional
)

const aiChat = Command.make("chat", { prompt: promptArg, session: sessionIdOption, system: systemPromptOption }).pipe(
  Command.withDescription("Have a conversation with Claude Code"),
  Command.withHandler(({ prompt, session, system }) =>
    Effect.scoped(
      Effect.gen(function*() {
        yield* Console.log("💬 Starting conversation with Claude Code...")

        const claudeClient = yield* Ai.internal.ClaudeCodeClient

        // Use session if provided, otherwise start new conversation
        const baseOptions = { outputFormat: "json" as const }
        const options = system._tag === "Some"
          ? { ...baseOptions, systemPrompt: system.value }
          : baseOptions

        const response = yield* (
          session._tag === "Some"
            ? claudeClient.continueSession(session.value, prompt, options)
            : claudeClient.prompt(prompt, options)
        )

        if ("content" in response) {
          yield* Console.log("\n📝 Response:")
          yield* Console.log(response.content)

          if ("session_id" in response && response.session_id) {
            yield* Console.log(`\n🔗 Session ID: ${response.session_id}`)
            yield* Console.log("💡 Use --session flag with this ID to continue the conversation")
          }
        }

        if ("model" in response) {
          yield* Console.log(`\n📊 Model: ${response.model}`)
        }

        if ("usage" in response && response.usage) {
          yield* Console.log(
            `📈 Tokens: ${response.usage.total_tokens} (input: ${response.usage.input_tokens}, output: ${response.usage.output_tokens})`
          )
        }

        if ("metadata" in response && response.metadata) {
          const meta = response.metadata
          if (meta.cost_usd) {
            yield* Console.log(`💰 Cost: $${meta.cost_usd.toFixed(6)} USD`)
          }
          if (meta.duration_ms) {
            yield* Console.log(`⏱️  Duration: ${meta.duration_ms}ms`)
          }
          if (meta.num_turns) {
            yield* Console.log(`🔄 Conversation turns: ${meta.num_turns}`)
          }
        }
      })
    ).pipe(
      Effect.provide(Ai.internal.ClaudeCodePtyClientLive),
      Effect.provide(Ai.internal.ClaudeCodeConfigDefault),
      Effect.catchAll((error) =>
        Effect.gen(function*() {
          yield* Console.error(`❌ Error: ${JSON.stringify(error, null, 2)}`)
          yield* Effect.fail(error)
        })
      )
    )
  )
)

const aiCheck = Command.make("check").pipe(
  Command.withDescription("Check if Claude Code CLI is available"),
  Command.withHandler(() =>
    Effect.scoped(
      Effect.gen(function*() {
        yield* Console.log("🔍 Checking Claude Code availability...")

        const claudeClient = yield* Ai.internal.ClaudeCodeClient
        const isAvailable = yield* claudeClient.checkAvailability()

        if (isAvailable) {
          yield* Console.log("✅ Claude Code CLI is available!")
          yield* Console.log("💡 You can now use 'ai prompt' and 'ai chat' commands")
        } else {
          yield* Console.log("❌ Claude Code CLI is not available")
          yield* Console.log("📝 Please ensure 'claude' is installed and in your PATH")
          yield* Console.log("🔗 Visit https://claude.ai/code for installation instructions")
        }
      })
    ).pipe(
      Effect.provide(Ai.internal.ClaudeCodePtyClientLive),
      Effect.provide(Ai.internal.ClaudeCodeConfigDefault),
      Effect.catchAll((error) =>
        Effect.gen(function*() {
          yield* Console.error(`❌ Error checking Claude Code: ${error}`)
          yield* Effect.fail(error)
        })
      )
    )
  )
)

const aiCommand = Command.make("ai").pipe(
  Command.withDescription("AI commands for testing Claude Code integration"),
  Command.withSubcommands([aiPrompt, aiChat, aiCheck])
)

const command = Command.make("openagents").pipe(
  Command.withSubcommands([aiCommand])
)

export const cli = Command.run(command, {
  name: "OpenAgents CLI",
  version: "0.0.0"
})
