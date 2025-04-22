import { it, describe, expect, vi, afterEach } from 'vitest';
import { Effect, Layer } from 'effect';
import { GitHubTools } from '../../src/AiService.js';

describe('Tool Execution Integration', () => {
  // Mock console.log to capture output
  const consoleLogMock = vi.spyOn(console, 'log').mockImplementation(() => {});
  
  // Reset mocks after each test
  afterEach(() => {
    consoleLogMock.mockReset();
  });
  
  // Create a mock implementation of GitHubTools
  const MockGitHubToolsLayer = Layer.succeed(
    GitHubTools,
    {
      getFileContent: ({ owner, repo, path }) => 
        Effect.succeed(`Mock file content for ${owner}/${repo}/${path}`),
      getIssue: ({ owner, repo, issueNumber }) => 
        Effect.succeed(`Mock issue #${issueNumber} from ${owner}/${repo}`)
    }
  );
  
  it('should correctly execute the GitHub file content tool', async () => {
    // Create a program that simulates executing the tool
    const program = Effect.gen(function* () {
      const githubTools = yield* GitHubTools;
      
      // Execute the GetGitHubFileContent tool with test parameters
      const result = yield* githubTools.getFileContent({
        owner: 'openagentsinc',
        repo: 'openagents',
        path: 'README.md'
      });
      
      return result;
    });
    
    // Run the program with our mock layer
    const result = await Effect.runPromise(
      program.pipe(Effect.provide(MockGitHubToolsLayer))
    );
    
    // Verify the tool execution returned the expected result
    expect(result).toBe('Mock file content for openagentsinc/openagents/README.md');
  });
  
  it('should correctly execute the GitHub issue tool', async () => {
    // Create a program that simulates executing the tool
    const program = Effect.gen(function* () {
      const githubTools = yield* GitHubTools;
      
      // Execute the GetGitHubIssue tool with test parameters
      const result = yield* githubTools.getIssue({
        owner: 'openagentsinc',
        repo: 'openagents',
        issueNumber: 123
      });
      
      return result;
    });
    
    // Run the program with our mock layer
    const result = await Effect.runPromise(
      program.pipe(Effect.provide(MockGitHubToolsLayer))
    );
    
    // Verify the tool execution returned the expected result
    expect(result).toBe('Mock issue #123 from openagentsinc/openagents');
  });
  
  // Create a failing mock implementation for error tests
  const FailingMockGitHubToolsLayer = Layer.succeed(
    GitHubTools,
    {
      getFileContent: () => Effect.fail('File not found: mock error'),
      getIssue: () => Effect.fail('Issue not found: mock error')
    }
  );
  
  it('should handle errors in tool execution', async () => {
    // Create a program that simulates executing the tool with an error
    const program = Effect.gen(function* () {
      const githubTools = yield* GitHubTools;
      
      // Execute the GetGitHubFileContent tool that will fail
      return yield* Effect.either(
        githubTools.getFileContent({
          owner: 'invalid',
          repo: 'invalid',
          path: 'invalid.md'
        })
      );
    });
    
    // Run the program with our failing mock layer
    const result = await Effect.runPromise(
      program.pipe(Effect.provide(FailingMockGitHubToolsLayer))
    );
    
    // Verify the error was correctly captured
    expect(result._tag).toBe('Left');
    if (result._tag === 'Left') {
      expect(result.left).toBe('File not found: mock error');
    }
  });
});