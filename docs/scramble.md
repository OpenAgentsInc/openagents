# Scramble Authentication in OpenAgents

## Overview

OpenAgents uses Scramble OIDC as its primary authentication provider for email/password login. Scramble provides robust authentication while maintaining strong privacy guarantees for users through pseudonymization.

## Why Scramble?

### Privacy-First Design

- Uses pseudonymization so OpenAgents never sees real user data
- Each application gets a unique, stable identifier for users
- OpenAgents only receives pseudonymous IDs, not actual user data
- Reduces our liability and data protection requirements

### Technical Benefits

- Standard OAuth2/OIDC integration
- Built-in security features (rate limiting, brute force protection)
- No need to handle password storage/reset flows
- Clean separation between auth and application logic

## Implementation

### Authentication Flow

1. User enters email in login/signup form
2. Frontend checks if email exists in our database
3. Based on existence check:
   - New users go to signup flow
   - Existing users go to login flow
4. User is redirected to Scramble OIDC with appropriate parameters
5. Scramble handles authentication
6. User is redirected back with authorization code
7. Backend exchanges code for tokens
8. User session is created and user is redirected to app

### Key Components

#### Frontend (React)

```typescript
// login-form.tsx
export function LoginForm() {
  const [email, setEmail] = useState("");
  const [isExistingUser, setIsExistingUser] = useState<boolean | null>(null);

  // Check if email exists in our database
  const checkEmail = async (email: string) => {
    const response = await fetch(
      `/api/users/check-email?email=${encodeURIComponent(email)}`,
    );
    const data = await response.json();
    setIsExistingUser(data.exists);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!isExistingUser) {
      window.location.href = `/auth/scramble/signup?email=${encodeURIComponent(email)}`;
    } else {
      window.location.href = `/auth/scramble/login?email=${encodeURIComponent(email)}`;
    }
  };
}
```

#### Backend (Rust)

```rust
// OAuth configuration
pub struct ScrambleOAuth {
    service: OAuthService,
    pool: PgPool,
    verifier_store: VerifierStore,
}

// PKCE verifier storage
pub struct VerifierStore {
    store: Arc<RwLock<HashMap<String, StoredVerifier>>>,
}

// Login/signup handlers
pub async fn scramble_login(
    State(state): State<AppState>,
    params: Option<Query<LoginParams>>,
) -> Response {
    let (url, csrf_token, pkce_verifier) = state
        .scramble_oauth
        .authorization_url_for_login(&form_data.email);

    // Store PKCE verifier
    state.verifier_store.store_verifier(
        csrf_token.secret(),
        pkce_verifier.clone()
    );

    Redirect::to(&url).into_response()
}

// Callback handler
pub async fn scramble_callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Response {
    // Get stored verifier
    let pkce_verifier = state
        .verifier_store
        .get_verifier(&params.state)?;

    // Exchange code for tokens
    let token_response = state
        .scramble_oauth
        .exchange_code(params.code, pkce_verifier)
        .await?;

    // Create or update user
    let user = state
        .scramble_oauth
        .authenticate(token_response)
        .await?;

    // Create session
    let session = Session::create(user.id, &state.pool).await?;

    // Set session cookie and redirect
    Response::builder()
        .header("Set-Cookie", session.to_cookie_string())
        .status(302)
        .header("Location", "/chat")
        .body(Body::empty())
        .unwrap()
}
```

### Security Features

1. **PKCE (Proof Key for Code Exchange)**

   - Prevents authorization code interception attacks
   - Verifier stored securely on backend
   - Automatic cleanup of old verifiers

2. **State Parameter**

   - Prevents CSRF attacks
   - Includes signup/login flag in state
   - Verified on callback

3. **Session Management**

   - HTTP-only cookies
   - Secure session storage
   - Proper session expiration

4. **Email Verification**
   - Checks email existence before auth flow
   - Prevents account enumeration
   - Proper error handling

## Database Schema

```sql
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    scramble_id VARCHAR(255) UNIQUE NOT NULL,
    metadata JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    last_login_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token VARCHAR(255) NOT NULL UNIQUE,
    expires_at TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_users_scramble_id ON users(scramble_id);
CREATE INDEX idx_sessions_token ON sessions(token);
CREATE INDEX idx_sessions_user_id ON sessions(user_id);
```

## Configuration

Required environment variables:

```env
SCRAMBLE_CLIENT_ID=client_...
SCRAMBLE_CLIENT_SECRET=secret_...
SCRAMBLE_AUTH_URL=https://auth.scramblesolutions.com/oauth2/auth
SCRAMBLE_TOKEN_URL=https://auth.scramblesolutions.com/oauth2/token
SCRAMBLE_CALLBACK_URL=http://localhost:5173/auth/callback
```

## Testing

1. **Unit Tests**

   - PKCE verifier generation and storage
   - State parameter handling
   - Token exchange and validation
   - Session creation and validation

2. **Integration Tests**

   - Complete auth flow with mock Scramble server
   - Database operations
   - Session management
   - Error handling

3. **End-to-End Tests**
   - User signup flow
   - User login flow
   - Session persistence
   - Logout flow

## Future Improvements

1. **Remember Me Functionality**

   - Implement longer-lasting sessions
   - Add refresh token support
   - Store device information

2. **Multi-Factor Authentication**

   - Support Scramble's MFA features
   - Add backup codes
   - Implement device confirmation

3. **Session Management UI**

   - View active sessions
   - Revoke individual sessions
   - See login history

4. **Enhanced Security**
   - Add rate limiting
   - Implement progressive delays
   - Add audit logging
