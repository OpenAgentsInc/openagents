# GitHub Token Management Implementation

**Date:** April 13, 2025  
**Author:** Claude

## Overview

This document outlines the implementation of GitHub Personal Access Token (PAT) management for the agent chat interface. The implementation replaces the previous approach of using environment variables with a user-managed token stored in localStorage.

## Problem Statement

The previous implementation relied on environment variables to provide GitHub tokens to the agent:

```typescript
// Old implementation - using environment variables
export async function loader({ params, context }: LoaderFunctionArgs) {
  const { agentId } = params;
  const { env } = context.cloudflare;

  return { id: agentId, githubToken: env.GITHUB_TOKEN };
}
```

This approach had several limitations:
- Tokens were hardcoded in environment configuration
- Users couldn't provide their own tokens
- No way to update tokens without redeploying
- All users shared the same token

## Solution

The solution involves:
1. Creating a GitHub token input component for the sidebar
2. Implementing localStorage persistence
3. Adding validation and feedback
4. Ensuring tokens are passed to the agent

## Implementation Details

### 1. GitHub Token Input Component

Created a new component at `apps/website/app/components/github-token-input.tsx`:

```tsx
import { useState, useEffect } from "react";
import { Input } from "~/components/ui/input";
import { Label } from "~/components/ui/label";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "~/components/ui/card";
import { Eye, EyeOff, Save, CheckCircle } from "lucide-react";

const TOKEN_STORAGE_KEY = "github_token";
const TOKEN_PREFIX = "github_pat_";

export function GitHubTokenInput() {
  const [token, setToken] = useState("");
  const [showToken, setShowToken] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load token from local storage on component mount
  useEffect(() => {
    const storedToken = localStorage.getItem(TOKEN_STORAGE_KEY);
    if (storedToken) {
      setToken(storedToken);
      setSaved(true);
    }
  }, []);

  const handleTokenChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newToken = e.target.value;
    setToken(newToken);
    setSaved(false);
    
    // Clear error if they start typing
    if (error) {
      setError(null);
    }
  };

  const handleSaveToken = () => {
    // Validate token format
    if (!token.startsWith(TOKEN_PREFIX)) {
      setError(`Token must start with "${TOKEN_PREFIX}"`);
      return;
    }

    // Save to local storage
    localStorage.setItem(TOKEN_STORAGE_KEY, token);
    setSaved(true);

    // Dispatch event to notify other components
    const event = new Event("github-token-changed");
    window.dispatchEvent(event);
  };

  return (
    <Card className="mb-4">
      <CardHeader className="pb-3">
        <CardTitle className="text-sm">GitHub Token</CardTitle>
        <CardDescription className="text-xs">
          Add your GitHub Personal Access Token to use GitHub tools
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div className="space-y-2">
          <div className="space-y-1">
            <Label htmlFor="github-token" className="text-xs">Personal Access Token</Label>
            <div className="flex gap-2">
              <div className="relative flex-1">
                <Input
                  id="github-token"
                  placeholder="github_pat_..."
                  value={token}
                  onChange={handleTokenChange}
                  type={showToken ? "text" : "password"}
                  className="pr-10 font-mono text-xs"
                />
                <button 
                  type="button"
                  onClick={() => setShowToken(!showToken)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  aria-label={showToken ? "Hide token" : "Show token"}
                >
                  {showToken ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                </button>
              </div>
              <button
                onClick={handleSaveToken}
                className="px-3 py-2 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90 flex items-center gap-1 disabled:opacity-50"
                disabled={!token || saved}
              >
                {saved ? <CheckCircle className="h-3.5 w-3.5" /> : <Save className="h-3.5 w-3.5" />}
                {saved ? "Saved" : "Save"}
              </button>
            </div>
            {error && (
              <p className="text-xs text-destructive mt-1">{error}</p>
            )}
            {saved && (
              <p className="text-xs text-green-600 dark:text-green-400 mt-1">Token saved!</p>
            )}
          </div>
          <p className="text-[10px] text-muted-foreground">
            Token must start with github_pat_ and have repo:read access.
          </p>
        </div>
      </CardContent>
    </Card>
  );
}
```

### 2. Agent Page Integration

Updated `apps/website/app/routes/agent/$agentId.tsx` to:
- Remove the environment variable dependency
- Add state for the GitHub token
- Retrieve token from localStorage
- Listen for token changes
- Include the token input in the sidebar
- Add warning and input field disabling when token is missing

Key changes:

```typescript
// Remove environment variable dependency
export async function loader({ params }: LoaderFunctionArgs) {
  const { agentId } = params;
  
  // For security, don't try to load agents on the server
  // Just return the ID and let client-side handle data lookup
  return { id: agentId };
}

function ClientOnly({ agentId, children }: { agentId: string, children: React.ReactNode }) {
  // ...existing state variables
  const [githubToken, setGithubToken] = useState<string | null>(null);
  // ...other variables

  // Set up component and load token
  useEffect(() => {
    // ...existing code
    
    // Load GitHub token from localStorage
    const storedToken = localStorage.getItem(TOKEN_STORAGE_KEY);
    if (storedToken) {
      setGithubToken(storedToken);
    }

    // Listen for GitHub token changes
    const handleTokenChange = () => {
      console.log("GitHub token changed, updating token");
      const updatedToken = localStorage.getItem(TOKEN_STORAGE_KEY);
      setGithubToken(updatedToken);
    };

    window.addEventListener('github-token-changed', handleTokenChange);

    // Clean up event listener on unmount
    return () => {
      // ...existing cleanup
      window.removeEventListener('github-token-changed', handleTokenChange);
    };
  }, [agentId, agentStore]);

  // Using the token in message submissions
  agent.send(JSON.stringify({
    githubToken: githubToken,
    userMessage: userMessage
  }));

  // In the render method, add token input and conditionally disable chat
  return (
    <div className="flex h-full">
      {/* Left Sidebar */}
      <div className="w-80 border-r overflow-y-auto flex flex-col">
        {/* ...existing code */}

        <div className="p-4">
          {/* GitHub Token Input */}
          <GitHubTokenInput />

          {/* Token missing warning */}
          {missingToken && (
            <div className="mb-4 p-3 text-xs rounded border border-amber-200 bg-amber-50 dark:bg-amber-900/10 dark:border-amber-800 text-amber-800 dark:text-amber-400">
              <div className="font-semibold mb-1">GitHub Token Required</div>
              <div>Please add your GitHub token above to use GitHub tools with this agent.</div>
            </div>
          )}

          {/* ...existing code */}
        </div>
      </div>

      {/* ...existing code */}

      {/* Disable input when token is missing */}
      <input
        ref={inputRef}
        type="text"
        value={input}
        onChange={(e) => setInput(e.target.value)}
        placeholder={!githubToken ? "Add GitHub token to chat" : "Type a message..."}
        disabled={connectionStatus !== 'connected' || !githubToken}
        className="flex-1 px-4 py-2 border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary bg-background"
      />
    </div>
  );
}
```

## Technical Design Decisions

1. **Event-based Communication**
   - Uses custom events (`github-token-changed`) to notify components of token changes
   - Allows real-time updates without prop drilling or complex state management

2. **Security Considerations**
   - Masks token input by default with show/hide toggle
   - Client-side validation ensures tokens start with `github_pat_`
   - Tokens stored only in localStorage, not in cookies or session state

3. **User Experience**
   - Provides clear validation feedback
   - Disables chat when token is missing
   - Shows success state when token is saved
   - Changes placeholder text to guide users

4. **Code Organization**
   - Isolates token management in a dedicated component
   - Follows existing styling patterns
   - Uses similar patterns to the Coder app's API key management

## Testing

Manual testing should include:
- Adding a valid token (starting with `github_pat_`)
- Attempting to add an invalid token
- Verifying the token persists after page refresh
- Confirming the token is sent with messages
- Checking that chat is disabled when no token is present

## Future Improvements

Potential enhancements:
- Add token validation with the GitHub API
- Support multiple tokens for different GitHub accounts
- Add token scopes display/validation
- Improve security with encryption in localStorage
- Add token expiration handling