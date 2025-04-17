/**
 * Direct GitHub tools implementation that bypasses MCP
 * This provides a direct API version of the GitHub tools for the Cloudflare Worker environment
 */

// Helper function to make GitHub API requests
async function callGitHubAPI<T>(endpoint: string, options: RequestInit = {}, token?: string): Promise<T> {
  const url = `https://api.github.com${endpoint}`;
  const headers = {
    'Accept': 'application/vnd.github.v3+json',
    'User-Agent': 'OpenAgentsInc-Coder-Agent',
    ...options.headers
  };

  // Add authorization if token is provided
  if (token) {
    headers['Authorization'] = `token ${token}`;
  }

  const response = await fetch(url, {
    ...options,
    headers
  });

  if (!response.ok) {
    const errorText = await response.text();
    throw new Error(`GitHub API error (${response.status}): ${errorText}`);
  }

  return response.json() as Promise<T>;
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
    const result = await callGitHubAPI(
      `/repos/${owner}/${repo}/issues`,
      {
        method: 'POST',
        body: JSON.stringify({ title, body, labels })
      },
      token
    );
    return JSON.stringify(result);
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
