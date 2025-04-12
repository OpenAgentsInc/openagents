import { betterAuth } from "better-auth";
import { oidcProvider } from "better-auth/plugins";
import { LibsqlDialect } from "@libsql/kysely-libsql";
import { env } from "cloudflare:workers"

const dialect = new LibsqlDialect({
  url: env.TURSO_DATABASE_URL || "",
  authToken: env.TURSO_AUTH_TOKEN || "",
})

// Export the initialized auth instance with proper type definition
// Ensure we use the correct structure for methods
export const auth: ReturnType<typeof betterAuth> = betterAuth({
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

  // Enable debug mode for more detailed logs
  debug: true,
});
