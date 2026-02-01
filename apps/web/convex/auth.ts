import { createClient } from "@convex-dev/better-auth";
import { convex, crossDomain } from "@convex-dev/better-auth/plugins";
import type { GenericCtx } from "@convex-dev/better-auth/utils";
import { betterAuth } from "better-auth/minimal";
import type { BetterAuthOptions } from "better-auth/minimal";
import { components } from "./_generated/api";
import type { DataModel } from "./_generated/dataModel";
import { query } from "./_generated/server";
import authConfig from "./auth.config";

const siteUrl = process.env.SITE_URL;

export const authComponent = createClient<DataModel>(components.betterAuth);

/** Origins allowed for CORS (frontend that calls Convex auth). */
const extraOrigins = process.env.CONVEX_ALLOWED_ORIGINS
  ? process.env.CONVEX_ALLOWED_ORIGINS.split(",").map((o) => o.trim()).filter(Boolean)
  : [];
const trustedOrigins = [
  siteUrl,
  "http://localhost:4321",
  "http://127.0.0.1:4321",
  "https://web-ct8.pages.dev",
  "https://bfd1a6db.web-ct8.pages.dev",
  "https://web.openagents.workers.dev",
  ...extraOrigins,
].filter((o): o is string => Boolean(o));

export const createAuthOptions = (ctx: GenericCtx<DataModel>): BetterAuthOptions => ({
  baseURL: siteUrl,
  secret: process.env.BETTER_AUTH_SECRET,
  database: authComponent.adapter(ctx),
  emailAndPassword: {
    enabled: true,
  },
  trustedOrigins,
  plugins: [
    convex({ authConfig }),
    ...(siteUrl ? [crossDomain({ siteUrl })] : []),
  ],
});

export const createAuth = (ctx: GenericCtx<DataModel>) =>
  betterAuth(createAuthOptions(ctx));

export const getCurrentUser = query({
  args: {},
  handler: async (ctx) => {
    return authComponent.safeGetAuthUser(ctx);
  },
});
