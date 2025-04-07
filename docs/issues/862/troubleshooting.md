# GitHub Integration Troubleshooting Guide

This guide provides solutions to common issues with GitHub integration in OpenAgents.

## Connection Timeout Issues

### Symptoms
- "Connection timeout" error messages (522, 524)
- "Server error" messages
- Operations start but never complete

### Solutions
1. **Automatic Retries**: The system will automatically retry up to 2 times with a 2-second delay
2. **Wait and Try Again**: If automatic retries fail, wait a few minutes and try again
3. **Check GitHub Status**: Verify GitHub services are operational at [GitHub Status](https://www.githubstatus.com/)
4. **Try a Different Repository**: Some repositories might have specific access issues
5. **Check Network Connection**: Ensure your network connection is stable

## Authentication Errors

### Symptoms
- "GitHub authentication failed" (401)
- "Access forbidden" (403)
- "Unauthorized" messages

### Solutions
1. **Verify Token Validity**: 
   - Check Settings > API Keys to ensure your token is entered correctly
   - Generate a new token if necessary
   - Watch for whitespace in copied tokens

2. **Check Token Permissions**:
   - Ensure your token has the necessary permissions
   - For repositories: Contents: Read
   - For issues: Issues: Read/Write
   - For PRs: Pull requests: Read/Write

3. **Check Token Expiration**:
   - GitHub tokens have expiration dates
   - If expired, generate a new token
   - Update the token in Settings > API Keys

4. **Check Repository Access**:
   - Verify the token has access to the specified repository
   - For private repositories, ensure token has explicit access
   - Try accessing a public repository as a test

## Repository Not Found

### Symptoms
- "Repository not found" (404)
- "Resource doesn't exist" messages

### Solutions
1. **Check Repository Name**:
   - Verify the owner/repo format is correct (e.g., "openagents/repo")
   - Check for typos in repository names
   - Repository names are case-sensitive

2. **Verify Repository Exists**:
   - Open a browser and try to visit the repository on GitHub
   - Ensure the repository hasn't been deleted or renamed
   - Check if the repository is private and your token has access

3. **Check Branch Names**:
   - For branch-specific operations, ensure the branch exists
   - Remember that repositories may use "main" instead of "master"

## Rate Limit Errors

### Symptoms
- "Rate limit exceeded" (429)
- Operations fail with limits information

### Solutions
1. **Wait for Reset**:
   - GitHub API rate limits reset hourly
   - The error message should include the reset time
   - Wait until the reset time and try again

2. **Use Authentication**:
   - Unauthenticated requests have very low limits
   - Adding a token significantly increases rate limits
   - Check that your token is being used correctly

3. **Reduce API Calls**:
   - Limit the frequency of GitHub operations
   - Batch operations when possible
   - Consider caching results that don't change frequently

## Data Processing Errors

### Symptoms
- "Validation failed" (422)
- "Invalid parameters" messages
- Operations partially complete

### Solutions
1. **Check Request Format**:
   - Ensure required parameters are provided
   - Verify parameter types are correct
   - Check for invalid characters in strings

2. **Check Content Size**:
   - GitHub has limits on content size
   - Break large operations into smaller chunks
   - For file operations, check file size limits

3. **Check Permissions**:
   - Some operations require specific permissions
   - Branch protection rules might prevent pushes
   - Repository settings might restrict certain actions

## Environment-Specific Issues

### Node.js Environment Issues
- Token not found in environment variables
- Window reference errors

### Browser Environment Issues
- Event listener not triggered
- Local storage access issues

### Solutions
1. **Check Environment Detection**:
   - The code should automatically detect the environment
   - Server logs will indicate environment detection
   - Different code paths handle browser vs Node.js environments

2. **Verify Token Propagation**:
   - Tokens should flow from settings to environment
   - Check environment logs to verify token presence
   - Ensure token synchronization events are firing

## Debugging

### General Debugging Approaches
1. **Check Server Logs**:
   - Look for GitHub plugin initialization messages
   - Check for token presence in environment logs
   - Note any error messages and status codes

2. **Test with Known-Good Repositories**:
   - Use public repositories for testing
   - Try simple operations like listing issues
   - Gradually test more complex operations

3. **Verify Token Workflow**:
   - Add a new token in the UI
   - Check logs for token detection events
   - Verify token synchronization to MCP client

### Common Log Messages and Their Meaning

| Log Message | Meaning | Solution |
|-------------|---------|----------|
| "No GitHub token found" | Token missing in environment | Add token in Settings > API Keys |
| "GitHub plugin initialization complete" | Plugin loaded successfully | N/A - Working as expected |
| "Using MCP tool for..." | Operation is using MCP | N/A - Working as expected |
| "Connection timeout detected" | Network issue with GitHub | Wait for automatic retry or try again later |
| "Repository validation failed" | Repo doesn't exist or token lacks access | Verify repo name and token permissions |
| "Error executing MCP tool" | General MCP tool execution failure | Check parameters and try again |

## When All Else Fails

If you're still experiencing issues:

1. **Regenerate Your Token**:
   - Create a new GitHub token with all necessary permissions
   - Update it in Settings > API Keys
   - Test with the new token

2. **Try Different Repository**:
   - Test with a public repository you don't own
   - Try a repository you own
   - Compare behavior between different repositories

3. **Check GitHub API Status**:
   - GitHub API may be experiencing issues
   - Check [GitHub Status](https://www.githubstatus.com/)
   - Wait and try again later if services are degraded

4. **Report the Issue**:
   - Include specific error messages
   - Note repository names (if not sensitive)
   - Describe steps to reproduce the problem