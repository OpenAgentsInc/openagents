import { Completions } from "@effect/ai"
import { AnthropicClient, AnthropicCompletions } from "@effect/ai-anthropic"
import { NodeHttpClient } from "@effect/platform-node"
import { Chunk, Config, Console, Effect, Layer, Option, Stream } from "effect"
import { startServer } from "./Server.js"
import { GitHubClientLayer } from "./github/GitHub.js"
import { GitHubTools, GitHubToolsLayer } from "./github/GitHubTools.js"

/**
 * Process a GitHub issue using AI and GitHub tools
 */
const processGitHubIssue = Effect.gen(function* () {
  yield* Console.log("🤖 Starting GitHub issue processing...")
  const completions = yield* Completions.Completions
  const { tools } = yield* GitHubTools
  yield* Console.log("🔧 Available GitHub tools:", Object.keys(tools.tools).join(", "))

  // Parameters from environment or command line
  const config = yield* Effect.try({
    try: () => {
      const owner = process.env.GITHUB_REPO_OWNER || "openagents"
      const repo = process.env.GITHUB_REPO_NAME || "openagents"
      const issueNumber = parseInt(process.env.GITHUB_ISSUE_NUMBER || "1", 10)
      
      return { owner, repo, issueNumber }
    },
    catch: (error) => new Error(`Failed to load configuration: ${error}`)
  })
  
  yield* Console.log(`Processing issue #${config.issueNumber} from ${config.owner}/${config.repo}`)

  // Setup the agent prompt
  const prompt = `You are an AI assistant tasked with analyzing and processing GitHub issues. 
You have access to GitHub API tools that allow you to retrieve issue details, create comments, and update issue status.

First, use the GetGitHubIssue tool to retrieve details about issue #${config.issueNumber} in the ${config.owner}/${config.repo} repository.
Then, analyze the issue and form a plan for addressing it. Create an agent state to track your progress using the CreateAgentStateForIssue tool.

Based on the issue content:
1. Summarize the issue's main points
2. Identify the type of issue (bug, feature request, question, etc.)
3. Determine what additional information might be needed
4. Outline potential approaches to solving the issue

IMPORTANT: Do not attempt to solve technical problems directly. Instead, focus on creating a clear plan and tracking system for addressing the issue.`

  // Stream the AI response with GitHub tools
  const streamResponse = completions.toolkitStream({
    input: prompt,
    tools,
    concurrency: 1
  })

  // Process each chunk as it arrives
  const processStream = streamResponse.pipe(
    Stream.tap((chunk) =>
      Effect.gen(function* () {
        // Extract the text content from the chunk's parts
        if (chunk.response && chunk.response.parts) {
          const parts = Chunk.toReadonlyArray(chunk.response.parts)
          for (const part of parts) {
            if (part._tag === "Text" && part.content) {
              yield* Console.log(`🔄 Delta: "${part.content}"`)
            } else if (part._tag === "ToolCall") {
              yield* Console.log(`🧰 Tool call: ${part.name} with params ${JSON.stringify(part.params)}`)
            }
          }
        }
        // Also log if we have a resolved value (happens with tool results)
        if (Option.isSome(chunk.value)) {
          yield* Console.log(`💡 Result: "${JSON.stringify(chunk.value.value).substring(0, 100)}..."`)
        }
      })
    ),
    Stream.runCollect
  )

  const chunks = yield* processStream
  const lastChunk = Chunk.last(chunks)

  if (Option.isSome(lastChunk)) {
    yield* Console.log("\n🎯 Final response:")
    yield* Console.log(Option.getOrElse(lastChunk.value.value, () => ""))
    return lastChunk.value
  }

  throw new Error("No response received")
})

// Setup Anthropic client configuration
const Claude3 = AnthropicCompletions.model("claude-3-5-sonnet-latest")

// Main function
const main = Effect.gen(function* () {
  const claude3 = yield* Claude3
  return yield* claude3.provide(processGitHubIssue)
})

// Anthropic configuration layer
const Anthropic = AnthropicClient.layerConfig({
  apiKey: Config.secret("ANTHROPIC_API_KEY")
})

// Combined layers
const AllLayers = Layer.mergeAll(
  Layer.provide(Anthropic, NodeHttpClient.layerUndici),
  GitHubClientLayer,
  GitHubToolsLayer
)

// Start the server when running the program directly
if (import.meta.url === `file://${process.argv[1]}`) {
  startServer()
} else {
  // If imported as a module, run the main function
  Effect.runPromise(
    main.pipe(
      Effect.catchAllDefect(() => Effect.succeed("Caught defect in main")),
      Effect.provide(AllLayers)
    )
  )
}