import { issuer } from "@openauthjs/openauth";
import { CloudflareStorage } from "@openauthjs/openauth/storage/cloudflare";
import { GithubProvider } from "@openauthjs/openauth/provider/github";
import { subjects, type User } from "./subjects";

// Store provider tokens (GitHub access tokens) in KV
// Key format: "provider_tokens:{user_sub}"
interface ProviderTokens {
  github?: {
    access_token: string;
    refresh_token?: string;
    expires_at?: number;
  };
}

// GitHub API user response
interface GitHubUser {
  id: number;
  login: string;
  email: string;
  name?: string;
  avatar_url?: string;
}

// Helper functions for provider token management
async function storeProviderTokens(storage: any, userSub: string, tokens: ProviderTokens): Promise<void> {
  const key = `provider_tokens:${userSub}`;
  await storage.put(key, JSON.stringify(tokens));
  console.log(`🔑 [AUTH] Stored provider tokens for user ${userSub}`);
}

async function getProviderTokens(storage: any, userSub: string): Promise<ProviderTokens | null> {
  const key = `provider_tokens:${userSub}`;
  const data = await storage.get(key);
  if (!data) return null;
  
  try {
    return JSON.parse(data);
  } catch (error) {
    console.error(`❌ [AUTH] Failed to parse provider tokens for user ${userSub}:`, error);
    return null;
  }
}

// User management functions
async function getUser(claims: { sub: string; github_id: string; email: string; name?: string; avatar?: string; username: string }): Promise<User> {
  // Validate required fields
  console.log("🔍 [AUTH] Validating user claims:", JSON.stringify(claims, null, 2));
  
  const missing = [];
  if (!claims.sub) missing.push("sub");
  if (!claims.github_id) missing.push("github_id");
  if (!claims.email) missing.push("email");
  if (!claims.username) missing.push("username");
  
  if (missing.length > 0) {
    throw new Error(`Missing required user claims: ${missing.join(", ")}`);
  }

  // For now, we'll create a user on-the-fly
  // In a real implementation, you might want to store this in a database
  // TODO: Implement persistent user storage
  return {
    id: claims.sub,
    email: claims.email,
    name: claims.name,
    avatar: claims.avatar,
    githubId: claims.github_id,
    githubUsername: claims.username,
  };
}

export default {
  async fetch(request: Request, env: any, ctx: ExecutionContext) {
    const url = new URL(request.url);
    
    // Store env in a closure so it can be accessed by issuer callbacks
    const storage = env.AUTH_STORAGE;
    
    // Handle /user endpoint to return user data from JWT
    if (url.pathname === '/user' && request.method === 'GET') {
      try {
        const authHeader = request.headers.get('Authorization');
        if (!authHeader?.startsWith('Bearer ')) {
          return new Response('Unauthorized', { status: 401 });
        }
        
        const token = authHeader.slice(7);
        // For now, return a simple response - we'll decode JWT properly later
        return new Response(JSON.stringify({
          id: "14167547",
          email: "chris@openagents.com",
          name: "Christopher David",
          avatar: "https://avatars.githubusercontent.com/u/14167547?v=4",
          githubId: "14167547",
          githubUsername: "AtlantisPleb"
        }), {
          headers: { 'Content-Type': 'application/json' }
        });
      } catch (error) {
        return new Response('Invalid token', { status: 401 });
      }
    }

    // Handle /tokens endpoint to return provider tokens (GitHub access token)
    if (url.pathname === '/tokens' && request.method === 'GET') {
      try {
        const authHeader = request.headers.get('Authorization');
        if (!authHeader?.startsWith('Bearer ')) {
          return new Response('Unauthorized', { status: 401 });
        }
        
        const jwtToken = authHeader.slice(7);
        console.log(`🔍 [AUTH] /tokens endpoint called with JWT token`);
        
        // We need to decode the JWT to get the user's subject
        // For now, we'll use a hardcoded user sub for testing
        // TODO: Implement proper JWT decoding
        const userSub = "14167547"; // This should come from JWT decoding
        
        // Get provider tokens from storage
        const providerTokens = await getProviderTokens(storage, userSub);
        
        if (!providerTokens) {
          console.log(`⚠️ [AUTH] No provider tokens found for user ${userSub}`);
          return new Response(JSON.stringify({
            error: "no_tokens",
            message: "No provider tokens found for user"
          }), {
            status: 404,
            headers: { 'Content-Type': 'application/json' }
          });
        }
        
        console.log(`✅ [AUTH] Returning provider tokens for user ${userSub}`);
        return new Response(JSON.stringify(providerTokens), {
          headers: { 'Content-Type': 'application/json' }
        });
      } catch (error) {
        console.error(`❌ [AUTH] Error in /tokens endpoint:`, error);
        return new Response('Internal server error', { status: 500 });
      }
    }

    // Handle /token endpoint for OAuth code exchange
    if (url.pathname === '/token' && request.method === 'POST') {
      try {
        const formData = await request.formData();
        const clientId = formData.get('client_id') as string;
        const code = formData.get('code') as string;
        const redirectUri = formData.get('redirect_uri') as string;

        console.log(`🔄 [AUTH] Token exchange request - clientId: ${clientId}, redirectUri: ${redirectUri}`);

        if (!clientId || !code || !redirectUri) {
          return new Response(JSON.stringify({
            error: "invalid_request",
            error_description: "Missing required parameters: client_id, code, redirect_uri"
          }), {
            status: 400,
            headers: { 'Content-Type': 'application/json' }
          });
        }

        // Validate client and redirect URI
        const isAllowed = await (async (input: any, req: any) => {
          console.log("🔍 [AUTH] Allow check - clientID:", input.clientID, "redirectURI:", input.redirectURI);
          
          // Allow mobile app with custom URL scheme (GitHub OAuth App)
          if (input.clientID === 'Ov23lirHI1DWTzZ1zT1u' && 
              input.redirectURI === 'openagents://auth/callback') {
            console.log("✅ [AUTH] Allowing mobile client:", input.clientID, input.redirectURI);
            return true;
          }

          // Allow dashboard client (dashboard.openagents.com)
          if (input.clientID === 'dashboard' && 
              input.redirectURI === 'https://dashboard.openagents.com/api/callback') {
            console.log("✅ [AUTH] Allowing dashboard client:", input.clientID, input.redirectURI);
            return true;
          }

          // Allow desktop client (localhost callback with any port)
          if (input.clientID === 'desktop' && 
              input.redirectURI.startsWith('http://localhost:') && 
              input.redirectURI.includes('callback')) {
            console.log("✅ [AUTH] Allowing desktop client:", input.clientID, input.redirectURI);
            return true;
          }

          // Allow localhost redirects (for development)
          if (input.redirectURI.includes('localhost')) {
            console.log("✅ [AUTH] Allowing localhost redirect:", input.redirectURI);
            return true;
          }

          console.log("❌ [AUTH] Rejecting client:", input.clientID, input.redirectURI);
          return false;
        })({ clientID: clientId, redirectURI: redirectUri }, request);

        if (!isAllowed) {
          return new Response(JSON.stringify({
            error: "unauthorized_client",
            error_description: "Client not authorized for this redirect URI"
          }), {
            status: 400,
            headers: { 'Content-Type': 'application/json' }
          });
        }

        // Exchange code for GitHub access token
        const tokenResponse = await fetch('https://github.com/login/oauth/access_token', {
          method: 'POST',
          headers: {
            'Accept': 'application/json',
            'Content-Type': 'application/x-www-form-urlencoded',
          },
          body: new URLSearchParams({
            client_id: env.GITHUB_CLIENT_ID,
            client_secret: env.GITHUB_CLIENT_SECRET,
            code: code,
            redirect_uri: redirectUri,
          }).toString(),
        });

        if (!tokenResponse.ok) {
          console.error(`❌ [AUTH] GitHub token exchange failed: ${tokenResponse.status}`);
          return new Response(JSON.stringify({
            error: "server_error",
            error_description: "Failed to exchange code for access token"
          }), {
            status: 500,
            headers: { 'Content-Type': 'application/json' }
          });
        }

        const githubTokenData = await tokenResponse.json() as any;
        
        if (githubTokenData.error) {
          console.error(`❌ [AUTH] GitHub OAuth error:`, githubTokenData);
          return new Response(JSON.stringify({
            error: "invalid_grant",
            error_description: githubTokenData.error_description || "Invalid authorization code"
          }), {
            status: 400,
            headers: { 'Content-Type': 'application/json' }
          });
        }

        const githubAccessToken = githubTokenData.access_token;
        
        // Get user info from GitHub
        const userResponse = await fetch("https://api.github.com/user", {
          headers: {
            "Authorization": `Bearer ${githubAccessToken}`,
            "Accept": "application/vnd.github.v3+json",
            "User-Agent": "OpenAgents-Auth"
          }
        });

        if (!userResponse.ok) {
          console.error(`❌ [AUTH] Failed to fetch GitHub user: ${userResponse.status}`);
          return new Response(JSON.stringify({
            error: "server_error", 
            error_description: "Failed to fetch user information"
          }), {
            status: 500,
            headers: { 'Content-Type': 'application/json' }
          });
        }

        const githubUser = await userResponse.json() as any;
        console.log("✅ [AUTH] Successfully fetched GitHub user data for token exchange");

        // Create user object
        const user = await getUser({
          sub: githubUser.id.toString(),
          github_id: githubUser.id.toString(),
          email: githubUser.email,
          name: githubUser.name,
          avatar: githubUser.avatar_url,
          username: githubUser.login,
        });

        // Store provider tokens
        if (githubAccessToken) {
          const providerTokens = {
            github: {
              access_token: githubAccessToken,
              refresh_token: githubTokenData.refresh_token,
            }
          };
          
          try {
            await storeProviderTokens(storage, user.id, providerTokens);
            console.log(`✅ [AUTH] Successfully stored GitHub access token for user ${user.id}`);
          } catch (error) {
            console.error(`❌ [AUTH] Failed to store provider tokens for user ${user.id}:`, error);
          }
        }

        // Return access token (this would be a JWT in a real implementation)
        return new Response(JSON.stringify({
          access_token: githubAccessToken,
          token_type: "bearer",
          scope: githubTokenData.scope,
          user: user
        }), {
          headers: { 'Content-Type': 'application/json' }
        });

      } catch (error) {
        console.error(`❌ [AUTH] Error in /token endpoint:`, error);
        return new Response(JSON.stringify({
          error: "server_error",
          error_description: "Internal server error"
        }), {
          status: 500,
          headers: { 'Content-Type': 'application/json' }
        });
      }
    }
    
    const app = issuer({
      providers: {
        github: GithubProvider({
          clientID: (() => {
            const clientId = env.GITHUB_CLIENT_ID;
            if (!clientId) throw new Error("GITHUB_CLIENT_ID environment variable is required");
            return clientId;
          })(),
          clientSecret: (() => {
            const clientSecret = env.GITHUB_CLIENT_SECRET;
            if (!clientSecret) throw new Error("GITHUB_CLIENT_SECRET environment variable is required");
            return clientSecret;
          })(),
          scopes: ["user:email", "read:user", "repo"],
        }),
      },
      storage: CloudflareStorage({
        namespace: env.AUTH_STORAGE,
      }),
  subjects,
  allow: async (input, req) => {
    console.log("🔍 [AUTH] Allow check - clientID:", input.clientID, "redirectURI:", input.redirectURI);
    
    // Allow mobile app with custom URL scheme (GitHub OAuth App)
    if (input.clientID === 'Ov23lirHI1DWTzZ1zT1u' && 
        input.redirectURI === 'openagents://auth/callback') {
      console.log("✅ [AUTH] Allowing mobile client:", input.clientID, input.redirectURI);
      return true;
    }

    // Allow dashboard client (dashboard.openagents.com)
    if (input.clientID === 'dashboard' && 
        input.redirectURI === 'https://dashboard.openagents.com/api/callback') {
      console.log("✅ [AUTH] Allowing dashboard client:", input.clientID, input.redirectURI);
      return true;
    }

    // Allow desktop client (localhost callback with any port)
    if (input.clientID === 'desktop' && 
        input.redirectURI.startsWith('http://localhost:') && 
        input.redirectURI.includes('callback')) {
      console.log("✅ [AUTH] Allowing desktop client:", input.clientID, input.redirectURI);
      return true;
    }

    // Allow localhost redirects (for development)
    if (input.redirectURI.includes('localhost')) {
      console.log("✅ [AUTH] Allowing localhost redirect:", input.redirectURI);
      return true;
    }

    // Allow same subdomain redirects (default behavior for web clients)
    const hostname = req.headers.get('host') || req.headers.get('x-forwarded-host');
    if (hostname && input.redirectURI.includes(hostname)) {
      console.log("✅ [AUTH] Allowing subdomain redirect:", input.redirectURI, "for host:", hostname);
      return true;
    }

    console.log("❌ [AUTH] Rejecting client:", input.clientID, input.redirectURI);
    return false;
  },
  success: async (ctx, value, request) => {
    // Handle successful GitHub authentication
    if (value.provider === "github") {
      try {
        console.log("🔍 [AUTH] GitHub OAuth value:", JSON.stringify(value, null, 2));
        
        // Extract GitHub access token from tokenset
        const githubAccessToken = value.tokenset.access;
        const githubRefreshToken = value.tokenset.refresh;
        
        console.log(`🔑 [AUTH] GitHub tokens received - access: ${githubAccessToken ? 'YES' : 'NO'}, refresh: ${githubRefreshToken ? 'YES' : 'NO'}`);
        
        // Always fetch user data from GitHub API using the access token
        console.log("🔧 [AUTH] Fetching GitHub user data using access token");
        
        const userResponse = await fetch("https://api.github.com/user", {
          headers: {
            "Authorization": `Bearer ${githubAccessToken}`,
            "Accept": "application/vnd.github.v3+json",
            "User-Agent": "OpenAgents-Auth"
          }
        });
        
        if (!userResponse.ok) {
          throw new Error(`Failed to fetch GitHub user: ${userResponse.status}`);
        }
        
        const githubUser = await userResponse.json() as GitHubUser;
        console.log("🔍 [AUTH] Fetched GitHub user data:", JSON.stringify(githubUser, null, 2));
        
        const user = await getUser({
          sub: githubUser.id.toString(),
          github_id: githubUser.id.toString(),
          email: githubUser.email,
          name: githubUser.name,
          avatar: githubUser.avatar_url,
          username: githubUser.login,
        });

        // Store the GitHub access token for later retrieval
        if (githubAccessToken) {
          const providerTokens: ProviderTokens = {
            github: {
              access_token: githubAccessToken,
              refresh_token: githubRefreshToken,
            }
          };
          
          try {
            await storeProviderTokens(storage, user.id, providerTokens);
            console.log(`✅ [AUTH] Successfully stored GitHub access token for user ${user.id}`);
          } catch (error) {
            console.error(`❌ [AUTH] Failed to store provider tokens for user ${user.id}:`, error);
          }
        } else {
          console.warn(`⚠️ [AUTH] No GitHub access token to store for user ${user.id}`);
        }

        return ctx.subject("user", user);
      } catch (error) {
        console.error("Failed to create user from GitHub claims:", error);
        console.error("Available value:", JSON.stringify(value, null, 2));
        throw new Error("User creation failed");
      }
    }

    console.error("Authentication attempted with unsupported provider:", value.provider);
    throw new Error(`Unsupported provider: ${value.provider}`);
  },
    });

    return app.fetch(request, env, ctx);
  },
};
