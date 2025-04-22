import * as Effect from "effect/Effect"
import * as Either from "effect/Either"
import { 
  createGitHubApiClient, 
  GitHubApiClient, 
  FileNotFoundError, 
  GitHubApiError
} from "./github/GitHubApi.js"

// Example of using the GitHub API client
const program = Effect.gen(function*() {
  // First, get the GitHub client from the context
  const githubClient = yield* GitHubApiClient
  
  // Try to fetch a file that doesn't exist
  const fileResult = yield* Effect.either(
    githubClient.fetchFile({
      owner: "openagentsinc",
      repo: "openagents",
      path: "nonexistent.md"
    })
  )
  
  // Log the result of the fetch attempt
  if (Either.isLeft(fileResult)) {
    const error = fileResult.left
    
    if (error instanceof FileNotFoundError) {
      yield* Effect.log(`File not found error: ${error.message}`)
    } else if (error instanceof GitHubApiError) {
      yield* Effect.log(`GitHub API error: ${error.message}`)
    } else {
      yield* Effect.log(`Unknown error: ${String(error)}`)
    }
  } else {
    const file = fileResult.right
    yield* Effect.log(`File found: ${file.name}`)
    yield* Effect.log(`Content: ${Buffer.from(file.content, "base64").toString("utf-8")}`)
  }
  
  // Try to fetch a file that exists (in our mock implementation)
  const validFileResult = yield* Effect.either(
    githubClient.fetchFile({
      owner: "openagentsinc",
      repo: "openagents",
      path: "README.md"
    })
  )
  
  // Log the result of the successful fetch
  if (Either.isRight(validFileResult)) {
    const file = validFileResult.right
    yield* Effect.log(`File found: ${file.name}`)
    yield* Effect.log(`Content: ${Buffer.from(file.content, "base64").toString("utf-8")}`)
  }
})

// Create a layer with our GitHub client implementation
const MainLayer = createGitHubApiClient("https://api.github.com")

// Run the program with the layer
Effect.runPromise(
  program.pipe(
    Effect.provide(MainLayer)
  )
)