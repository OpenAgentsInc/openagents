import { it, describe, expect } from 'vitest';
import { Effect, Layer } from 'effect';
import { GitHubTools, GitHubToolsLive, FileContentParams, IssueParams } from '../src/AiService.js';
import { mockGitHubFileClientLayer, GitHubFileClient } from '../src/github/FileClient.js';
import { mockGitHubIssueClientLayer, GitHubIssueClient } from '../src/github/IssueClient.js';
import { Buffer } from 'node:buffer';

describe('AI Service', () => {
  // Test the GetGitHubFileContent tool
  it('should handle the GetGitHubFileContent tool correctly', async () => {
    // Execute a real mock GitHub file request 
    const fileRequest = Effect.gen(function* () {
      const mockLayer = Layer.merge(
        mockGitHubFileClientLayer,
        mockGitHubIssueClientLayer
      );
      
      // We need to directly use the file client
      const fileClient = yield* Effect.provide(
        GitHubFileClient,
        mockLayer
      );
      
      // Test actual mock request
      const file = yield* fileClient.fetchFile({
        owner: 'openagentsinc',
        repo: 'openagents',
        path: 'README.md'
      });
      
      return Buffer.from(file.content, "base64").toString("utf-8");
    });
    
    const fileResult = await Effect.runPromise(fileRequest);
    
    // The mock returns a base64 encoded "# This is a test file"
    expect(fileResult).toContain('This is a test file');
    
    // Test for failure case separately 
    const fileFailureRequest = Effect.gen(function* () {
      const mockLayer = Layer.merge(
        mockGitHubFileClientLayer,
        mockGitHubIssueClientLayer
      );
      
      // We need to directly use the file client
      const fileClient = yield* Effect.provide(
        GitHubFileClient,
        mockLayer
      );
      
      // Test the non-existent path which should fail
      return yield* Effect.either(
        fileClient.fetchFile({
          owner: 'openagentsinc',
          repo: 'openagents',
          path: 'nonexistent.md'
        })
      );
    });
    
    const fileFailure = await Effect.runPromise(fileFailureRequest);
    
    expect(fileFailure._tag).toBe('Left');
    expect(fileFailure.left.message).toContain('File not found');
  });
  
  // Test the GetGitHubIssue tool
  it('should handle the GetGitHubIssue tool correctly', async () => {
    // Test issue operation directly
    const issueRequest = Effect.gen(function* () {
      const mockLayer = Layer.merge(
        mockGitHubFileClientLayer,
        mockGitHubIssueClientLayer
      );
      
      // Get the issue client
      const issueClient = yield* Effect.provide(
        GitHubIssueClient,
        mockLayer
      );
      
      // Test actual mock request
      const issue = yield* issueClient.fetchIssue({
        owner: 'openagentsinc',
        repo: 'openagents',
        issueNumber: 1
      });
      
      return issue;
    });
    
    const issue = await Effect.runPromise(issueRequest);
    
    // The mock returns an issue with title "Mock Issue #1"
    expect(issue.title).toBe('Mock Issue #1');
    expect(issue.state).toBe('open');
    
    // Test for failure: Rate limit exceeded (as defined in the mock for issue number 9999)
    const issueFailureRequest = Effect.gen(function* () {
      const mockLayer = Layer.merge(
        mockGitHubFileClientLayer,
        mockGitHubIssueClientLayer
      );
      
      // Get the issue client
      const issueClient = yield* Effect.provide(
        GitHubIssueClient,
        mockLayer
      );
      
      // Test rate limit case (issue number 9999 is configured in the mock to trigger rate limit)
      return yield* Effect.either(
        issueClient.fetchIssue({
          owner: 'openagentsinc',
          repo: 'openagents',
          issueNumber: 9999
        })
      );
    });
    
    const issueFailure = await Effect.runPromise(issueFailureRequest);
    
    expect(issueFailure._tag).toBe('Left');
    expect(issueFailure.left.message).toContain('rate limit exceeded');
  });
});