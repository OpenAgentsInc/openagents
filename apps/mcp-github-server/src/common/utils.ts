import { getUserAgent } from "universal-user-agent";
import { createGitHubError } from "./errors.js";
import { VERSION } from "./version.js";

type RequestOptions = {
  method?: string;
  body?: unknown;
  headers?: Record<string, string>;
  token?: string;
}

async function parseResponseBody(response: Response): Promise<unknown> {
  const contentType = response.headers.get("content-type");
  if (contentType?.includes("application/json")) {
    return response.json();
  }
  return response.text();
}

export function buildUrl(baseUrl: string, params: Record<string, string | number | undefined>): string {
  const url = new URL(baseUrl);
  Object.entries(params).forEach(([key, value]) => {
    if (value !== undefined) {
      url.searchParams.append(key, value.toString());
    }
  });
  return url.toString();
}

const USER_AGENT = `modelcontextprotocol/servers/github/v${VERSION} ${getUserAgent()}`;

export async function githubRequest(
  url: string,
  options: RequestOptions = {}
): Promise<unknown> {
  const headers: Record<string, string> = {
    "Accept": "application/vnd.github.v3+json",
    "Content-Type": "application/json",
    "User-Agent": USER_AGENT,
    ...options.headers,
  };

  if (options.token) {
    headers["Authorization"] = `Bearer ${options.token}`;
  }
  
  // Add logging for debugging the request
  console.log(`🔄 GitHub API Request: ${url.substring(0, url.indexOf('?') > 0 ? url.indexOf('?') : url.length)}`);
  console.log(`🔑 Auth present: ${!!options.token}`);
  
  try {
    const response = await fetch(url, {
      method: options.method || "GET",
      headers,
      body: options.body ? JSON.stringify(options.body) : undefined,
    });
    
    console.log(`📥 GitHub API Response status: ${response.status}`);
    const responseBody = await parseResponseBody(response);

    if (!response.ok) {
      // Enhanced error logging
      console.error(`❌ GitHub API Error [${response.status}]: ${JSON.stringify(responseBody).substring(0, 200)}`);
      
      // For rate limit errors, add more details
      if (response.status === 429) {
        const resetTime = response.headers.get('x-ratelimit-reset');
        const remaining = response.headers.get('x-ratelimit-remaining');
        console.error(`⏱️ Rate limit details - Remaining: ${remaining}, Reset time: ${resetTime ? new Date(parseInt(resetTime) * 1000).toISOString() : 'unknown'}`);
      }
      
      // For auth errors, give a hint that public repos should work without auth
      if (response.status === 401 || response.status === 403) {
        console.warn(`🔑 Note: Public repositories should be accessible without authentication. This could be a rate limit issue or the repo might be private.`);
      }
      
      throw createGitHubError(response.status, responseBody);
    }

    return responseBody;
  } catch (error) {
    // Catch network errors that might not be handled by response.ok check
    if (!(error instanceof GitHubError)) {
      console.error(`❌ GitHub API Network Error: ${error instanceof Error ? error.message : String(error)}`);
    }
    throw error;
  }
}

export function validateBranchName(branch: string): string {
  const sanitized = branch.trim();
  if (!sanitized) {
    throw new Error("Branch name cannot be empty");
  }
  if (sanitized.includes("..")) {
    throw new Error("Branch name cannot contain '..'");
  }
  if (/[\s~^:?*[\\\]]/.test(sanitized)) {
    throw new Error("Branch name contains invalid characters");
  }
  if (sanitized.startsWith("/") || sanitized.endsWith("/")) {
    throw new Error("Branch name cannot start or end with '/'");
  }
  if (sanitized.endsWith(".lock")) {
    throw new Error("Branch name cannot end with '.lock'");
  }
  return sanitized;
}

export function validateRepositoryName(name: string): string {
  const sanitized = name.trim().toLowerCase();
  if (!sanitized) {
    throw new Error("Repository name cannot be empty");
  }
  if (!/^[a-z0-9_.-]+$/.test(sanitized)) {
    throw new Error(
      "Repository name can only contain lowercase letters, numbers, hyphens, periods, and underscores"
    );
  }
  if (sanitized.startsWith(".") || sanitized.endsWith(".")) {
    throw new Error("Repository name cannot start or end with a period");
  }
  return sanitized;
}

export function validateOwnerName(owner: string): string {
  const sanitized = owner.trim().toLowerCase();
  if (!sanitized) {
    throw new Error("Owner name cannot be empty");
  }
  if (!/^[a-z0-9](?:[a-z0-9]|-(?=[a-z0-9])){0,38}$/.test(sanitized)) {
    throw new Error(
      "Owner name must start with a letter or number and can contain up to 39 characters"
    );
  }
  return sanitized;
}

export async function checkBranchExists(
  owner: string,
  repo: string,
  branch: string
): Promise<boolean> {
  try {
    await githubRequest(
      `https://api.github.com/repos/${owner}/${repo}/branches/${branch}`
    );
    return true;
  } catch (error) {
    if (error && typeof error === "object" && "status" in error && error.status === 404) {
      return false;
    }
    throw error;
  }
}

export async function checkUserExists(username: string): Promise<boolean> {
  try {
    await githubRequest(`https://api.github.com/users/${username}`);
    return true;
  } catch (error) {
    if (error && typeof error === "object" && "status" in error && error.status === 404) {
      return false;
    }
    throw error;
  }
}
