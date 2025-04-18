import { betterAuth } from "better-auth";
import { genericOAuth } from "better-auth/plugins";
import { env } from "cloudflare:workers";
import { D1Dialect } from 'kysely-d1';

const dialect = new D1Dialect({
  database: env.DB,
});

// Export the initialized auth instance with a more specific type
// This avoids the portability issue while preventing type conflicts
type BetterAuthInstance = {
  handler: (request: Request) => Promise<Response>;
  api: any; // Using 'any' for the api property to avoid type conflicts
};

export const auth: BetterAuthInstance = betterAuth({
  account: {
    accountLinking: {
      enabled: true,
      trustedProviders: ["github", "consentkeys"]
    }
  },

  database: {
    dialect,
    type: "sqlite",
  },

  // Email & Password Authentication
  emailAndPassword: {
    enabled: true,
    autoSignIn: true, // Auto sign in user after successful sign up
  },

  socialProviders: {
    github: {
      clientId: env.GITHUB_CLIENT_ID,
      clientSecret: env.GITHUB_CLIENT_SECRET,
    },
  },

  // Use the genericOAuth plugin for ConsentKeys instead of defining it as a social provider
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
  ],

  // Enable debug mode for more detailed logs
  debug: true,
});

// Helper function to create a loader that requires authentication
// and redirects to the homepage if not authenticated
export async function requireAuth(request: Request) {
  const result = await auth.api.getSession(request);
  
  if (!result || !result.session) {
    return { redirect: '/', authError: 'You must be logged in to access this page' };
  }
  
  return { session: result.session, user: result.user };
}