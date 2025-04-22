import * as Effect from "effect/Effect"
import * as Either from "effect/Either"
import * as Layer from "effect/Layer"

import { 
  GitHubFileClient, 
  FileNotFoundError
} from "./github/FileClient.js"

import {
  GitHubIssueClient,
  IssueNotFoundError
} from "./github/IssueClient.js"

import {
  GitHubApiError
} from "./github/Errors.js"

import {
  defaultGitHubLayer,
  createGitHubConfig
} from "./github/Client.js"

import {
  githubFileClientLayer
} from "./github/FileClient.js"

import {
  githubIssueClientLayer
} from "./github/IssueClient.js"

// Example of using the GitHub API clients
const program = Effect.gen(function*() {
  console.log("Starting GitHub API test...")
  
  // PART 1: Testing GitHub File API
  console.log("\n--- Testing GitHub File API ---")
  const githubClient = yield* GitHubFileClient
  
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
  
  // PART 2: Testing GitHub Issue API
  console.log("\n--- Testing GitHub Issue API ---")
  const issueClient = yield* GitHubIssueClient
  
  // Try to fetch an issue that doesn't exist
  const nonExistentIssue = yield* Effect.either(
    issueClient.fetchIssue({
      owner: "openagentsinc",
      repo: "openagents",
      issueNumber: 999999
    })
  )
  
  // Log the result of the fetch attempt
  if (Either.isLeft(nonExistentIssue)) {
    const error = nonExistentIssue.left
    
    if (error instanceof IssueNotFoundError) {
      yield* Effect.log(`Issue not found error: ${error.message}`)
    } else if (error instanceof GitHubApiError) {
      yield* Effect.log(`GitHub API error: ${error.message}`)
    } else {
      yield* Effect.log(`Unknown error: ${String(error)}`)
    }
  } else {
    const issue = nonExistentIssue.right
    yield* Effect.log(`Issue found: #${issue.number} - ${issue.title}`)
    yield* Effect.log(`Body (first 100 chars): ${issue.body.substring(0, 100)}...`)
  }
  
  // Try to fetch an issue that exists
  yield* Effect.log("Now attempting to fetch a real issue that exists...")
  
  // Change this to a valid issue number in the openagents repo
  // or leave as is to see the not found error
  const ISSUE_NUMBER = 1
  
  const validIssueResult = yield* Effect.either(
    issueClient.fetchIssue({
      owner: "openagentsinc",
      repo: "openagents",
      issueNumber: ISSUE_NUMBER
    })
  )
  
  // Log the result
  if (Either.isLeft(validIssueResult)) {
    const error = validIssueResult.left
    yield* Effect.log(`Error fetching real issue: ${error.message}`)
  } else {
    const issue = validIssueResult.right
    yield* Effect.log(`Issue found: #${issue.number} - ${issue.title}`)
    yield* Effect.log(`State: ${issue.state}`)
    yield* Effect.log(`Created by: ${issue.user.login}`)
    yield* Effect.log(`Labels: ${issue.labels.map(l => l.name).join(", ")}`)
    yield* Effect.log(`Body (first 100 chars): ${issue.body.substring(0, 100)}...`)
  }
})

// Add your GitHub token here if you have one to avoid rate limits
const token = undefined // "your_github_token_here"

// Create the base GitHub layer with config
const baseGitHubLayer = Layer.provide(
  defaultGitHubLayer,
  createGitHubConfig("https://api.github.com", token)
)

// Create the specific client layers
const clientLayers = Layer.merge(
  githubFileClientLayer,
  githubIssueClientLayer
)

// Combine the layers
const MainLayer = Layer.provide(
  clientLayers,
  baseGitHubLayer
)

// Run the program with the combined layer
const runProgramWithSafeExit = (): void => {
  Effect.runPromise(
    program.pipe(
      Effect.provide(MainLayer),
      Effect.tapErrorCause((cause) => Effect.sync(() => {
        console.error("Error occurred:", cause)
      }))
    )
  ).catch(err => {
    console.error("Unhandled error in runtime:", err)
  })
}

runProgramWithSafeExit()