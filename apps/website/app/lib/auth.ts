import { betterAuth } from "better-auth";
// Remove direct import of better-sqlite3
// import Database from "better-sqlite3"; 
import { LibsqlDialect } from "@libsql/kysely-libsql"; // Use LibSQL for CF Workers compatibility

// Configure the LibSQL dialect for Turso or local SQLite via env vars
const dialect = new LibsqlDialect({
  url: process.env.TURSO_DATABASE_URL!,
  authToken: process.env.TURSO_AUTH_TOKEN!,
});

export const auth: ReturnType<typeof betterAuth> = betterAuth({
  // For development, using local SQLite via LibSQL (requires wrangler dev --local or similar setup)
  // database: new Database("./sqlite.db"), 

  // Use the LibSQL dialect for both development (local) and production (Cloudflare)
  database: {
    dialect,
    type: "sqlite", // Kysely needs the base type hint
  },

  // Email & Password Authentication
  emailAndPassword: {
    enabled: true,
    // autoSignIn: true, // Default: sign in user after successful sign up
  },

  // Social Providers (configure via environment variables)
  socialProviders: {
    github: {
      clientId: process.env.GITHUB_CLIENT_ID || "",
      clientSecret: process.env.GITHUB_CLIENT_SECRET || "",
    },
    google: {
      clientId: process.env.GOOGLE_CLIENT_ID || "",
      clientSecret: process.env.GOOGLE_CLIENT_SECRET || "",
    },
  },
})

// Type definition for session user (optional but recommended)
export type SessionUser = Awaited<ReturnType<typeof auth.api.getSession>>["user"];
