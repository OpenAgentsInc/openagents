import { betterAuth } from "better-auth";
import Database from "better-sqlite3";

// In a production environment, you would use LibSQL for CF Workers compatibility
// import { LibsqlDialect } from "@libsql/kysely-libsql"; 
// const dialect = new LibsqlDialect({
//   url: process.env.TURSO_DATABASE_URL!,
//   authToken: process.env.TURSO_AUTH_TOKEN!,
// });

export const auth: ReturnType<typeof betterAuth> = betterAuth({
  // For development, using local SQLite
  database: new Database("./sqlite.db"),
  
  // For production with Cloudflare Workers, you'd use:
  // database: {
  //   dialect,
  //   type: "sqlite",
  // },
  
  // Email & Password Authentication
  emailAndPassword: {
    enabled: true,
    // autoSignIn: true, // Default: sign in user after successful sign up
  },
  
  // Social Providers (uncomment and configure as needed)
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
