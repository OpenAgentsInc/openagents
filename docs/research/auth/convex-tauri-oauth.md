# Authentication Integration Research Report for OpenAgents PR #1195 and PR #1214

## OAuth callback handling in Tauri backend

Based on the research, the **recommended approach for OAuth callback handling in Tauri is using the tauri-plugin-oauth**. This plugin solves the fundamental challenge of OAuth providers not allowing custom URI schemes for desktop applications.

### Implementation Strategy

The plugin works by spawning a temporary localhost server to capture OAuth redirects:

```rust
use tauri_plugin_oauth::start;

#[tauri::command]
async fn start_oauth_server(window: Window) -> Result<u16, String> {
    start(move |url| {
        // Verify the URL here for security
        let _ = window.emit("oauth_callback", url);
    })
    .map_err(|err| err.to_string())
}
```

For token storage, you have several options:

1. **Development**: Use localStorage for simplicity
2. **Production**: Use tauri-plugin-stronghold for encrypted storage
3. **Alternative**: Use native OS keyring integration

### Recommended Tauri Commands

Yes, you should add Tauri commands for callback handling:

```rust
#[tauri::command]
async fn exchange_code_for_token(code: String) -> Result<TokenResponse, String> {
    // Exchange authorization code for tokens
    let client = reqwest::Client::new();
    let response = client
        .post("https://auth.openagents.com/token")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("client_id", CLIENT_ID),
            ("redirect_uri", REDIRECT_URI),
        ])
        .send()
        .await?;

    response.json().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn store_tokens_securely(
    access_token: String,
    refresh_token: String,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Store in Stronghold or keyring for production
    // Use localStorage for development
}
```

## JWT validation strategy

**You should NOT implement JWT validation in the Rust client**. Here's why:

### Convex Handles JWT Validation Automatically

Convex has built-in JWT validation that:
- Validates JWT signatures using configured JWKS endpoints
- Supports RS256 and ES256 algorithms
- Validates standard claims: `iss`, `aud`, `exp`, `iat`, `sub`
- Provides validated user identity through `ctx.auth.getUserIdentity()`

### Production JWT Validation Approach

1. **Client-side (Tauri)**: Only parse JWT for UI logic (checking expiration)
2. **Server-side (Convex)**: Handles all cryptographic validation automatically
3. **No JWKS fetching needed**: Convex manages this internally

The proper approach is to let Convex handle all JWT validation server-side. Your Tauri client should simply:
- Store the JWT securely
- Pass it in the Authorization header to Convex
- Handle token refresh when needed

## User context duplication

**You ARE duplicating authentication by adding auth_userId and auth_githubId to arguments**. This should be removed.

### Why Manual Parameters Are Unnecessary

Convex's `ctx.auth.getUserIdentity()` automatically provides:
- `tokenIdentifier`: Unique user identifier
- `subject`: User ID from the auth provider
- `email`: User email (if available)
- `name`: User name (if available)
- Custom claims from your JWT

### Recommended Pattern

Instead of:
```javascript
// ❌ Don't do this
export const createMessage = mutation({
  args: {
    content: v.string(),
    auth_userId: v.string(),     // Remove this
    auth_githubId: v.string()    // Remove this
  },
  handler: async (ctx, args) => {
    // Using manual auth parameters
  }
});
```

Use this:
```javascript
// ✅ Do this
export const createMessage = mutation({
  args: {
    content: v.string()
  },
  handler: async (ctx, args) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) throw new Error("Unauthorized");

    const userId = identity.subject; // Or identity.tokenIdentifier
    // Use the validated identity
  }
});
```

## Convex function mapping

Based on Convex naming conventions, your functions should follow these patterns:

### Standard Naming Conventions

```javascript
// convex/messages.ts
export const listMessages = query({...});      // Get multiple
export const getMessage = query({...});        // Get single
export const createMessage = mutation({...});  // Create new
export const updateMessage = mutation({...});  // Update existing
export const deleteMessage = mutation({...});  // Delete

// convex/sessions.ts
export const getCurrentSession = query({...});
export const createSession = mutation({...});
export const endSession = mutation({...});

// convex/users.ts
export const getUser = query({...});
export const getOrCreateUser = mutation({...});
export const updateUserProfile = mutation({...});
```

### Integration with Existing Functions

If you have existing claude.ts functions, align the naming:
- Keep function names in camelCase
- Use descriptive verb-based names for mutations
- Use noun-based or getter patterns for queries
- Export functions as named exports

You'll need to create these Convex functions as they don't exist by default. The functions should be in the `convex/` directory of your project.

## User management flow

**Yes, session/message operations should ensure the user exists first**. Here's the recommended flow:

### Proper User Creation Flow

```javascript
// convex/users.ts
export const getOrCreateUser = mutation({
  handler: async (ctx) => {
    const identity = await ctx.auth.getUserIdentity();
    if (!identity) throw new Error("Unauthenticated");

    // Check if user exists
    let user = await ctx.db
      .query("users")
      .withIndex("by_token", (q) =>
        q.eq("tokenIdentifier", identity.tokenIdentifier)
      )
      .unique();

    // Create if doesn't exist
    if (!user) {
      const userId = await ctx.db.insert("users", {
        name: identity.name || "Anonymous",
        email: identity.email || "",
        tokenIdentifier: identity.tokenIdentifier,
        githubId: identity.githubId, // If using GitHub OAuth
        createdAt: Date.now()
      });
      user = await ctx.db.get(userId);
    }

    return user;
  }
});

// convex/messages.ts
export const createMessage = mutation({
  args: { content: v.string() },
  handler: async (ctx, args) => {
    // Ensure user exists
    const user = await ctx.runMutation(internal.users.getOrCreateUser);

    // Create message with user reference
    return await ctx.db.insert("messages", {
      content: args.content,
      userId: user._id,
      createdAt: Date.now()
    });
  }
});
```

### Best Practice Pattern

1. Always call `getOrCreateUser` at the start of authenticated operations
2. Use internal functions to avoid exposing user creation to clients
3. Store the user reference (`userId`) with all user-scoped data
4. Use database indexes for efficient user lookups

## Environment variables

**Use VITE_OPENAUTH_URL for client-side configuration**. Here's the standardized approach:

### Environment Variable Naming Convention

```bash
# .env.local (for development)
# Client-side variables (exposed to browser)
VITE_OPENAUTH_URL=https://auth.openagents.com
VITE_CONVEX_URL=https://your-app.convex.cloud
VITE_APP_URL=http://localhost:5173

# .env.production
# Production client-side variables
VITE_OPENAUTH_URL=https://auth.openagents.com
VITE_CONVEX_URL=https://your-app.convex.cloud
VITE_APP_URL=https://app.openagents.com

# Server-side only (not prefixed with VITE_)
OPENAUTH_DOMAIN=auth.openagents.com
GITHUB_CLIENT_SECRET=your-secret
CONVEX_DEPLOY_KEY=your-deploy-key
```

### Key Differences

- **VITE_OPENAUTH_URL**: Full URL used by frontend/Tauri app (client-side)
- **OPENAUTH_DOMAIN**: Domain only, used for server configuration (server-side)
- Vite requires `VITE_` prefix for any variable accessible in the browser
- Non-prefixed variables are server-side only and never exposed

### Usage in Code

```javascript
// Frontend/Tauri
const authUrl = import.meta.env.VITE_OPENAUTH_URL;

// OpenAuth server configuration
const domain = process.env.OPENAUTH_DOMAIN;
```

## Summary Recommendations

1. **OAuth Callback**: Use tauri-plugin-oauth with localhost server approach
2. **JWT Validation**: Let Convex handle it - no client-side validation needed
3. **User Context**: Remove manual auth parameters, use ctx.auth.getUserIdentity()
4. **Function Names**: Follow Convex camelCase conventions (getMessage, createUser, etc.)
5. **User Flow**: Implement getOrCreateUser pattern before all user-scoped operations
6. **Environment**: Use VITE_OPENAUTH_URL for client-side, OPENAUTH_DOMAIN for server-side

These patterns will ensure seamless integration between your enhanced Convex client (PR #1214) and the OpenAuth authentication system (PR #1195), starting with GitHub OAuth login support.
