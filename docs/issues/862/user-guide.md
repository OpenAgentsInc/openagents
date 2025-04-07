# GitHub Integration User Guide

## Overview

This guide explains how to set up and use GitHub integration in OpenAgents. With GitHub integration, you can access repositories, create issues, search code, and perform other GitHub operations directly through chat.

## Setting Up GitHub Integration

### Creating a GitHub Personal Access Token

1. Go to your GitHub account settings
2. Navigate to Developer settings > Personal access tokens > Fine-grained tokens
3. Click "Generate new token"
4. Give your token a name (e.g., "OpenAgents Integration")
5. Set an expiration date (recommended: 90 days or less)
6. Select the repositories you want to access
7. Set the following permissions:
   - Repository permissions:
     - Contents: Read (for reading files and code)
     - Issues: Read and Write (for creating/modifying issues)
     - Pull requests: Read and Write (for creating/modifying PRs)
8. Click "Generate token"
9. Copy the token immediately (you won't be able to see it again)

### Adding Your GitHub Token to OpenAgents

1. In OpenAgents, go to Settings > API Keys
2. Find the GitHub section
3. Paste your Personal Access Token in the input field
4. Click Save
5. The token will be securely stored and synchronized with all components

## Using GitHub Tools in Conversations

Once your GitHub token is set up, you can use GitHub functionality in your chats. Here are some examples of what you can do:

### Repository Operations

- **Get file contents**: "Show me the README file from openagents/repo"
  ```
  githubGetFile(owner="openagents", repo="repo", path="README.md")
  ```

- **Push files**: "Save these changes to openagents/repo"
  ```
  githubPushFiles(owner="openagents", repo="repo", branch="main", message="Update files", files=[{path:"file.txt", content:"new content"}])
  ```

- **Create repository**: "Create a new repository called my-project"
  ```
  githubCreateRepository(name="my-project", description="A new project", private=true)
  ```

- **Create branch**: "Create a new branch called feature/login"
  ```
  githubCreateBranch(owner="openagents", repo="repo", branch="feature/login", from_branch="main")
  ```

### Issue Operations

- **List issues**: "Show me open issues in openagents/repo"
  ```
  githubListIssues(owner="openagents", repo="repo", state="open")
  ```

- **Create issue**: "Create an issue about the login bug"
  ```
  githubCreateIssue(owner="openagents", repo="repo", title="Login bug", body="The login feature is broken")
  ```

- **Get issue details**: "Tell me about issue #42"
  ```
  githubGetIssue(owner="openagents", repo="repo", issue_number=42)
  ```

- **Update issue**: "Close issue #42"
  ```
  githubUpdateIssue(owner="openagents", repo="repo", issue_number=42, state="closed")
  ```

### Pull Request Operations

- **List pull requests**: "Show me open pull requests"
  ```
  githubListPullRequests(owner="openagents", repo="repo", state="open")
  ```

- **Create pull request**: "Create a pull request from my feature branch"
  ```
  githubCreatePullRequest(owner="openagents", repo="repo", title="Add login feature", body="This PR adds login functionality", head="feature/login", base="main")
  ```

- **Get pull request**: "Show me details for PR #15"
  ```
  githubGetPullRequest(owner="openagents", repo="repo", pull_number=15)
  ```

### Search Operations

- **Search code**: "Find examples of API key handling"
  ```
  githubSearchCode(query="apiKey", language="typescript", user="openagents")
  ```

### Commit Operations

- **List commits**: "Show recent commits to the main branch"
  ```
  githubListCommits(owner="openagents", repo="repo", sha="main")
  ```

## Troubleshooting

### Authentication Errors

If you see messages like "GitHub authentication failed" or "Unauthorized access":

1. Check that your token is correctly entered in Settings > API Keys
2. Verify that your token hasn't expired
3. Ensure your token has the necessary permissions for the action
4. Try generating a new token if issues persist

### Connection Issues

If you see "Connection timeout" or "Server error" messages:

1. This is typically a temporary issue with the GitHub server connection
2. Wait a few moments and try your request again
3. The system will automatically retry up to two times for most connection issues
4. If problems persist, try a different repository or action

### Repository Not Found

If you see "Repository not found" errors:

1. Double-check the repository owner and name for typos
2. Verify that the repository exists
3. Ensure your token has access to the repository (especially for private repositories)
4. Check that your token has the correct permissions

### Rate Limit Errors

If you see "GitHub API rate limit exceeded":

1. Wait until the rate limit resets (usually 1 hour)
2. Consider using a token with higher rate limits (authenticated users get higher limits)
3. Reduce the frequency of GitHub API requests

## Privacy and Security

- Your GitHub token is stored securely and never exposed in client-side code
- Only token length is logged for debugging purposes, never the actual token
- Be cautious about which repositories you grant access to
- Use tokens with the minimum necessary permissions
- Set reasonable expiration dates and rotate tokens regularly
- Consider using different tokens for different applications

## Permissions and Scope

Depending on what GitHub operations you want to perform, your token may need different permissions:

| Operation | Required Permissions |
|-----------|----------------------|
| View repositories | Contents: Read |
| List issues | Issues: Read |
| Create issues | Issues: Write |
| View pull requests | Pull requests: Read |
| Create pull requests | Pull requests: Write |
| Access workflows | Actions: Read |

Only grant the permissions you need for your specific use case.