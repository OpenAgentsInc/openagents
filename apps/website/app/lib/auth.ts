import { betterAuth } from "better-auth";
// import { createClient } from "@libsql/client/web";
import { LibsqlDialect } from "@libsql/kysely-libsql";

// Properly access environment variables based on Cloudflare Workers context
function getEnv(key: string): string {
  // First check if we're in a Workers environment with 'env' context
  // @ts-ignore - Access potential Cloudflare env
  if (typeof globalThis.process === 'undefined' && typeof env !== 'undefined') {
    // @ts-ignore - Cloudflare Worker context
    return env[key] || '';
  }
  // Fallback to node process.env
  return process.env[key] || '';
}

// Create a Turso client that works in the Cloudflare Workers environment
function createTursoClient() {
  const url = getEnv('TURSO_URL') || getEnv('TURSO_DATABASE_URL');
  const authToken = getEnv('TURSO_AUTH_TOKEN');

  if (!url) {
    console.error('Missing Turso URL configuration');
    throw new Error('Missing Turso URL configuration');
  }

  // Configure the LibSQL dialect for Cloudflare Workers
  return new LibsqlDialect({
    url,
    authToken: authToken || undefined,
    // Use fetch API from global
    fetch: (url, options) => fetch(url, options),
  });
}

// Create the dialect instance
const dialect = createTursoClient();

export const auth = betterAuth({
  // Use the LibSQL dialect configured for Cloudflare Workers
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
      clientId: getEnv('GITHUB_CLIENT_ID'),
      clientSecret: getEnv('GITHUB_CLIENT_SECRET'),
    },
    google: {
      clientId: getEnv('GOOGLE_CLIENT_ID'),
      clientSecret: getEnv('GOOGLE_CLIENT_SECRET'),
    },
  },
})

// Type definition for session user (optional but recommended)
export type SessionUser = Awaited<ReturnType<typeof auth.api.getSession>>["user"];
