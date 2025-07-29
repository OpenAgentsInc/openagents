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
          scopes: ["user:email", "read:user"],
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
  success: async (ctx, value) => {
    // Handle successful GitHub authentication
    if (value.provider === "github") {
      try {
        console.log("🔍 [AUTH] GitHub OAuth value:", JSON.stringify(value, null, 2));
        console.log("🔍 [AUTH] GitHub claims:", JSON.stringify(value.claims, null, 2));
        
        // If claims are null, manually fetch user data from GitHub API
        let githubUser;
        if (!value.claims) {
          console.log("🔧 [AUTH] Claims are null, manually fetching GitHub user data");
          const accessToken = value.tokenset.access;
          
          const userResponse = await fetch("https://api.github.com/user", {
            headers: {
              "Authorization": `Bearer ${accessToken}`,
              "Accept": "application/vnd.github.v3+json",
              "User-Agent": "OpenAgents-Auth"
            }
          });
          
          if (!userResponse.ok) {
            throw new Error(`Failed to fetch GitHub user: ${userResponse.status}`);
          }
          
          githubUser = await userResponse.json();
          console.log("🔍 [AUTH] Fetched GitHub user data:", JSON.stringify(githubUser, null, 2));
        } else {
          githubUser = value.claims;
        }
        
        const user = await getUser({
          sub: githubUser.id?.toString() || value.sub,
          github_id: githubUser.id?.toString() || githubUser.sub,
          email: githubUser.email,
          name: githubUser.name,
          avatar: githubUser.avatar_url,
          username: githubUser.login,
        });

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
      cookie: {
        // Configure for subdomain sharing
        domain: env.COOKIE_DOMAIN || undefined,
        secure: true,
        httpOnly: true,
        sameSite: "lax",
      },
    });

    return app.fetch(request, env, ctx);
  },
};
