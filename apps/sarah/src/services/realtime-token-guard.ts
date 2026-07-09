
import { appendFile } from "node:fs/promises";

export type SarahRealtimeTokenErrorCode =
  | "csrf_origin_missing"
  | "csrf_origin_mismatch"
  | "rate_limited"
  | "session_cap_exceeded"
  | "daily_cap_exceeded";

export type SarahRealtimeTokenError = {
  code: SarahRealtimeTokenErrorCode;
  message: string;
  retryAfterMs?: number;
};

export type SarahRealtimeTokenGuardResult =
  | {
      ok: true;
      ip: string;
      prospectRef: string;
      setProspectCookie: boolean;
      activeSessionExpiresAt: string;
    }
  | {
      ok: false;
      status: 403 | 429;
      error: SarahRealtimeTokenError;
      prospectRef?: string;
      setProspectCookie?: boolean;
    };

type Bucket = {
  count: number;
  resetAt: number;
};

type ActiveSlot = {
  expiresAt: number;
};

const rateBuckets = new Map<string, Bucket>();
const activeSessions = new Map<string, ActiveSlot[]>();
const dailySpend = new Map<string, Bucket>();
const sentAlerts = new Set<string>();

function numberEnv(name: string, fallback: number) {
  const value = process.env[name];
  if (!value) return fallback;

  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallback;
}

function stringEnv(name: string) {
  const value = process.env[name];
  return value && value.trim().length > 0 ? value.trim() : null;
}

function dayKey(now: number) {
  return new Date(now).toISOString().slice(0, 10);
}

function nextUtcMidnight(now: number) {
  const date = new Date(now);
  date.setUTCHours(24, 0, 0, 0);
  return date.getTime();
}

function retryAfter(resetAt: number, now: number) {
  return Math.max(0, resetAt - now);
}

function requestOrigin(request: Request) {
  const origin = request.headers.get("origin");
  return origin && origin.trim().length > 0 ? origin.trim() : null;
}

function allowedOrigins(request: Request) {
  const configured = (process.env.SARAH_ALLOWED_ORIGINS ?? "")
    .split(",")
    .map((origin) => origin.trim())
    .filter(Boolean);

  if (configured.length > 0) return configured;

  return [new URL(request.url).origin];
}

function isAllowedOrigin(request: Request) {
  const origin = requestOrigin(request);
  if (!origin) {
    return {
      ok: false as const,
      error: {
        code: "csrf_origin_missing" as const,
        message:
          "Sarah refused to mint a realtime token because the request did not include an Origin header.",
      },
    };
  }

  if (!allowedOrigins(request).includes(origin)) {
    return {
      ok: false as const,
      error: {
        code: "csrf_origin_mismatch" as const,
        message:
          "Sarah refused to mint a realtime token because the request origin is not allowed.",
      },
    };
  }

  return { ok: true as const };
}

function requestIp(request: Request) {
  const forwarded = request.headers.get("x-forwarded-for");
  if (forwarded) {
    const first = forwarded.split(",")[0]?.trim();
    if (first) return first;
  }

  return (
    request.headers.get("x-real-ip") ??
    request.headers.get("cf-connecting-ip") ??
    "local"
  );
}

export const SARAH_PROSPECT_COOKIE = "sarah_prospect_ref";

export function readProspectRef(request: Request) {
  const cookie = request.headers.get("cookie");
  if (!cookie) return null;

  for (const entry of cookie.split(";")) {
    const [rawName, ...rawValue] = entry.trim().split("=");
    if (rawName === SARAH_PROSPECT_COOKIE) {
      const value = rawValue.join("=");
      return value && value.length <= 96 ? decodeURIComponent(value) : null;
    }
  }

  return null;
}

function takeRateLimit(key: string, limit: number, windowMs: number, now: number) {
  if (limit <= 0) {
    return { ok: false as const, retryAfterMs: windowMs };
  }

  const bucket = rateBuckets.get(key);
  if (!bucket || bucket.resetAt <= now) {
    rateBuckets.set(key, { count: 1, resetAt: now + windowMs });
    return { ok: true as const };
  }

  if (bucket.count >= limit) {
    return {
      ok: false as const,
      retryAfterMs: retryAfter(bucket.resetAt, now),
    };
  }

  bucket.count += 1;
  return { ok: true as const };
}

function takeActiveSlot(key: string, limit: number, ttlMs: number, now: number) {
  if (limit <= 0) {
    return { ok: false as const, retryAfterMs: ttlMs };
  }

  const current = (activeSessions.get(key) ?? []).filter(
    (slot) => slot.expiresAt > now,
  );

  if (current.length >= limit) {
    const soonest = Math.min(...current.map((slot) => slot.expiresAt));
    activeSessions.set(key, current);
    return { ok: false as const, retryAfterMs: retryAfter(soonest, now) };
  }

  const expiresAt = now + ttlMs;
  current.push({ expiresAt });
  activeSessions.set(key, current);
  return { ok: true as const, expiresAt };
}

function releaseActiveSlot(key: string, expiresAt: number) {
  const current = activeSessions.get(key);
  if (!current) return;

  activeSessions.set(
    key,
    current.filter((slot) => slot.expiresAt !== expiresAt),
  );
}

function dailyBucket(now: number) {
  const key = dayKey(now);
  const bucket = dailySpend.get(key);
  if (bucket && bucket.resetAt > now) return { key, bucket };

  const nextBucket = { count: 0, resetAt: nextUtcMidnight(now) };
  dailySpend.set(key, nextBucket);
  return { key, bucket: nextBucket };
}

function readConfig() {
  return {
    rateWindowMs: numberEnv("SARAH_REALTIME_RATE_WINDOW_MS", 60_000),
    maxPerIp: numberEnv("SARAH_REALTIME_MAX_TOKENS_PER_IP", 20),
    maxPerProspect: numberEnv("SARAH_REALTIME_MAX_TOKENS_PER_PROSPECT", 10),
    maxActivePerIp: numberEnv("SARAH_REALTIME_MAX_ACTIVE_SESSIONS_PER_IP", 5),
    maxActivePerProspect: numberEnv(
      "SARAH_REALTIME_MAX_ACTIVE_SESSIONS_PER_PROSPECT",
      2,
    ),
    sessionTtlMs: numberEnv("SARAH_REALTIME_SESSION_TTL_MS", 120_000),
    dailyTokenCap: numberEnv("SARAH_REALTIME_DAILY_TOKEN_CAP", 500),
    spendAlertThreshold: numberEnv(
      "SARAH_REALTIME_SPEND_ALERT_THRESHOLD",
      0.8,
    ),
    spendAlertFile: stringEnv("SARAH_REALTIME_SPEND_ALERT_FILE"),
    spendAlertWebhookUrl: stringEnv("SARAH_REALTIME_SPEND_ALERT_WEBHOOK_URL"),
  };
}

export function checkSarahRealtimeTokenRequest(
  request: Request,
  now = Date.now(),
): SarahRealtimeTokenGuardResult {
  const origin = isAllowedOrigin(request);
  if (!origin.ok) return { ok: false, status: 403, error: origin.error };

  const config = readConfig();
  const ip = requestIp(request);
  const existingProspectRef = readProspectRef(request);
  const prospectRef = existingProspectRef ?? crypto.randomUUID();
  const setProspectCookie = !existingProspectRef;

  const ipRate = takeRateLimit(
    `ip:${ip}`,
    config.maxPerIp,
    config.rateWindowMs,
    now,
  );
  if (!ipRate.ok) {
    return {
      ok: false,
      status: 429,
      prospectRef,
      setProspectCookie,
      error: {
        code: "rate_limited",
        message:
          "Sarah is receiving too many realtime token requests from this network. Please wait and try again.",
        retryAfterMs: ipRate.retryAfterMs,
      },
    };
  }

  const prospectRate = takeRateLimit(
    `prospect:${prospectRef}`,
    config.maxPerProspect,
    config.rateWindowMs,
    now,
  );
  if (!prospectRate.ok) {
    return {
      ok: false,
      status: 429,
      prospectRef,
      setProspectCookie,
      error: {
        code: "rate_limited",
        message:
          "Sarah is receiving too many realtime token requests for this prospect. Please wait and try again.",
        retryAfterMs: prospectRate.retryAfterMs,
      },
    };
  }

  const daily = dailyBucket(now);
  if (daily.bucket.count >= config.dailyTokenCap) {
    return {
      ok: false,
      status: 429,
      prospectRef,
      setProspectCookie,
      error: {
        code: "daily_cap_exceeded",
        message:
          "Sarah's realtime token budget is capped for today. Please try again later or contact OpenAgents.",
        retryAfterMs: retryAfter(daily.bucket.resetAt, now),
      },
    };
  }

  const ipSlot = takeActiveSlot(
    `ip:${ip}`,
    config.maxActivePerIp,
    config.sessionTtlMs,
    now,
  );
  if (!ipSlot.ok) {
    return {
      ok: false,
      status: 429,
      prospectRef,
      setProspectCookie,
      error: {
        code: "session_cap_exceeded",
        message:
          "Sarah already has the maximum number of active realtime sessions for this network. Please wait for one to expire.",
        retryAfterMs: ipSlot.retryAfterMs,
      },
    };
  }

  const prospectSlot = takeActiveSlot(
    `prospect:${prospectRef}`,
    config.maxActivePerProspect,
    config.sessionTtlMs,
    now,
  );
  if (!prospectSlot.ok) {
    releaseActiveSlot(`ip:${ip}`, ipSlot.expiresAt);
    return {
      ok: false,
      status: 429,
      prospectRef,
      setProspectCookie,
      error: {
        code: "session_cap_exceeded",
        message:
          "Sarah already has the maximum number of active realtime sessions for this prospect. Please wait for one to expire.",
        retryAfterMs: prospectSlot.retryAfterMs,
      },
    };
  }

  return {
    ok: true,
    ip,
    prospectRef,
    setProspectCookie,
    activeSessionExpiresAt: new Date(prospectSlot.expiresAt).toISOString(),
  };
}

export async function recordSarahRealtimeTokenMint(now = Date.now()) {
  const config = readConfig();
  const daily = dailyBucket(now);
  daily.bucket.count += 1;

  const threshold = Math.max(
    1,
    Math.ceil(config.dailyTokenCap * config.spendAlertThreshold),
  );
  const shouldAlert =
    daily.bucket.count >= threshold || daily.bucket.count >= config.dailyTokenCap;

  if (!shouldAlert) return;

  const alertKey = `${daily.key}:${daily.bucket.count >= config.dailyTokenCap ? "cap" : "threshold"}`;
  if (sentAlerts.has(alertKey)) return;
  sentAlerts.add(alertKey);

  const payload = {
    type: "sarah.realtime_token_spend_alert.v1",
    day: daily.key,
    tokenMints: daily.bucket.count,
    dailyTokenCap: config.dailyTokenCap,
    threshold,
    emittedAt: new Date(now).toISOString(),
  };

  if (config.spendAlertFile) {
    await appendFile(config.spendAlertFile, `${JSON.stringify(payload)}\n`);
  }

  if (config.spendAlertWebhookUrl) {
    await fetch(config.spendAlertWebhookUrl, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload),
    });
  }
}

export function sarahRealtimeTokenTestMode() {
  return process.env.SARAH_REALTIME_TOKEN_TEST_MODE === "1";
}

export function getSarahRealtimeTokenGuardSnapshot(now = Date.now()) {
  const config = readConfig();
  const daily = dailyBucket(now);

  return {
    schema: "sarah.realtime_token_guard_snapshot.v1" as const,
    day: daily.key,
    tokenMints: daily.bucket.count,
    dailyTokenCap: config.dailyTokenCap,
    spendAlertThreshold: config.spendAlertThreshold,
    rateWindowMs: config.rateWindowMs,
    maxPerIp: config.maxPerIp,
    maxPerProspect: config.maxPerProspect,
    maxActivePerIp: config.maxActivePerIp,
    maxActivePerProspect: config.maxActivePerProspect,
    sessionTtlMs: config.sessionTtlMs,
    resetAt: new Date(daily.bucket.resetAt).toISOString(),
    alertConfigured: Boolean(
      config.spendAlertFile || config.spendAlertWebhookUrl,
    ),
  };
}
