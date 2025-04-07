/**
 * Direct GitHub tools implementation that bypasses MCP
 * This provides a direct API version of the GitHub tools for the Cloudflare Worker environment
 */

// Helper function to make GitHub API requests
/**
 * Makes a GitHub API request with proper error handling
 * @param endpoint The GitHub API endpoint
 * @param options Request options
 * @param token Optional GitHub token
 * @returns Response data or throws an error
 */
async function callGitHubAPI(endpoint: string, options: RequestInit = {}, token?: string): Promise<any> {
  const url = `https://api.github.com${endpoint}`;
  const headers: Record<string, string> = {
    'Accept': 'application/vnd.github.v3+json',
    'User-Agent': 'OpenAgentsInc-Coder-Agent',
    ...(options.headers as Record<string, string> || {})
  };

  // Add authorization if token is provided
  const hasToken = !!token;
  if (token) {
    // GitHub API uses 'token' prefix, not 'Bearer'
    headers['Authorization'] = `token ${token}`;
    console.log(`Using GitHub token for request to ${endpoint} (token length: ${token.length})`);
  } else {
    console.warn(`No GitHub token provided for request to ${endpoint}. API rate limits will be lower and private repositories will be inaccessible.`);
  }

  try {
    // Validate endpoint - must start with /
    if (!endpoint.startsWith('/')) {
      throw new Error(`Invalid GitHub API endpoint: ${endpoint}. Must start with /`);
    }
    
    console.log(`Making GitHub API request to: ${url}`);
    console.log(`Request method: ${options.method || 'GET'}`);
    console.log(`Authorization header present: ${hasToken}`);
    
    const response = await fetch(url, {
      ...options,
      headers
    });

    if (!response.ok) {
      const errorText = await response.text();
      console.error(`GitHub API error (${response.status}) for ${url}:`, errorText);
      
      let errorDetails = '';
      try {
        // Try to parse error response as JSON
        const errorData = JSON.parse(errorText);
        errorDetails = errorData.message || errorText;
      } catch {
        // If not valid JSON, use the text directly
        errorDetails = errorText;
      }
      
      // Provide more specific error messages based on status code
      if (response.status === 401) {
        throw new Error(`GitHub authentication failed (401): The provided token is invalid or has expired. Details: ${errorDetails}`);
      } else if (response.status === 403) {
        const resetTime = response.headers.get('X-RateLimit-Reset');
        if (resetTime) {
          const resetDate = new Date(parseInt(resetTime) * 1000);
          throw new Error(`GitHub API rate limit exceeded (403): Rate limit will reset at ${resetDate.toLocaleString()}. Details: ${errorDetails}`);
        }
        throw new Error(`GitHub API access forbidden (403): The token might not have the required permissions for this operation. Details: ${errorDetails}`);
      } else if (response.status === 404) {
        throw new Error(`GitHub resource not found (404): The requested resource (${endpoint}) doesn't exist or the token doesn't have access to it. Details: ${errorDetails}`);
      } else if (response.status === 422) {
        throw new Error(`GitHub validation failed (422): The request contains invalid parameters. Details: ${errorDetails}`);
      } else {
        throw new Error(`GitHub API error (${response.status}): ${errorDetails}`);
      }
    }

    try {
      return await response.json();
    } catch (jsonError) {
      console.warn('Error parsing JSON response:', jsonError);
      throw new Error(`Error parsing GitHub API response: ${jsonError instanceof Error ? jsonError.message : String(jsonError)}`);
    }
  } catch (error) {
    console.error(`GitHub API call failed for ${url}:`, error);
    throw error;
  }
}

/**
 * Helper function to validate a repository exists and is accessible
 * Returns null if validation succeeds, or an error message if it fails
 */
async function validateRepository(owner: string, repo: string, token?: string): Promise<string | null> {
  if (!token) {
    console.log(`No token provided, skipping repository validation for ${owner}/${repo}`);
    return null;
  }
  
  try {
    console.log(`Validating repository ${owner}/${repo} exists...`);
    await callGitHubAPI(`/repos/${owner}/${repo}`, { method: 'GET' }, token);
    console.log(`Repository ${owner}/${repo} exists and is accessible.`);
    return null;
  } catch (repoError) {
    console.error(`Repository validation failed for ${owner}/${repo}:`, repoError);
    if (repoError instanceof Error) {
      if (repoError.message.includes('404')) {
        return `Repository "${owner}/${repo}" not found. Please check that the repository exists and is spelled correctly. If this is a private repository, make sure your token has access to it.`;
      } else if (repoError.message.includes('401')) {
        return `GitHub authentication failed. Your token is invalid or expired. Please add a valid GitHub token in Settings > API Keys.`;
      } else if (repoError.message.includes('403')) {
        return `Access forbidden to repository "${owner}/${repo}". Your token doesn't have permission to access this repository.`;
      }
    }
    return `Error validating repository ${owner}/${repo}: ${repoError instanceof Error ? repoError.message : String(repoError)}`;
  }
}

export const directGitHubTools = {
  // Repository operations
  async getFileContents(owner: string, repo: string, path: string, branch?: string, token?: string): Promise<string> {
    try {
      console.log(`Getting file contents from ${owner}/${repo}/${path}`);
      
      // Validate repository exists
      const validationError = await validateRepository(owner, repo, token);
      if (validationError) {
        console.log(`Repository validation failed: ${validationError}`);
        return JSON.stringify({ error: validationError });
      }
      
      const ref = branch ? `?ref=${branch}` : '';
      const result = await callGitHubAPI(`/repos/${owner}/${repo}/contents/${path}${ref}`, {}, token);
      
      // GitHub returns content as base64 encoded
      if (result.content && result.encoding === 'base64') {
        try {
          // In Workers environment, use atob for base64 decoding
          return atob(result.content.replace(/\n/g, ''));
        } catch (e) {
          const errorMessage = e instanceof Error ? e.message : String(e);
          return JSON.stringify({ error: `Error decoding content: ${errorMessage}` });
        }
      }
      
      return JSON.stringify(result);
    } catch (error) {
      console.error(`Error getting file contents from ${owner}/${repo}/${path}:`, error);
      return JSON.stringify({ 
        error: `Failed to get file contents: ${error instanceof Error ? error.message : String(error)}` 
      });
    }
  },
  
  // Issue operations
  async listIssues(owner: string, repo: string, state?: string, sort?: string, direction?: string, token?: string): Promise<string> {
    try {
      console.log(`Listing issues for ${owner}/${repo}`);
      
      // Validate repository exists
      const validationError = await validateRepository(owner, repo, token);
      if (validationError) {
        console.log(`Repository validation failed: ${validationError}`);
        return JSON.stringify({ error: validationError });
      }
      
      let queryParams = '?';
      if (state) queryParams += `state=${state}&`;
      if (sort) queryParams += `sort=${sort}&`;
      if (direction) queryParams += `direction=${direction}&`;
      
      const result = await callGitHubAPI(`/repos/${owner}/${repo}/issues${queryParams.slice(0, -1)}`, {}, token);
      return JSON.stringify(result);
    } catch (error) {
      console.error(`Error listing issues for ${owner}/${repo}:`, error);
      return JSON.stringify({ 
        error: `Failed to list issues: ${error instanceof Error ? error.message : String(error)}` 
      });
    }
  },
  
  async createIssue(owner: string, repo: string, title: string, body: string, labels?: string[], token?: string): Promise<string> {
    try {
      console.log(`Creating issue in repository ${owner}/${repo}`);
      console.log(`Issue title: "${title}"`);
      console.log(`Issue body length: ${body.length} characters`);
      console.log(`Labels: ${labels ? JSON.stringify(labels) : 'none'}`);
      console.log(`Token provided: ${token ? 'Yes' : 'No'}`);
      
      // Validate repository exists
      const validationError = await validateRepository(owner, repo, token);
      if (validationError) {
        console.log(`Repository validation failed: ${validationError}`);
        return JSON.stringify({ error: validationError });
      }
      
      const requestBody = JSON.stringify({ title, body, labels });
      console.log(`Request body: ${requestBody}`);
      
      const result = await callGitHubAPI(
        `/repos/${owner}/${repo}/issues`,
        {
          method: 'POST',
          body: requestBody
        },
        token
      );
      
      console.log(`Issue created successfully, issue number: ${result.number}`);
      return JSON.stringify(result);
    } catch (error) {
      console.error(`Error creating issue in ${owner}/${repo}:`, error);
      
      // Provide more useful error message
      if (error instanceof Error) {
        if (error.message.includes('404')) {
          return JSON.stringify({
            error: `Repository "${owner}/${repo}" not found or token doesn't have access to it. Please check the following:
1. The repository name is spelled correctly
2. The repository exists
3. Your GitHub token has the 'repo' scope for private repositories
4. You have write access to the repository`
          });
        } else if (error.message.includes('401')) {
          return JSON.stringify({
            error: `GitHub authentication failed. Your token is invalid or expired. Please add a valid GitHub token in Settings > API Keys.`
          });
        } else if (error.message.includes('403')) {
          return JSON.stringify({
            error: `Access forbidden to repository "${owner}/${repo}". Your token doesn't have permission to create issues in this repository. Make sure your token has the 'repo' scope.`
          });
        } else if (error.message.includes('422')) {
          return JSON.stringify({
            error: `Validation error when creating issue in "${owner}/${repo}". The issue title or body may be invalid, or you might be missing required fields.`
          });
        }
      }
      
      // Return a generic error message instead of throwing
      return JSON.stringify({
        error: `Failed to create issue in ${owner}/${repo}: ${error instanceof Error ? error.message : String(error)}`
      });
    }
  },
  
  async getIssue(owner: string, repo: string, issue_number: number, token?: string): Promise<string> {
    try {
      console.log(`Getting issue #${issue_number} from ${owner}/${repo}`);
      
      // Validate repository exists
      const validationError = await validateRepository(owner, repo, token);
      if (validationError) {
        console.log(`Repository validation failed: ${validationError}`);
        return JSON.stringify({ error: validationError });
      }
      
      const result = await callGitHubAPI(`/repos/${owner}/${repo}/issues/${issue_number}`, {}, token);
      return JSON.stringify(result);
    } catch (error) {
      console.error(`Error getting issue #${issue_number} from ${owner}/${repo}:`, error);
      return JSON.stringify({ 
        error: `Failed to get issue: ${error instanceof Error ? error.message : String(error)}` 
      });
    }
  },
  
  // Pull request operations
  async listPullRequests(owner: string, repo: string, state?: string, sort?: string, direction?: string, token?: string): Promise<string> {
    try {
      console.log(`Listing pull requests for ${owner}/${repo}`);
      
      // Validate repository exists
      const validationError = await validateRepository(owner, repo, token);
      if (validationError) {
        console.log(`Repository validation failed: ${validationError}`);
        return JSON.stringify({ error: validationError });
      }
      
      let queryParams = '?';
      if (state) queryParams += `state=${state}&`;
      if (sort) queryParams += `sort=${sort}&`;
      if (direction) queryParams += `direction=${direction}&`;
      
      const result = await callGitHubAPI(`/repos/${owner}/${repo}/pulls${queryParams.slice(0, -1)}`, {}, token);
      return JSON.stringify(result);
    } catch (error) {
      console.error(`Error listing pull requests for ${owner}/${repo}:`, error);
      return JSON.stringify({ 
        error: `Failed to list pull requests: ${error instanceof Error ? error.message : String(error)}` 
      });
    }
  },
  
  // Commit operations
  async listCommits(owner: string, repo: string, sha?: string, page?: number, perPage?: number, token?: string): Promise<string> {
    try {
      console.log(`Listing commits for ${owner}/${repo}`);
      
      // Validate repository exists
      const validationError = await validateRepository(owner, repo, token);
      if (validationError) {
        console.log(`Repository validation failed: ${validationError}`);
        return JSON.stringify({ error: validationError });
      }
      
      let queryParams = '?';
      if (sha) queryParams += `sha=${sha}&`;
      if (page) queryParams += `page=${page}&`;
      if (perPage) queryParams += `per_page=${perPage}&`;
      
      const result = await callGitHubAPI(`/repos/${owner}/${repo}/commits${queryParams.slice(0, -1)}`, {}, token);
      return JSON.stringify(result);
    } catch (error) {
      console.error(`Error listing commits for ${owner}/${repo}:`, error);
      return JSON.stringify({ 
        error: `Failed to list commits: ${error instanceof Error ? error.message : String(error)}` 
      });
    }
  }
};