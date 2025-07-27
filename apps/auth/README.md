# OpenAgents Authentication Service

This is the OpenAuth-powered authentication service for OpenAgents, deployed on Cloudflare Workers.

## Setup

### 1. GitHub OAuth App

Create a new GitHub OAuth App at https://github.com/settings/applications/new:

- **Application name**: OpenAgents Auth
- **Homepage URL**: `https://your-domain.com`
- **Authorization callback URL**: `https://auth.openagents.com/callback/github`

Note your Client ID and Client Secret.

### 2. Environment Variables

Set the following secrets using Wrangler:

```bash
cd apps/auth
wrangler secret put GITHUB_CLIENT_ID
wrangler secret put GITHUB_CLIENT_SECRET
wrangler secret put COOKIE_DOMAIN  # Set to ".openagents.com" for production
```

### 3. KV Namespace Setup

Create KV namespaces for authentication storage:

```bash
cd apps/auth
wrangler kv namespace create "AUTH_STORAGE"
wrangler kv namespace create "AUTH_STORAGE" --preview
```

**Important**: For security, namespace IDs should **never** be hardcoded in configuration files.

Create a local environment file (not committed to git):

```bash
# Create .env.local with your namespace IDs
cat > .env.local << EOF
# KV Namespace Configuration (DO NOT COMMIT)
KV_AUTH_STORAGE_ID=your_production_namespace_id
KV_AUTH_STORAGE_PREVIEW_ID=your_preview_namespace_id
EOF
```

Replace the placeholder IDs with the actual IDs returned from the namespace creation commands above.

### 4. Custom Domain (Production)

For production deployment, configure a custom domain:

1. Add `auth.openagents.com` as a custom domain in Cloudflare dashboard
2. Update DNS to point to your worker
3. SSL certificates will be automatically provisioned

## Development

Run the auth service locally:

```bash
cd apps/auth
bun run dev
```

The service will be available at `http://localhost:8787`.

## Deployment

### Secure Deployment (Recommended)

Use the secure deployment script that manages KV namespace IDs via environment variables:

```bash
cd apps/auth
./deploy.sh
```

This script:
- Loads KV namespace IDs from `.env.local` (not committed to git)
- Creates a temporary configuration with the namespace IDs
- Deploys the worker securely
- Cleans up temporary files

### Alternative Deployment Methods

Deploy using SST from the project root:

```bash
# From project root
bun sst deploy
```

**Note**: Direct wrangler deployment without the secure script will fail due to missing KV namespace configuration.

## API Endpoints

The authentication service provides these key endpoints:

- `GET /authorize` - Start OAuth flow
- `GET /callback/github` - GitHub OAuth callback
- `GET /token` - Token exchange endpoint
- `GET /user` - Get user info (requires valid token)
- `GET /.well-known/oauth-authorization-server` - OAuth server metadata

## Integration

### Desktop App

The desktop app uses the authorization code flow with PKCE:

1. User clicks "Login" → opens system browser to `/authorize`
2. User completes GitHub OAuth
3. Callback redirects to `openagents://auth/callback` with authorization code
4. Desktop app exchanges code for JWT tokens
5. Tokens are stored securely and used for Convex requests

### Mobile App

The mobile app uses `expo-auth-session` with the same flow:

1. User taps "Login" → opens secure web view
2. User completes GitHub OAuth
3. Deep link callback with authorization code
4. App exchanges code for JWT tokens
5. Tokens stored in `expo-secure-store`

### Convex Integration

Convex validates JWT tokens from OpenAuth:

```typescript
// convex/auth.config.ts
export default {
  providers: [
    {
      domain: "https://auth.openagents.com",
      applicationID: "openagents",
    },
  ],
};
```

## Security

### Authentication Security
- Cookies are configured for subdomain sharing (`.openagents.com`)
- HTTPS enforced in production
- Secure, httpOnly cookies
- CSRF protection via state parameter
- PKCE for public clients
- Token rotation on refresh

### Configuration Security
- **KV namespace IDs**: Never hardcoded in configuration files, managed via environment variables
- **Secrets management**: GitHub OAuth credentials stored using `wrangler secret`
- **Environment isolation**: Separate namespaces for production and preview environments
- **Repository safety**: Sensitive configuration files (`.env.local`) excluded from git

### Deployment Security
- Secure deployment script prevents accidental exposure of namespace IDs
- Temporary configuration files are automatically cleaned up after deployment
- Environment variable validation ensures required values are present before deployment