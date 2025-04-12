# ConsentKeys OIDC Integration Summary

## Overview
This document outlines the implementation of OIDC authentication using the ConsentKeys OIDC provider for the OpenAgents platform. The integration enables users to sign in and sign up using their ConsentKeys accounts.

## Implementation Details

### 1. Server-Side Configuration

Added ConsentKeys as an OAuth2 provider in `apps/website/app/lib/auth.ts`:

```typescript
import { betterAuth } from "better-auth";
import { oidcProvider } from "better-auth/plugins";
import { env } from "cloudflare:workers";

// ...

export const auth: ReturnType<typeof betterAuth> = betterAuth({
  // ...existing configuration...
  
  account: {
    accountLinking: {
      enabled: true,
      trustedProviders: ["github", "consentkeys"] // Added consentkeys as trusted provider
    }
  },
  
  // ...

  socialProviders: {
    github: {
      clientId: env.GITHUB_CLIENT_ID,
      clientSecret: env.GITHUB_CLIENT_SECRET,
    },
    consentkeys: {
      type: "oauth2",
      clientId: env.CONSENTKEYS_CLIENT_ID,
      clientSecret: env.CONSENTKEYS_CLIENT_SECRET,
      issuer: "https://consentkeys.openagents.com",
      authorization: {
        url: "https://consentkeys.openagents.com/api/auth/oauth2/authorize",
        params: { scope: "openid profile email" }
      },
      token: {
        url: "https://consentkeys.openagents.com/api/auth/oauth2/token"
      },
      userinfo: {
        url: "https://consentkeys.openagents.com/api/auth/oauth2/userinfo"
      },
      profile: (profile: any) => {
        return {
          id: profile.sub,
          name: profile.name,
          email: profile.email,
          image: profile.picture
        };
      }
    },
  },
  
  plugins: [
    oidcProvider({
      loginPage: "/login"
    })
  ],
  
  // ...
});
```

### 2. Client-Side Configuration

Added the generic OAuth client plugin to `apps/website/app/lib/auth-client.ts`:

```typescript
import { createAuthClient } from "better-auth/react"; // Use the React client
import { genericOAuthClient } from "better-auth/client/plugins"; // Import OAuth2 client plugin

export const authClient: ReturnType<typeof createAuthClient> = createAuthClient({
  plugins: [
    genericOAuthClient(), // Add OAuth2 client plugin
  ],
});
```

### 3. UI Updates

#### Login Form

Added a "Sign in with ConsentKeys" button to the login form in `apps/website/app/components/login-form.tsx`:

```typescript
<Button
  variant="outline"
  className="w-full"
  type="button"
  onClick={async () => {
    try {
      setIsSubmitting(true);
      // Use OAuth2 sign-in for ConsentKeys
      await signIn.oauth2({
        providerId: "consentkeys",
        callbackURL: "/",
      });
    } catch (error) {
      console.error("ConsentKeys login error:", error);
      setError(error instanceof Error ? error.message : "Failed to login with ConsentKeys");
      setIsSubmitting(false);
    }
  }}
  disabled={isSubmitting}
>
  Sign in with ConsentKeys
</Button>
```

#### Sign Up Form

Added a "Sign up with ConsentKeys" button to the signup form in `apps/website/app/components/signup-form.tsx`:

```typescript
<Button 
  variant="outline" 
  className="w-full"
  type="button"
  onClick={async () => {
    try {
      setIsSubmitting(true);
      // Use OAuth2 sign-up for ConsentKeys
      await signUp.oauth2({
        providerId: "consentkeys", 
        callbackURL: "/",
      });
    } catch (error) {
      console.error("ConsentKeys signup error:", error);
      setError(error instanceof Error ? error.message : "Failed to sign up with ConsentKeys");
      setIsSubmitting(false);
    }
  }}
  disabled={isSubmitting}
>
  Sign up with ConsentKeys
</Button>
```

## Authentication Flow

1. User clicks "Sign in with ConsentKeys" or "Sign up with ConsentKeys"
2. User is redirected to ConsentKeys authentication page: `https://consentkeys.openagents.com/api/auth/oauth2/authorize`
3. After successful authentication, user is redirected back to: `https://v5.openagents.com/api/auth/oauth2/callback/consentkeys`
4. The callback endpoint processes the authentication, establishes a session, and redirects to the homepage (`/`)

## Environment Variables

The implementation requires the following environment variables to be set in the deployment environment:
- `CONSENTKEYS_CLIENT_ID`: The client ID obtained from ConsentKeys
- `CONSENTKEYS_CLIENT_SECRET`: The client secret obtained from ConsentKeys

## Testing

To test the implementation:
1. Click "Sign in with ConsentKeys" or "Sign up with ConsentKeys"
2. Verify redirection to ConsentKeys authentication page
3. Complete authentication on ConsentKeys
4. Verify redirection back to the application and successful session establishment