#!/usr/bin/env node

import { existsSync, readFileSync, writeFileSync } from "node:fs";

const baseUrl = (
  process.env.SARAH_S3_SMOKE_BASE_URL ?? "http://127.0.0.1:8790/sarah"
).replace(/\/+$/, "");
// Origin must be scheme+host(+port) only — not the /sarah path mount.
const defaultOrigin =
  process.env.SARAH_S3_SMOKE_ORIGIN ?? new URL(baseUrl).origin;
const scenario = process.env.SARAH_S3_SMOKE_SCENARIO ?? "session-daily";
const evidencePath = process.env.SARAH_S3_EVIDENCE_OUT;
const alertFile = process.env.SARAH_REALTIME_SPEND_ALERT_FILE;

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

const result =
  scenario === "rate" ? await runRateScenario() : await runSessionDailyScenario();

if (evidencePath) {
  writeFileSync(evidencePath, `${JSON.stringify(result, null, 2)}\n`);
}

console.log(JSON.stringify(result, null, 2));

if (!result.passed) {
  process.exitCode = 1;
}
