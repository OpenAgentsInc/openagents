/**
 * Better Auth catch-all handler.
 * Mounts the auth routes at /api/auth/*
 * When runtime.env.DB (D1) is available, uses createAuth(DB); otherwise uses default auth (URL-based or no DB).
 * See: https://docs.astro.build/en/guides/authentication/
 */

import type { APIRoute } from "astro";
import { auth, createAuth } from "../../../lib/auth";
import type { D1Database } from "@cloudflare/workers-types";

export const prerender = false;

export const ALL: APIRoute = async (ctx) => {
  const runtime = (ctx.locals as { runtime?: { env?: { DB?: D1Database } } }).runtime;
  const db = runtime?.env?.DB;
  const authInstance = db ? createAuth(db) : auth;
  return authInstance.handler(ctx.request);
};
