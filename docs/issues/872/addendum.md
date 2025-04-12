# ConsentKeys OAuth Integration Addendum

## Client-Side Integration with ConsentKeys OIDC Provider

This document outlines the implementation of OAuth client integration with the ConsentKeys OIDC provider in the OpenAgents website application.

### Key Implementation Details

1. **Created a new utility file**: `apps/website/app/lib/oauth-utils.ts`
   - Contains pure utility functions for OAuth/OIDC client flows
   - Functions include:
     - `generateRandomString`: Creates secure random strings for state/nonce
     - `generateAuthorizationUrl`: Builds auth URLs for ConsentKeys OAuth
     - `startOAuthFlow`: Example implementation of OAuth flow initialization
     - `handleOAuthCallback`: Example implementation of OAuth callback handling
     - `getUserInfo`: Example of fetching user information with access token

2. **Configured Generic OAuth Plugin for ConsentKeys**
   - Added the `genericOAuth` plugin with ConsentKeys configuration:
   ```typescript
   import { genericOAuth } from "better-auth/plugins";
   
   // Auth configuration
   plugins: [
     genericOAuth({
       config: [{
         providerId: "consentkeys",
         clientId: env.CONSENTKEYS_CLIENT_ID || "",
         clientSecret: env.CONSENTKEYS_CLIENT_SECRET || "",
         authorizationUrl: "https://consentkeys.openagents.com/api/auth/oauth2/authorize",
         tokenUrl: "https://consentkeys.openagents.com/api/auth/oauth2/token",
         userInfoUrl: "https://consentkeys.openagents.com/api/auth/oauth2/userinfo",
         scopes: ["openid", "profile", "email"],
         responseType: "code",
         mapProfileToUser: (profile: Record<string, any>) => {
           return {
             id: profile.sub,
             name: profile.name,
             email: profile.email,
             image: profile.picture
           };
         }
       }]
     })
   ]
   ```

3. **Updated Login/Signup Components**
   - Used Better Auth's oauth2 methods for generic OAuth providers:
   ```typescript
   // Login form
   await signIn.oauth2({
     providerId: "consentkeys",
     callbackURL: "/",
   });

   // Signup form
   await signUp.oauth2({
     providerId: "consentkeys",
     callbackURL: "/",
   });
   ```

### Important Clarification

It's important to understand that our application is a client of the ConsentKeys OIDC provider, not an OIDC provider itself:

- **ConsentKeys** is the OIDC provider (authentication service)
- Our **OpenAgents website** is an OAuth client that uses ConsentKeys for authentication

The initial confusion stemmed from:
1. Mistakenly including the `oidcProvider` plugin, which is for setting up an application as an OIDC provider
2. Incorrectly configuring ConsentKeys as a social provider instead of using the `genericOAuth` plugin

### Correct Implementation Approach

1. **Removed OIDC Provider Plugin** - We should not use this plugin as our app is a client, not a provider.

2. **Used Generic OAuth Plugin** - The correct approach for custom OAuth providers:
   ```typescript
   // Import the genericOAuth plugin
   import { genericOAuth } from "better-auth/plugins";
   
   // Configure it in the plugins section
   plugins: [
     genericOAuth({
       config: [{
         providerId: "consentkeys",
         clientId: "YOUR_CLIENT_ID",
         clientSecret: "YOUR_CLIENT_SECRET",
         authorizationUrl: "https://provider.com/oauth2/authorize",
         tokenUrl: "https://provider.com/oauth2/token",
         userInfoUrl: "https://provider.com/oauth2/userinfo",
         scopes: ["openid", "profile", "email"],
         responseType: "code",
         mapProfileToUser: (profile) => ({
           id: profile.sub,
           name: profile.name,
           email: profile.email
         })
       }]
     })
   ],
   ```

3. **Removed Incorrect Social Provider Configuration** - OAuth providers should be configured using the genericOAuth plugin, not in the socialProviders section when they're not officially supported providers.

### Expected Authentication Flow

With these corrections in place, the OAuth flow should work as follows:

1. User clicks "Log in with ConsentKeys" on our website
2. User is redirected to ConsentKeys for authentication
3. After successful authentication, user is redirected back to our callback URL
4. Better Auth handles the token exchange and creates a user session
5. User is redirected to the application homepage

### Type Issues and Resolution

During implementation, we encountered type errors related to Better Auth's typings:

```
Type '{ handler: (request: Request<unknown, CfProperties<unknown>>) => Promise<Response>; api: ... }' 
is not assignable to type '{ handler: (request: Request<unknown, CfProperties<unknown>>) => Promise<Response>; api: ... }'
```

This issue was resolved by:

1. **Creating custom portable type definitions**: Created explicit type interfaces that avoid referencing internal node_modules paths
   ```typescript
   // In auth.ts
   type BetterAuthInstance = {
     handler: (request: Request) => Promise<Response>;
     api: any; // Using 'any' for the api property to avoid type conflicts
   };
   
   export const auth: BetterAuthInstance = betterAuth({...});
   
   // In auth-client.ts
   type AuthClient = {
     signIn: any;
     signUp: any;
     signOut: any;
     getSession: any;
     useSession: any;
   };
   
   export const authClient: AuthClient = createAuthClient({...});
   ```

2. **Creating a helper function**: Added a handler function to abstract away the direct auth.handler usage
   ```typescript
   const handleRequest = (request: Request) => {
     return auth.handler(request);
   };
   ```

3. **Using selective type annotation**: Applied types only where needed to maintain portability while avoiding type conflicts

These changes allow the application to work correctly while avoiding TypeScript errors that stem from incompatible type definitions between different versions or imports of the Better Auth library. They also prevent TypeScript from generating non-portable types that reference specific paths in node_modules.

### Future Improvements

1. Add custom branding for the ConsentKeys buttons
2. Implement proper error handling for failed authentication
3. Add loading states during the OAuth flow
4. Update to the latest Better Auth version when type issues are resolved upstream

The client-side OIDC utility functions we created (`oauth-utils.ts`) provide flexibility for custom OAuth flows if needed in the future.