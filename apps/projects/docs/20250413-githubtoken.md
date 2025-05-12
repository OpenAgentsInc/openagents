# GitHub Token Management

**2025-04-13**

## Changes Made

1. Created `GitHubTokenInput` component for the sidebar
2. Updated `$agentId.tsx` to use localStorage instead of env variables
3. Added token validation and persistence

## Implementation Details

- **Storage**: Uses localStorage with key "github_token"
- **Validation**: Ensures tokens start with "github_pat_"
- **Updates**: Uses custom event "github-token-changed" to notify components
- **UX**: Disables chat input when token is missing, shows validation errors

## Files Modified

- New: `/app/components/github-token-input.tsx`
- Updated: `/app/routes/agent/$agentId.tsx`

## Security Considerations

- Tokens stored in localStorage (client-side only)
- Password field masked by default with show/hide toggle
- No server-side token storage