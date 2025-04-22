import * as Effect from "effect/Effect"
import * as Either from "effect/Either"
import { 
  GitHubApiClient, 
  FileNotFoundError, 
  GitHubApiError,
  createGitHubApiClient
} from "./github/GitHubApi.js"

// Example of using the GitHub API client
const program = Effect.gen(function*() {
  console.log("Starting GitHub API test...")
  
  // First, get the GitHub client from the context
  const githubClient = yield* GitHubApiClient
  
  // Try to fetch a file that doesn't exist
  const nonExistentFile = yield* Effect.either(
    githubClient.fetchFile({
      owner: "openagentsinc",
      repo: "openagents",
      path: "nonexistent-file-test-123.md"
    })
  )
  
  // Log the result of the fetch attempt
  if (Either.isLeft(nonExistentFile)) {
    const error = nonExistentFile.left
    
    if (error instanceof FileNotFoundError) {
      yield* Effect.log(`File not found error: ${error.message}`)
    } else if (error instanceof GitHubApiError) {
      yield* Effect.log(`GitHub API error: ${error.message}`)
    } else {
      yield* Effect.log(`Unknown error: ${String(error)}`)
    }
  } else {
    const file = nonExistentFile.right
    yield* Effect.log(`File found: ${file.name}`)
    yield* Effect.log(`Content: ${Buffer.from(file.content, "base64").toString("utf-8")}`)
  }
  
  // Try to fetch a file that exists (README.md on the openagents repo)
  yield* Effect.log("Now attempting to fetch a real file that exists...")
  
  const validFileResult = yield* Effect.either(
    githubClient.fetchFile({
      owner: "openagentsinc",
      repo: "openagents",
      path: "README.md"
    })
  )
  
  // Log the result of the successful fetch
  if (Either.isLeft(validFileResult)) {
    const error = validFileResult.left
    yield* Effect.log(`Error fetching real file: ${error.message}`)
  } else if (Either.isRight(validFileResult)) {
    const file = validFileResult.right
    yield* Effect.log(`File found: ${file.name}`)
    yield* Effect.log(`Content (first 200 chars): ${Buffer.from(file.content, "base64").toString("utf-8").substring(0, 200)}...`)
  }
})

// Create a layer with our real HTTP client implementation
// Add your GitHub token here if you have one, to avoid rate limits
const MainLayer = createGitHubApiClient("https://api.github.com")

// Run the program with the layer
Effect.runPromise(
  program.pipe(
    Effect.provide(MainLayer)
  )
)