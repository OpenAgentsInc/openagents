/**
 * Direct GitHub tools implementation that bypasses MCP
 * This provides a direct API version of the GitHub tools for the Cloudflare Worker environment
 */

// Helper function to make GitHub API requests
async function callGitHubAPI(endpoint: string, options: RequestInit = {}, token?: string): Promise<any> {
  const url = `https://api.github.com${endpoint}`;
  const headers = {
    'Accept': 'application/vnd.github.v3+json',
    'User-Agent': 'OpenAgentsInc-Coder-Agent',
    ...options.headers
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
        return `Error decoding content: ${e.message}`;
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
            error: `Repository not found or token doesn't have access to ${owner}/${repo}. Check that the repository exists and your token has proper permissions.`
          });
        } else if (error.message.includes('401') || error.message.includes('403')) {
          return JSON.stringify({
            error: `Authentication failed for creating issue in ${owner}/${repo}. Make sure you've added a GitHub token with 'repo' scope in Settings > API Keys.`
          });
        }
      }
      
      throw error;
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