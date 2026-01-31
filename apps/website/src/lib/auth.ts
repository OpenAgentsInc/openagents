/**
 * Better Auth for OpenAgents website.
 * Uses D1 (via Kysely + kysely-d1) when runtime.env.DB is available; otherwise URL-based or no DB.
 * See: https://docs.astro.build/en/guides/authentication/
 *      https://better-auth.com/docs/installation
 *      https://github.com/aidenwallis/kysely-d1
 */

import { betterAuth } from "better-auth";
import { kyselyAdapter } from "better-auth/adapters/kysely";
import { Kysely } from "kysely";
import { D1Dialect } from "kysely-d1";
import type { D1Database } from "@cloudflare/workers-types";

const basePath = "/api/auth";
const baseURL = import.meta.env.PUBLIC_SITE_URL ?? "https://openagents.com";
const secret =
  import.meta.env.BETTER_AUTH_SECRET ||
  (import.meta.env.DEV ? "dev-secret-min-32-chars-for-openagents-website" : undefined);

const trustedOrigins = [
  "https://openagents.com",
  "http://localhost:4321",
  "http://127.0.0.1:4321",
];

const socialProviders = {
  github: {
    clientId: import.meta.env.GITHUB_CLIENT_ID,
    clientSecret: import.meta.env.GITHUB_CLIENT_SECRET,
  },
};

/** Create Better Auth instance backed by Cloudflare D1 (Kysely + kysely-d1). Use in API route when runtime.env.DB is available. */
export function createAuth(db: D1Database) {
  const kysely = new Kysely({
    dialect: new D1Dialect({ database: db }),
  });
  return betterAuth({
    basePath,
    baseURL,
    secret,
    trustedOrigins,
    socialProviders,
    database: kyselyAdapter(kysely, {
      type: "sqlite",
      transaction: false, // D1 does not support transactions in kysely-d1
    }),
  });
}

/** Default auth (URL-based or no DB). Used when D1 binding is not available (e.g. dev without platform proxy). */
export const auth = betterAuth({
  basePath,
  baseURL,
  secret,
  trustedOrigins,
  socialProviders,
  database: getDatabaseConfig(),
});

function getDatabaseConfig() {
  const url = import.meta.env.BETTER_AUTH_DATABASE_URL;
  if (!url) {
    return undefined;
  }
  if (url.startsWith("libsql:") || url.startsWith("file:")) {
    return { type: "sqlite" as const, url };
  }
  return { type: "postgres" as const, url };
}
