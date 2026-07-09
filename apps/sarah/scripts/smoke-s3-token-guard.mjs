#!/usr/bin/env node

import { spawn } from "node:child_process";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Self-contained by default: without an explicit SARAH_S3_SMOKE_BASE_URL the
// smoke spawns its own isolated server with the caps this scenario needs
// (active-session cap 1, daily cap 3, alert threshold 2) and a per-run alert
// file. A fresh process means fresh in-memory counters, so the smoke is
// idempotent — pointing it at a long-lived shared server exhausts the daily
// cap across reruns and is only correct for deployment-pointed runs with an
// explicit URL.
//
// Self-spawned servers default to SARAH_REALTIME_TOKEN_TEST_MODE=1 so the
// hardening oracle is offline-green without a Vercel AI Gateway key. Set
// SARAH_S3_SMOKE_LIVE_GATEWAY=1 (and AI_GATEWAY_API_KEY) to exercise the real
// mint path on the self-spawned server.
let serverChild = null;
let baseUrlRaw = process.env.SARAH_S3_SMOKE_BASE_URL;
let alertFile = process.env.SARAH_REALTIME_SPEND_ALERT_FILE;

if (!baseUrlRaw) {
  const port = Number(process.env.SARAH_S3_SMOKE_PORT ?? 8793);
  alertFile = alertFile ?? join(tmpdir(), `sarah-s3-alerts-${process.pid}.jsonl`);
  rmSync(alertFile, { force: true });
  const liveGateway = process.env.SARAH_S3_SMOKE_LIVE_GATEWAY === "1";
  serverChild = spawn("bun", ["src/server.ts"], {
    env: {
      ...process.env,
      SARAH_PORT: String(port),
      SARAH_REALTIME_MAX_ACTIVE_SESSIONS_PER_PROSPECT: "1",
      SARAH_REALTIME_DAILY_TOKEN_CAP: "3",
      // Ratio of daily cap (0–1) at which spend alerts fire; 0.5 ⇒ mint #2.
      SARAH_REALTIME_SPEND_ALERT_THRESHOLD: "0.5",
      SARAH_REALTIME_SPEND_ALERT_FILE: alertFile,
      // Prefer test mode unless live gateway is explicitly armed.
      SARAH_REALTIME_TOKEN_TEST_MODE: liveGateway
        ? process.env.SARAH_REALTIME_TOKEN_TEST_MODE ?? "0"
        : "1",
    },
    stdio: "ignore",
  });
  baseUrlRaw = `http://127.0.0.1:${port}/sarah`;
  const deadline = Date.now() + 15_000;
  for (;;) {
    try {
      const response = await fetch(`${baseUrlRaw}/`);
      if (response.status < 500) break;
    } catch {
      // Server not up yet.
    }
    if (Date.now() > deadline) {
      serverChild.kill();
      throw new Error("S-3 smoke server did not come up in time.");
    }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
}

const baseUrl = baseUrlRaw.replace(/\/+$/, "");
// Origin must be scheme+host(+port) only — not the /sarah path mount.
const defaultOrigin =
  process.env.SARAH_S3_SMOKE_ORIGIN ?? new URL(baseUrl).origin;
const scenario = process.env.SARAH_S3_SMOKE_SCENARIO ?? "session-daily";
const evidencePath = process.env.SARAH_S3_EVIDENCE_OUT;

function cookieFrom(response) {
  return response.headers.get("set-cookie")?.split(";")[0] ?? null;
}

async function tokenPost({ origin = defaultOrigin, cookie } = {}) {
  const headers = { "content-type": "application/json" };
  if (origin) headers.origin = origin;
  if (cookie) headers.cookie = cookie;

  const response = await fetch(`${baseUrl}/api/realtime/token`, {
    method: "POST",
    headers,
    body: JSON.stringify({ sessionConfig: {} }),
  });

  const text = await response.text();
  let body = text;
  try {
    body = JSON.parse(text);
  } catch {
    // Keep raw text.
  }

  return {
    status: response.status,
    body,
    retryAfter: response.headers.get("retry-after"),
    setCookie: cookieFrom(response),
  };
}

async function runRateScenario() {
  const first = await tokenPost();
  const second = await tokenPost({ cookie: first.setCookie });

  return {
    scenario,
    expectations: [
      "first same-origin request mints a token",
      "second same-prospect request is throttled by the configured prospect/IP rate cap",
    ],
    observations: { first, second },
    passed:
      first.status === 200 &&
      second.status === 429 &&
      second.body?.error?.code === "rate_limited",
  };
}

async function runSessionDailyScenario() {
  const noOrigin = await tokenPost({ origin: null });
  const first = await tokenPost();
  const sameProspect = await tokenPost({ cookie: first.setCookie });
  const secondProspect = await tokenPost();
  const thirdProspect = await tokenPost();
  const overDailyCap = await tokenPost();
  const alertLines =
    alertFile && existsSync(alertFile)
      ? readFileSync(alertFile, "utf8").trim().split("\n").filter(Boolean)
      : [];

  return {
    scenario,
    expectations: [
      "missing Origin is denied as CSRF protection",
      "first same-origin request mints a token and prospect cookie",
      "same-prospect concurrent request is denied by the active-session cap",
      "daily token cap denies additional new-prospect mints",
      "spend alert file receives at least one alert record",
    ],
    observations: {
      noOrigin,
      first,
      sameProspect,
      secondProspect,
      thirdProspect,
      overDailyCap,
      alertLines,
    },
    passed:
      noOrigin.status === 403 &&
      noOrigin.body?.error?.code === "csrf_origin_missing" &&
      first.status === 200 &&
      Boolean(first.setCookie) &&
      sameProspect.status === 429 &&
      sameProspect.body?.error?.code === "session_cap_exceeded" &&
      overDailyCap.status === 429 &&
      overDailyCap.body?.error?.code === "daily_cap_exceeded" &&
      alertLines.length > 0,
  };
}

// Never print or persist minted client secrets — even ephemeral ones.
function redactTokens(value) {
  if (Array.isArray(value)) return value.map(redactTokens);
  if (value && typeof value === "object") {
    const clean = {};
    for (const [key, entry] of Object.entries(value)) {
      clean[key] =
        key === "token" && typeof entry === "string"
          ? "[redacted-ephemeral-client-token]"
          : redactTokens(entry);
    }
    return clean;
  }
  return value;
}

let result;
try {
  result =
    scenario === "rate" ? await runRateScenario() : await runSessionDailyScenario();
} finally {
  serverChild?.kill();
}
result = redactTokens(result);

if (evidencePath) {
  writeFileSync(evidencePath, `${JSON.stringify(result, null, 2)}\n`);
}

console.log(JSON.stringify(result, null, 2));

if (!result.passed) {
  process.exitCode = 1;
}
