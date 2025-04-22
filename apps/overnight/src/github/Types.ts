/**
 * Common GitHub API types
 */

/**
 * Configuration for GitHub clients
 */
export interface GitHubConfig {
  readonly baseUrl: string
  readonly token?: string
}
