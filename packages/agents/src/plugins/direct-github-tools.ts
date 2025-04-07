/**
 * Direct GitHub tools implementation that bypasses MCP
 * This provides a direct API version of the GitHub tools for the Cloudflare Worker environment
 */

// Helper function to make GitHub API requests
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
      
      // Provide more specific error messages based on status code
      if (response.status === 401) {
        throw new Error(`GitHub authentication failed (401): The provided token is invalid or has expired.`);
      } else if (response.status === 403) {
        const resetTime = response.headers.get('X-RateLimit-Reset');
        if (resetTime) {
          const resetDate = new Date(parseInt(resetTime) * 1000);
          throw new Error(`GitHub API rate limit exceeded (403): Rate limit will reset at ${resetDate.toLocaleString()}.`);
        }
        throw new Error(`GitHub API access forbidden (403): The token might not have the required permissions for this operation.`);
      } else if (response.status === 404) {
        throw new Error(`GitHub resource not found (404): The requested resource (${endpoint}) doesn't exist or the token doesn't have access to it.`);
      } else if (response.status === 422) {
        throw new Error(`GitHub validation failed (422): The request contains invalid parameters. Details: ${errorText}`);
      } else {
        throw new Error(`GitHub API error (${response.status}): ${errorText}`);
      }
    }

    return response.json();
  } catch (error) {
    console.error(`GitHub API call failed for ${url}:`, error);
    throw error;
  }
}

export const directGitHubTools = {
  // Repository operations
  async getFileContents(owner: string, repo: string, path: string, branch?: string, token?: string): Promise<string> {
    const ref = branch ? `?ref=${branch}` : '';
    const result = await callGitHubAPI(`/repos/${owner}/${repo}/contents/${path}${ref}`, {}, token);
    
    // GitHub returns content as base64 encoded
    if (result.content && result.encoding === 'base64') {
      try {
        // In Workers environment, use atob for base64 decoding
        return atob(result.content.replace(/\n/g, ''));
      } catch (e) {
        const errorMessage = e instanceof Error ? e.message : String(e);
        return `Error decoding content: ${errorMessage}`;
      }
    }
    
    return JSON.stringify(result);
  },
  
  // Issue operations
  async listIssues(owner: string, repo: string, state?: string, sort?: string, direction?: string, token?: string): Promise<string> {
    let queryParams = '?';
    if (state) queryParams += `state=${state}&`;
    if (sort) queryParams += `sort=${sort}&`;
    if (direction) queryParams += `direction=${direction}&`;
    
    const result = await callGitHubAPI(`/repos/${owner}/${repo}/issues${queryParams.slice(0, -1)}`, {}, token);
    return JSON.stringify(result);
  },
  
  async createIssue(owner: string, repo: string, title: string, body: string, labels?: string[], token?: string): Promise<string> {
    try {
      console.log(`Creating issue in repository ${owner}/${repo}`);
      console.log(`Issue title: "${title}"`);
      console.log(`Issue body length: ${body.length} characters`);
      console.log(`Labels: ${labels ? JSON.stringify(labels) : 'none'}`);
      console.log(`Token provided: ${token ? 'Yes' : 'No'}`);
      
      // Check if repository exists before attempting to create an issue
      try {
        console.log(`Validating repository ${owner}/${repo} exists...`);
        await callGitHubAPI(`/repos/${owner}/${repo}`, { method: 'GET' }, token);
        console.log(`Repository ${owner}/${repo} exists and is accessible.`);
      } catch (repoError) {
        console.error(`Repository validation failed for ${owner}/${repo}:`, repoError);
        if (repoError instanceof Error && repoError.message.includes('404')) {
          return JSON.stringify({
            error: `Repository "${owner}/${repo}" not found. Please check that the repository exists and is spelled correctly. If this is a private repository, make sure your token has access to it.`
          });
        }
        // Re-throw other errors to be handled below
        throw repoError;
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
    const result = await callGitHubAPI(`/repos/${owner}/${repo}/issues/${issue_number}`, {}, token);
    return JSON.stringify(result);
  },
  
  // Pull request operations
  async listPullRequests(owner: string, repo: string, state?: string, sort?: string, direction?: string, token?: string): Promise<string> {
    let queryParams = '?';
    if (state) queryParams += `state=${state}&`;
    if (sort) queryParams += `sort=${sort}&`;
    if (direction) queryParams += `direction=${direction}&`;
    
    const result = await callGitHubAPI(`/repos/${owner}/${repo}/pulls${queryParams.slice(0, -1)}`, {}, token);
    return JSON.stringify(result);
  },
  
  // Commit operations
  async listCommits(owner: string, repo: string, sha?: string, page?: number, perPage?: number, token?: string): Promise<string> {
    let queryParams = '?';
    if (sha) queryParams += `sha=${sha}&`;
    if (page) queryParams += `page=${page}&`;
    if (perPage) queryParams += `per_page=${perPage}&`;
    
    const result = await callGitHubAPI(`/repos/${owner}/${repo}/commits${queryParams.slice(0, -1)}`, {}, token);
    return JSON.stringify(result);
  }
};