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
     - Contents: Read
     - Issues: Read and Write (if you want to create/modify issues)
     - Pull requests: Read and Write (if you want to create/modify PRs)
8. Click "Generate token"
9. Copy the token immediately (you won't be able to see it again)

### Adding Your GitHub Token to OpenAgents

1. In OpenAgents, go to Settings > API Keys
2. Find the GitHub section
3. Paste your Personal Access Token in the input field
4. Click Save

## Using GitHub Tools in Conversations

Once your GitHub token is set up, you can use GitHub functionality in your chats. Here are some examples of what you can do:

- **Search repositories**: "Find repositories related to AI agents"
- **Get repository information**: "Show me details about openagents/repo"
- **List issues**: "List open issues in openagents/repo"
- **Create an issue**: "Create an issue in openagents/repo about GitHub token handling"
- **Search code**: "Find examples of API key handling in openagents/repo"

## Troubleshooting

### Authentication Errors

If you see messages like "GitHub authentication failed" or "Unauthorized access":

1. Check that your token is correctly entered in Settings > API Keys
2. Verify that your token hasn't expired
3. Ensure your token has the necessary permissions for the action
4. Try generating a new token if issues persist

### Rate Limit Errors

If you see "GitHub API rate limit exceeded":

1. Wait until the rate limit resets (usually 1 hour)
2. Consider using a token with higher rate limits (authenticated users get higher limits)

## Privacy and Security

- Your GitHub token is stored securely and never exposed in client-side code
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
