# Convex Rust SDK Authentication

## Summary

✅ **The Convex Rust SDK HAS built-in authentication support!**

The `ConvexClient` provides a `set_auth()` method that accepts user tokens from any auth provider.

## API Documentation

### User Authentication

```rust
pub async fn set_auth(&mut self, token: Option<String>)
```

**Purpose**: Set authentication for use when calling Convex functions.

**Parameters**:
- `token`: `Option<String>`
  - `Some(token)` - Set the auth token from your auth provider
  - `None` - Clear auth (logout)

**From source** (`convex/src/client/mod.rs:347-362`):
```rust
/// Set auth for use when calling Convex functions.
///
/// Set it with a token that you get from your auth provider via their login
/// flow. If `None` is passed as the token, then auth is unset (logging
/// out).
pub async fn set_auth(&mut self, token: Option<String>) {
    let req = AuthenticateRequest {
        token: match token {
            None => AuthenticationToken::None,
            Some(token) => AuthenticationToken::User(token),
        },
    };
    self.request_sender
        .send(ClientRequest::Authenticate(req))
        .expect("INTERNAL BUG: Worker has gone away");
}
```

### Admin Authentication (Advanced)

```rust
pub async fn set_admin_auth(
    &mut self,
    deploy_key: String,
    acting_as: Option<UserIdentityAttributes>,
)
```

**Purpose**: Authenticate as deployment admin (not typically required).

**Use case**: Development/testing to act as specific users.

**From source** (`convex/src/client/mod.rs:364-382`):
```rust
/// Set admin auth for use when calling Convex functions as a deployment
/// admin. Not typically required.
///
/// You can get a deploy_key from the Convex dashboard's deployment settings
/// page. Deployment admins can act as users as part of their
/// development flow to see how a function would act.
#[doc(hidden)]
pub async fn set_admin_auth(
    &mut self,
    deploy_key: String,
    acting_as: Option<UserIdentityAttributes>,
) {
    let req = AuthenticateRequest {
        token: AuthenticationToken::Admin(deploy_key, acting_as),
    };
    self.request_sender
        .send(ClientRequest::Authenticate(req))
        .expect("INTERNAL BUG: Worker has gone away");
}
```

## Implementation for OpenAgents

### Architecture

```
Frontend (TypeScript)
    ↓
Get auth token from Convex
    ↓
Pass to Rust via Tauri command
    ↓
Rust ConvexClient.set_auth(token)
    ↓
All mutations/queries use this auth
```

### Step 1: Get Auth Token from Frontend

**File**: `tauri/src/lib/convex-auth.ts` (new)

```typescript
import { convexClient } from "./convexClient";

export async function getConvexAuthToken(): Promise<string | null> {
  // Get token from Convex client
  // Note: Need to check Convex React SDK for how to get the token
  // Likely something like:
  // const token = await convexClient.auth.fetchToken();
  // return token;

  // TODO: Research exact API for getting token from ConvexReactClient
  return null;
}
```

### Step 2: Create Tauri Command to Set Auth

**File**: `tauri/src-tauri/src/commands.rs`

```rust
use crate::convex_client::ConvexClientManager;

#[tauri::command]
pub async fn set_convex_auth(
    app: AppHandle,
    token: Option<String>,
) -> Result<(), String> {
    let mut manager = CONVEX_MANAGER.lock().await;

    if let Some(client) = &mut manager.client {
        client.set_auth(token).await;
        Ok(())
    } else {
        Err("Convex client not initialized".to_string())
    }
}
```

### Step 3: Call from Frontend on Login

**File**: `tauri/src/App.tsx` or auth component

```typescript
import { invoke } from "@tauri-apps/api/core";
import { getConvexAuthToken } from "@/lib/convex-auth";

// When user logs in
async function handleLogin() {
  // Login flow happens via Convex Auth
  // ...

  // Get the token
  const token = await getConvexAuthToken();

  // Pass to Rust backend
  if (token) {
    await invoke("set_convex_auth", { token });
  }
}

// When user logs out
async function handleLogout() {
  // Clear auth in Rust
  await invoke("set_convex_auth", { token: null });
}
```

### Step 4: Convex Client Manager in Rust

**File**: `tauri/src-tauri/src/convex_client.rs`

```rust
use convex::ConvexClient;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;

pub struct ConvexClientManager {
    pub client: Option<ConvexClient>,
}

pub static CONVEX_MANAGER: Lazy<Mutex<ConvexClientManager>> =
    Lazy::new(|| Mutex::new(ConvexClientManager { client: None }));

impl ConvexClientManager {
    pub async fn initialize(deployment_url: &str) -> Result<(), Box<dyn std::error::Error>> {
        let client = ConvexClient::new(deployment_url).await?;

        let mut manager = CONVEX_MANAGER.lock().await;
        manager.client = Some(client);

        Ok(())
    }

    pub async fn get_client() -> Option<ConvexClient> {
        let manager = CONVEX_MANAGER.lock().await;
        manager.client.clone()
    }
}
```

### Step 5: Initialize on App Start

**File**: `tauri/src-tauri/src/main.rs`

```rust
mod convex_client;

use convex_client::ConvexClientManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize Convex client if enabled
    if std::env::var("VITE_USE_CONVEX").unwrap_or_default() == "true" {
        let deployment_url = std::env::var("VITE_CONVEX_URL")?;
        ConvexClientManager::initialize(&deployment_url).await?;
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            set_convex_auth,
            // ... other commands
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
```

## Auth Token Flow

1. **User logs in** via Convex Auth (frontend)
2. **Frontend gets token** from Convex React SDK
3. **Frontend calls Tauri command** `set_convex_auth(token)`
4. **Rust receives token** and calls `client.set_auth(Some(token))`
5. **All subsequent mutations** use this token for auth
6. **Convex backend** validates token and populates `ctx.auth` in functions

## Token Lifecycle

- **On app start**: No token (anonymous)
- **On login**: Set token via `set_auth(Some(token))`
- **On logout**: Clear token via `set_auth(None)`
- **On token refresh**: Update token via `set_auth(Some(new_token))`

## Security Considerations

✅ **Token never logged**: Don't log tokens in Rust
✅ **Secure passing**: Tauri commands are secure IPC
✅ **Token refresh**: Frontend handles refresh, passes new token to Rust
✅ **Per-user data**: Convex enforces auth via `ctx.auth` in functions

## Testing

**Test auth flow**:
```rust
#[tokio::test]
async fn test_auth_flow() -> anyhow::Result<()> {
    let mut client = ConvexClient::new("https://test.convex.cloud").await?;

    // No auth initially
    client.set_auth(None).await;

    // Set auth token
    client.set_auth(Some("test_token_123".to_string())).await;

    // Make authenticated mutation
    let result = client.mutation("myMutation", maplit::btreemap!{}).await?;

    // Clear auth
    client.set_auth(None).await;

    Ok(())
}
```

## Open Questions

1. **How to get token from ConvexReactClient?**
   - Need to check Convex React SDK API
   - Likely `convexClient.auth.fetchToken()` or similar
   - May need to access Clerk/Auth0 directly

2. **How to handle token refresh?**
   - Frontend should monitor token expiration
   - Call `set_convex_auth` with new token when refreshed
   - Or set up interval to refresh proactively

3. **Can we clone ConvexClient?**
   - Yes! `ConvexClient` implements `Clone`
   - All clones share the same connection
   - Auth is shared across all clones

## Conclusion

✅ **Authentication is fully supported** in Convex Rust SDK
✅ **Simple API**: `client.set_auth(Option<String>)`
✅ **Async**: No blocking
✅ **Flexible**: Works with any auth provider

The main work is:
1. Get token from frontend Convex client
2. Pass to Rust via Tauri command
3. Call `set_auth()` on Rust ConvexClient

**This confirms Option B (Rust → Convex directly) is the right choice!**
