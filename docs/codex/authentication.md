# Authentication System

## Overview

The OpenAI Codex CLI implements a sophisticated authentication system supporting multiple authentication methods, secure token management, and seamless integration with both OpenAI API keys and ChatGPT OAuth flows. The system is designed with enterprise-grade security practices and comprehensive error handling.

## Architecture

### Core Components

1. **AuthManager** (`core/src/auth.rs`) - Central authentication management
2. **Token Management** (`core/src/token_data.rs`) - JWT and token handling
3. **OAuth Login Server** (`login/src/server.rs`) - PKCE OAuth implementation
4. **PKCE Security** (`login/src/pkce.rs`) - OAuth security measures
5. **Authentication Storage** - Secure credential persistence

## Authentication Methods

### 1. ChatGPT OAuth Authentication (Recommended)

**Purpose**: Integration with ChatGPT subscription plans (Plus, Pro, Team, Business, Enterprise, Edu)

**Implementation**: OAuth 2.0 authorization code flow with PKCE

```bash
# Start OAuth login flow
codex login

# Login will open browser to:
# https://auth.openai.com/oauth/authorize
```

**OAuth Configuration**:
- **Client ID**: `app_EMoamEEZ73f0CkXaXp7hrann`
- **Authorization Server**: `https://auth.openai.com`
- **Scopes**: `openid profile email offline_access`
- **Callback URL**: `http://localhost:1455/callback`

### 2. API Key Authentication (Legacy)

**Purpose**: Traditional usage-based billing with OpenAI API keys

**Configuration Methods**:

1. **Direct Login**:
   ```bash
   codex login --api-key "your-api-key"
   ```

2. **Environment Variable**:
   ```bash
   export OPENAI_API_KEY="your-api-key"
   codex exec "task description"
   ```

3. **Configuration File**:
   ```json
   {
     "OPENAI_API_KEY": "your-api-key"
   }
   ```

## OAuth Implementation Details

### PKCE Security Flow

Location: `login/src/pkce.rs`

```rust
pub fn generate_pkce() -> PkceCodes {
    let mut bytes = [0u8; 64];
    rand::rng().fill_bytes(&mut bytes);
    let code_verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let digest = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    PkceCodes { code_verifier, code_challenge }
}
```

**Security Features**:
- **Code Verifier**: 64 random bytes, base64url encoded
- **Code Challenge**: SHA256 hash of verifier, base64url encoded
- **Method**: S256 (SHA256) for maximum security
- **State Parameter**: CSRF protection

### Local OAuth Server

Location: `login/src/server.rs`

**Features**:
- Binds only to localhost (127.0.0.1:1455) for security
- Handles OAuth callback and token exchange
- Automatic browser opening for user convenience
- State validation for CSRF protection

**Flow**:
1. Start local server on `localhost:1455`
2. Open browser to OpenAI authorization URL
3. User completes authentication
4. Receive authorization code via callback
5. Exchange code for tokens using PKCE
6. Store tokens securely in `auth.json`

## Token Management

### Token Data Structure

Location: `core/src/token_data.rs`

```rust
pub struct TokenData {
    pub id_token: IdTokenInfo,     // JWT with user info and plan details
    pub access_token: String,      // JWT for API access
    pub refresh_token: String,     // For token renewal
    pub account_id: Option<String>, // ChatGPT account identifier
}
```

### JWT Token Processing

**ID Token Claims**:
- User email and profile information
- ChatGPT plan type (free, plus, pro, team, business, enterprise, edu)
- Organization and project identifiers
- Account setup status

**Access Token**:
- Bearer token for API requests
- JWT format with expiration
- Used for all OpenAI API calls

### Automatic Token Refresh

```rust
// 28-day refresh cycle
if last_refresh < Utc::now() - chrono::Duration::days(28) {
    let refresh_response = tokio::time::timeout(
        Duration::from_secs(60),
        try_refresh_token(tokens.refresh_token.clone(), &self.client),
    ).await?;
    // Update tokens and persist to disk
}
```

**Refresh Features**:
- **Trigger**: 28 days after last refresh
- **Timeout**: 60-second timeout for refresh requests
- **Endpoint**: `https://auth.openai.com/oauth/token`
- **Persistence**: Automatic update of auth.json
- **Error Handling**: Graceful fallback on failures

## Authentication Storage

### File Location and Security

**Storage Path**: `$CODEX_HOME/auth.json` (defaults to `~/.codex/auth.json`)

**Security Measures**:
- **Unix Permissions**: `0o600` (read/write for owner only)
- **JSON Format**: Structured credential storage
- **Atomic Updates**: Prevents corruption during writes

### Auth.json Structure

```json
{
  "OPENAI_API_KEY": "optional-api-key",
  "tokens": {
    "id_token": {
      "email": "user@example.com",
      "chatgpt_plan": "plus",
      "organizations": ["org-123"],
      "has_account_setup": true
    },
    "access_token": "eyJ0eXAiOiJKV1QiLCJhbGciOi...",
    "refresh_token": "refresh-token-string",
    "account_id": "account-123"
  },
  "last_refresh": "2025-01-15T10:30:00Z"
}
```

## Session Management

### AuthManager Implementation

Location: `core/src/auth.rs`

```rust
pub struct AuthManager {
    codex_home: PathBuf,
    inner: RwLock<CachedAuth>,
}
```

**Features**:
- **Thread Safety**: RwLock for concurrent access
- **Caching**: Single source of truth for auth state
- **Consistency**: Prevents mid-run auth state changes
- **Reload Mechanism**: Explicit reload for external changes

### Authentication Priority

**Resolution Order** (highest to lowest priority):
1. API key in `auth.json`
2. ChatGPT tokens in `auth.json`
3. Environment variable `OPENAI_API_KEY`

## Login and Logout Processes

### Login Implementation

**ChatGPT Login**:
```rust
pub async fn login_with_chatgpt(codex_home: PathBuf) -> std::io::Result<()> {
    let opts = ServerOptions::new(codex_home, CLIENT_ID.to_string());
    let server = run_login_server(opts)?;
    server.block_until_done().await
}
```

**API Key Login**:
```rust
pub fn login_with_api_key(codex_home: &Path, api_key: &str) -> std::io::Result<()> {
    let auth_dot_json = AuthDotJson {
        openai_api_key: Some(api_key.to_string()),
        tokens: None,
        last_refresh: None,
    };
    write_auth_json(&get_auth_file(codex_home), &auth_dot_json)
}
```

### Logout Process

```bash
codex logout
```

**Implementation**:
- Deletes `auth.json` file
- Clears in-memory authentication cache
- Returns status indicating success/failure

## Error Handling

### Authentication Errors

**Comprehensive Error Types**:
- Token data unavailable
- Network timeouts during refresh
- Invalid JWT format
- Missing authorization codes
- State parameter mismatches
- File I/O errors
- JSON parsing errors

### Error Recovery Strategies

**Token Refresh Failures**:
```rust
match try_refresh_token(refresh_token).await {
    Ok(new_tokens) => update_auth_file(new_tokens),
    Err(e) => {
        log::warn!("Token refresh failed: {}", e);
        // Continue with existing tokens
    }
}
```

**Network Issues**:
- Timeout handling with user-friendly messages
- Retry logic for transient failures
- Offline mode detection

**Invalid Credentials**:
- Clear error messages with suggested actions
- Automatic re-authentication prompts
- Recovery guidance for common issues

## Configuration and Overrides

### Environment Variables

- `OPENAI_API_KEY`: Direct API key override
- `CODEX_HOME`: Custom authentication storage location

### CLI Configuration Overrides

```bash
# Override authentication method
codex exec --config auth_mode=api_key "task"

# Force re-authentication
codex login --force
```

### Profile Integration

```toml
[profiles.dev]
auth_mode = "api_key"

[profiles.production]
auth_mode = "chatgpt"
organization_id = "org-123"
```

## Security Considerations

### Network Security

1. **HTTPS Only**: All OAuth endpoints use HTTPS
2. **Localhost Binding**: OAuth server binds only to 127.0.0.1
3. **State Validation**: CSRF protection in OAuth flow
4. **Code Challenge**: PKCE prevents authorization code interception

### Storage Security

1. **File Permissions**: Restrictive Unix permissions (0o600)
2. **Token Rotation**: 28-day refresh cycle limits exposure
3. **Secure Directory**: Stored in user's home directory
4. **Atomic Writes**: Prevents corruption during updates

### Best Practices

1. **Least Privilege**: Tokens have minimal required scope
2. **Secure Defaults**: OAuth preferred over API keys
3. **Error Isolation**: Auth errors don't expose sensitive data
4. **Input Validation**: Comprehensive validation of all auth data

## Integration Examples

### CI/CD Pipeline

```yaml
# GitHub Actions
- name: Setup Codex Authentication
  run: |
    echo "${{ secrets.OPENAI_API_KEY }}" | \
    codex login --api-key "$(cat)"
    
- name: Run Codex Tasks
  run: |
    codex exec --full-auto "run tests and analyze results"
```

### Docker Container

```dockerfile
# Option 1: Environment variable
ENV OPENAI_API_KEY=your-api-key

# Option 2: Mount auth file
COPY auth.json /root/.codex/auth.json
RUN chmod 600 /root/.codex/auth.json
```

### Headless Servers

```bash
# SSH with port forwarding for OAuth
ssh -L 1455:localhost:1455 remote-server
codex login  # On remote server

# Or use API key for automation
export OPENAI_API_KEY="key-for-automation"
codex exec "automated task"
```

## Troubleshooting

### Common Issues

1. **Permission Denied**: Check file permissions on `auth.json`
2. **Network Timeout**: Verify internet connectivity and proxy settings
3. **Invalid Token**: Run `codex logout && codex login` to re-authenticate
4. **Browser Issues**: Use `--no-browser` flag and manual URL copy

### Debug Commands

```bash
# Check authentication status
codex auth status

# Verify token validity
codex auth check

# Refresh tokens manually
codex auth refresh
```

The authentication system provides robust, secure access to OpenAI services while maintaining ease of use and comprehensive error handling. The dual support for OAuth and API keys ensures compatibility with different usage patterns and deployment scenarios.