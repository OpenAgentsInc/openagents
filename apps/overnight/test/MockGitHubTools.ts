import { Effect } from 'effect';

/**
 * Helper function to create a standalone mock GitHubTools implementation
 * that can be used in tests without effect dependencies
 */
export const createMockGitHubTools = () => {
  return {
    getFileContent: ({ owner, repo, path, ref }: { owner: string, repo: string, path: string, ref?: string }) => 
      Effect.succeed(`Mock file content for ${owner}/${repo}/${path}${ref ? ` at ${ref}` : ''}`),
    
    getIssue: ({ owner, repo, issueNumber }: { owner: string, repo: string, issueNumber: number }) => 
      Effect.succeed(`Mock issue #${issueNumber} from ${owner}/${repo}`)
  };
};

/**
 * Mock GitHubTools that always fail with error messages
 */
export const createFailingMockGitHubTools = () => {
  return {
    getFileContent: () => Effect.fail('File not found: mock error'),
    getIssue: () => Effect.fail('Issue not found: mock error')
  };
};

/**
 * Creates a GitHubTools implementation that validates inputs before returning mocked results
 */
export const createValidatingMockGitHubTools = () => {
  return {
    getFileContent: ({ owner, repo, path }: { owner: string, repo: string, path: string, ref?: string }) => {
      // Validate required inputs
      if (!owner || !repo || !path) {
        return Effect.fail(`Missing required parameter: ${!owner ? 'owner' : !repo ? 'repo' : 'path'}`);
      }
      
      return Effect.succeed(`Validated mock file content for ${owner}/${repo}/${path}`);
    },
    
    getIssue: ({ owner, repo, issueNumber }: { owner: string, repo: string, issueNumber: number }) => {
      // Validate required inputs
      if (!owner || !repo || !issueNumber) {
        return Effect.fail(`Missing required parameter: ${!owner ? 'owner' : !repo ? 'repo' : 'issueNumber'}`);
      }
      
      // Validate issue number is a positive number
      if (typeof issueNumber !== 'number' || issueNumber <= 0) {
        return Effect.fail(`Invalid issue number: ${issueNumber}`);
      }
      
      return Effect.succeed(`Validated mock issue #${issueNumber} from ${owner}/${repo}`);
    }
  };
};