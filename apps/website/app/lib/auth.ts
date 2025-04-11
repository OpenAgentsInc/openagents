import { betterAuth } from "better-auth";
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
      trustedProviders: ["github"]
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

  // Enable debug mode for more detailed logs
  debug: true,
});
