import { randomUUID } from "node:crypto";

export const DEFAULT_TELEMETRY_ENDPOINT =
  "https://openagents.com/api/telemetry/events";
const DEFAULT_TELEMETRY_TIMEOUT_MS = 2_000;

function timedSignal(timeoutMs = DEFAULT_TELEMETRY_TIMEOUT_MS) {
  if (typeof AbortSignal?.timeout === "function") {
    return {
      signal: AbortSignal.timeout(timeoutMs),
      dispose() {},
    };
  }

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  return {
    signal: controller.signal,
    dispose() {
      clearTimeout(timer);
    },
  };
}

function cleanString(value, fallback = null) {
  if (typeof value !== "string") {
    return fallback;
  }

  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : fallback;
}

function normalizeProperties(properties = {}) {
  try {
    return JSON.parse(JSON.stringify(properties ?? {}));
  } catch {
    return {};
  }
}

export function detectPackageInvoker(env = process.env) {
  const userAgent = cleanString(env.npm_config_user_agent);
  if (userAgent?.includes("bun/")) {
    return "bun";
  }
  if (userAgent?.includes("npm/")) {
    return "npm";
  }

  const execPath = cleanString(env.npm_execpath);
  if (execPath?.includes("bun")) {
    return "bun";
  }
  if (execPath?.includes("npm")) {
    return "npm";
  }

  return "unknown";
}

export function installSourceForTelemetry(installMethod, cached) {
  if (installMethod === "source_build") {
    return cached ? "cached_source_build" : "source_build";
  }

  return cached ? "cached_prebuilt" : "prebuilt";
}

export function telemetryFailureContext(error, fallbackStage = "unknown") {
  const cause = error?.cause ?? null;
  const stage = cleanString(error?.stage)?.toLowerCase().replace(/[^a-z0-9]+/g, "_");
  const code =
    cleanString(error?.code) ??
    cleanString(error?.errno) ??
    cleanString(cause?.code) ??
    cleanString(cause?.errno) ??
    (typeof error?.httpStatus === "number" ? `http_${error.httpStatus}` : null) ??
    "unknown";
  const message =
    (error instanceof Error ? error.message : String(error)).split("\n")[0] ??
    "unknown error";

  return {
    error_stage: stage || fallbackStage,
    error_code: code,
    error_message: message.slice(0, 240),
  };
}

export function createTelemetryClient({
  endpoint = process.env.OPENAGENTS_TELEMETRY_URL ?? DEFAULT_TELEMETRY_ENDPOINT,
  fetchImpl = globalThis.fetch,
  anonymousActorId = randomUUID(),
  sessionId = anonymousActorId,
  installId = anonymousActorId,
  appVersion = null,
  sourceSurface = "installer",
} = {}) {
  const pending = new Set();
  const enabled =
    typeof fetchImpl === "function" &&
    Boolean(cleanString(endpoint)) &&
    process.env.OPENAGENTS_DISABLE_TELEMETRY !== "1";

  async function post(payload) {
    if (!enabled) {
      return false;
    }

    const timeout = timedSignal();
    try {
      const response = await fetchImpl(endpoint, {
        method: "POST",
        headers: {
          accept: "application/json",
          "content-type": "application/json",
        },
        body: JSON.stringify(payload),
        signal: timeout.signal,
      });
      return response.ok;
    } catch {
      return false;
    } finally {
      timeout.dispose();
    }
  }

  return {
    endpoint,
    anonymousActorId,
    sessionId,
    installId,
    emit(eventName, properties = {}) {
      const promise = post({
        event_name: eventName,
        source_surface: sourceSurface,
        occurred_at: new Date().toISOString(),
        anonymous_actor_id: anonymousActorId,
        session_id: sessionId,
        install_id: installId,
        app_version: cleanString(appVersion),
        properties: normalizeProperties(properties),
      });
      pending.add(promise);
      promise.finally(() => {
        pending.delete(promise);
      });
      return promise;
    },
    async flush() {
      const current = [...pending];
      if (current.length === 0) {
        return;
      }
      await Promise.allSettled(current);
    },
  };
}
