import { betterAuth } from "better-auth";
import { genericOAuth } from "better-auth/plugins";
import { LibsqlDialect } from "@libsql/kysely-libsql";
import { env } from "cloudflare:workers"

const dialect = new LibsqlDialect({
  url: env.TURSO_DATABASE_URL || "",
  authToken: env.TURSO_AUTH_TOKEN || "",
})

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
