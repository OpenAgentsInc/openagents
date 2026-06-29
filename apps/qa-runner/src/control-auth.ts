// Khala agent bearer-token auth for the QA control daemon (#6196).
//
// Every control endpoint requires a Khala agent token in `Authorization:
// Bearer <token>`. The daemon validates the presented token against a
// configured allowlist (env), constant-time, so a missing/invalid token is
// rejected with 401 BEFORE any run is dispatched.
//
// WHY an allowlist, not a live D1 lookup: the daemon runs on a machine with
// Chrome, not inside the Worker that owns the agent-token D1 table. A future
// armed deployment can swap `makeTokenVerifier` for a network verifier against
// the Worker; the interface (`verify(token) => AuthResult`) stays the same so
// the server code does not change. The default deterministic verifier is an
// env allowlist, which keeps the mock/test path self-contained (no network).
//
// Public-safe: this module NEVER logs or echoes a token. `AuthResult` carries
// only a boolean + a public-safe agent label.

import { timingSafeEqual } from "node:crypto";

export interface AuthResult {
  readonly ok: boolean;
  /** Public-safe label for the authenticated agent (never the token). */
  readonly agent?: string;
  /** Honest reason on failure (never echoes the presented token). */
  readonly reason?: string;
}

export interface TokenVerifier {
  readonly verify: (presented: string | null) => AuthResult;
}

/** Constant-time equality over two strings (length-safe). */
function safeEqual(a: string, b: string): boolean {
  const ab = Buffer.from(a, "utf8");
  const bb = Buffer.from(b, "utf8");
  if (ab.length !== bb.length) return false;
  return timingSafeEqual(ab, bb);
}

/** Extract a bearer token from an Authorization header value, or null. */
export function bearerFrom(header: string | null | undefined): string | null {
  if (!header) return null;
  const match = /^Bearer\s+(.+)$/i.exec(header.trim());
  return match && match[1] ? match[1].trim() : null;
}

export interface AllowlistEntry {
  /** The opaque Khala agent token (secret; never surfaced). */
  readonly token: string;
  /** Public-safe agent label surfaced on success. */
  readonly agent: string;
}

/**
 * Build a verifier from an allowlist of (token -> agent) entries. A presented
 * token must match one entry exactly (constant-time). The mock/test path uses a
 * fixed allowlist; a real deployment loads it from env.
 */
export function makeTokenVerifier(allowlist: ReadonlyArray<AllowlistEntry>): TokenVerifier {
  return {
    verify: (presented) => {
      if (!presented) return { ok: false, reason: "missing bearer token" };
      for (const entry of allowlist) {
        if (safeEqual(presented, entry.token)) {
          return { ok: true, agent: entry.agent };
        }
      }
      return { ok: false, reason: "invalid token" };
    },
  };
}

/**
 * Load the allowlist from env. `QA_CONTROL_TOKENS` is a comma-separated list of
 * `agent:token` pairs (the agent label is public-safe; the token is the secret).
 * Empty/absent => an empty allowlist, so EVERY request is rejected (fail
 * closed) — the daemon never runs open.
 */
export function allowlistFromEnv(
  env: Readonly<Record<string, string | undefined>> = process.env,
): ReadonlyArray<AllowlistEntry> {
  const raw = env.QA_CONTROL_TOKENS;
  if (!raw) return [];
  return raw
    .split(",")
    .map((pair) => pair.trim())
    .filter((pair) => pair.length > 0)
    .map((pair) => {
      const idx = pair.indexOf(":");
      if (idx <= 0) return { agent: "agent", token: pair };
      return { agent: pair.slice(0, idx), token: pair.slice(idx + 1) };
    });
}
