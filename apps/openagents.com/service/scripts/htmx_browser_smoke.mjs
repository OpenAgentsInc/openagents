#!/usr/bin/env node
import { chromium } from "@playwright/test";
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "../../../../");

const timeoutMs = parsePositiveInt(process.env.OA_BROWSER_SMOKE_TIMEOUT_MS, 20_000);
const baseUrl = normalizeBaseUrl(
  process.env.BASE_URL || process.env.OPENAGENTS_BASE_URL || "http://127.0.0.1:8787",
);
const accessToken = nonEmpty(process.env.OA_BROWSER_SMOKE_ACCESS_TOKEN);
const requireLoginFlow = parseBool(
  process.env.OA_BROWSER_SMOKE_REQUIRE_LOGIN_FLOW,
  accessToken ? false : true,
);
const loginEmail = process.env.OA_BROWSER_SMOKE_EMAIL || "htmx-smoke@openagents.com";
const loginCode = process.env.OA_BROWSER_SMOKE_CODE || "123456";
const headless = parseBool(process.env.OA_BROWSER_SMOKE_HEADLESS, true);
const artifactDir =
  nonEmpty(process.env.OA_BROWSER_SMOKE_ARTIFACT_DIR) ||
  path.join(
    repoRoot,
    "apps/openagents.com/service/docs/reports/htmx-browser-smoke",
    utcCompactTimestamp(),
  );

const summary = {
  startedAt: new Date().toISOString(),
  completedAt: null,
  baseUrl,
  requireLoginFlow,
  usingAccessToken: Boolean(accessToken),
  timeoutMs,
  artifactDir,
  steps: [],
  diagnostics: {
    consoleErrors: [],
    pageErrors: [],
  },
  outcome: "running",
  failure: null,
};

let browser = null;
let context = null;
let page = null;
let activeAccessToken = accessToken;

await fs.mkdir(artifactDir, { recursive: true });

try {
  browser = await chromium.launch({ headless });

  if (requireLoginFlow) {
    const loginContext = await browser.newContext({ baseURL: baseUrl });
    const loginPage = await loginContext.newPage();
    wireDiagnostics(loginPage, "login", summary);
    let loginChallengeId = null;

    await runStep(summary, "login-load-page", loginPage, async () => {
      await loginPage.goto("/login", { waitUntil: "domcontentloaded", timeout: timeoutMs });
      await waitForHtmx(loginPage, timeoutMs);
      await loginPage.locator("form[action='/login/email']").waitFor({
        state: "visible",
        timeout: timeoutMs,
      });
      // Force full-page form posts for login smoke. Local HTTP mode does not persist Secure cookies.
      await loginPage.evaluate(() => {
        for (const selector of ["form[action='/login/email']", "form[action='/login/verify']"]) {
          const form = document.querySelector(selector);
          if (!form) {
            continue;
          }
          form.removeAttribute("hx-post");
          form.removeAttribute("hx-target");
          form.removeAttribute("hx-swap");
          form.setAttribute("hx-boost", "false");
        }
      });
    });

    await runStep(summary, "login-send-code", loginPage, async () => {
      const sendResponse = await loginPage.request.post(`${baseUrl}/login/email`, {
        form: {
          email: loginEmail,
        },
        maxRedirects: 0,
      });
      if (![200, 302, 303, 307].includes(sendResponse.status())) {
        throw new Error(`unexpected /login/email status ${sendResponse.status()}`);
      }
    });

    await runStep(summary, "login-seed-challenge-id", loginPage, async () => {
      const challengeResponse = await loginPage.request.post(`${baseUrl}/api/auth/email`, {
        headers: {
          "content-type": "application/json",
        },
        data: {
          email: loginEmail,
        },
      });
      if (!challengeResponse.ok()) {
        throw new Error(`/api/auth/email returned status ${challengeResponse.status()}`);
      }
      const challengePayload = await challengeResponse.json();
      loginChallengeId = challengePayload?.challengeId || null;
      if (!loginChallengeId) {
        throw new Error("missing challengeId from /api/auth/email response");
      }
    });

    await runStep(summary, "login-verify-code", loginPage, async () => {
      if (!loginChallengeId) {
        throw new Error("challenge id was not initialized before login verify");
      }
      const verifyResponse = await loginPage.request.post(`${baseUrl}/login/verify`, {
        form: {
          code: loginCode,
          challenge_id: loginChallengeId,
        },
        maxRedirects: 0,
      });
      if (![200, 302, 303, 307].includes(verifyResponse.status())) {
        throw new Error(`unexpected /login/verify status ${verifyResponse.status()}`);
      }
    });

    if (!accessToken) {
      context = loginContext;
      page = loginPage;
    } else {
      await loginContext.close();
    }
  }

  if (!context) {
    context = await browser.newContext({
      baseURL: baseUrl,
      extraHTTPHeaders: accessToken
        ? {
            authorization: `Bearer ${accessToken}`,
          }
        : undefined,
    });
    page = await context.newPage();
    wireDiagnostics(page, "primary", summary);
  }

  if (!activeAccessToken) {
    await runStep(summary, "token-bootstrap", page, async () => {
      const challengeResponse = await page.request.post(`${baseUrl}/api/auth/email`, {
        headers: {
          "content-type": "application/json",
        },
        data: {
          email: loginEmail,
        },
      });
      if (!challengeResponse.ok()) {
        throw new Error(`/api/auth/email returned status ${challengeResponse.status()}`);
      }

      const challengePayload = await challengeResponse.json();
      const challengeId = challengePayload?.challengeId || null;
      if (!challengeId) {
        throw new Error("missing challengeId from /api/auth/email response");
      }

      const verifyResponse = await page.request.post(`${baseUrl}/api/auth/verify`, {
        headers: {
          "content-type": "application/json",
          "x-client": "openagents-browser-smoke",
        },
        data: {
          code: loginCode,
          challenge_id: challengeId,
          device_id: "browser:htmx-smoke",
        },
      });
      if (!verifyResponse.ok()) {
        throw new Error(`/api/auth/verify returned status ${verifyResponse.status()}`);
      }

      const verifyPayload = await verifyResponse.json();
      const token = verifyPayload?.token || null;
      if (!token) {
        throw new Error("missing token from /api/auth/verify response");
      }
      activeAccessToken = token;
      await context.setExtraHTTPHeaders({
        authorization: `Bearer ${token}`,
      });
    });
  }

  await runStep(summary, "session-bootstrap", page, async () => {
    await page.goto("/", { waitUntil: "domcontentloaded", timeout: timeoutMs });
    await waitForHtmx(page, timeoutMs);
    await page.locator("#chat-surface").waitFor({ state: "visible", timeout: timeoutMs });
    if (activeAccessToken) {
      const currentPath = new URL(page.url()).pathname;
      if (currentPath === "/login") {
        throw new Error("access token bootstrap failed: /login rendered instead of authenticated chat route");
      }
    }
  });

  let firstThreadId = "";
  let secondThreadId = "";
  let chatHistoryReady = true;

  await runStep(summary, "chat-create-thread-1", page, async () => {
    await page
      .locator("form[action='/chat/new'] button[type='submit']")
      .first()
      .click({ timeout: timeoutMs });
    await page.waitForURL(/\/chat\/thread_[^/?]+/, { timeout: timeoutMs });
    firstThreadId = threadIdFromUrl(page.url());
    if (!firstThreadId) {
      throw new Error(`expected first thread id in URL, got ${page.url()}`);
    }
    if (chatHistoryReady) {
      await waitForNoticeText(page, "#chat-status", "thread created", timeoutMs);
    }
    await ensureChatComposerVisible(page, timeoutMs);
  });

  const messageText = `htmx browser smoke message ${Date.now()}`;
  await runStep(summary, "chat-send-message", page, async () => {
    if (!activeAccessToken) {
      throw new Error("active access token missing before chat send step");
    }
    await page.fill("form.chat-send textarea[name='text']", messageText, {
      timeout: timeoutMs,
    });
    await page.locator("form.chat-send button[type='submit']").click({ timeout: timeoutMs });
    await waitForThreadMessageViaApi(
      page,
      baseUrl,
      activeAccessToken,
      firstThreadId,
      messageText,
      timeoutMs,
    );
  });

  await runStep(summary, "chat-create-thread-2", page, async () => {
    await page
      .locator("form[action='/chat/new'] button[type='submit']")
      .first()
      .click({ timeout: timeoutMs });
    await page.waitForURL(/\/chat\/thread_[^/?]+/, { timeout: timeoutMs });
    secondThreadId = threadIdFromUrl(page.url());
    if (!secondThreadId) {
      throw new Error(`expected second thread id in URL, got ${page.url()}`);
    }
    if (secondThreadId === firstThreadId) {
      if (!activeAccessToken) {
        throw new Error("expected second thread id to differ from first thread id");
      }
      const fallbackCreateResponse = await page.request.post(`${baseUrl}/chat/new`, {
        headers: {
          authorization: `Bearer ${activeAccessToken}`,
        },
        maxRedirects: 0,
      });
      if (![302, 303, 307].includes(fallbackCreateResponse.status())) {
        throw new Error(
          `fallback /chat/new returned unexpected status ${fallbackCreateResponse.status()}`,
        );
      }
      const fallbackLocation = fallbackCreateResponse.headers().location || "";
      const fallbackThreadId = threadIdFromLocation(fallbackLocation, baseUrl);
      if (!fallbackThreadId || fallbackThreadId === firstThreadId) {
        throw new Error("expected fallback second thread id to differ from first thread id");
      }
      secondThreadId = fallbackThreadId;
      chatHistoryReady = false;
      await page.goto(`/chat/${secondThreadId}`, {
        waitUntil: "domcontentloaded",
        timeout: timeoutMs,
      });
      await waitForHtmx(page, timeoutMs);
    }
    await ensureChatComposerVisible(page, timeoutMs);
  });

  await runStep(summary, "chat-history-back-forward", page, async () => {
    if (!chatHistoryReady) {
      return;
    }
    await page.goBack({ waitUntil: "domcontentloaded", timeout: timeoutMs });
    await page.waitForURL(
      (value) => {
        const id = threadIdFromUrl(value);
        return id.length > 0 && id === firstThreadId;
      },
      { timeout: timeoutMs },
    );

    await page.goForward({ waitUntil: "domcontentloaded", timeout: timeoutMs });
    await page.waitForURL(
      (value) => {
        const id = threadIdFromUrl(value);
        return id.length > 0 && id === secondThreadId;
      },
      { timeout: timeoutMs },
    );
  });

  const zoneName = `smoke-zone-${Date.now()}`;
  const shoutBody = `htmx browser smoke shout ${Date.now()}`;

  await runStep(summary, "feed-post-shout", page, async () => {
    await page.goto("/feed?zone=all", { waitUntil: "domcontentloaded", timeout: timeoutMs });
    await waitForHtmx(page, timeoutMs);

    await page.fill("form.feed-compose input[name='zone']", zoneName, { timeout: timeoutMs });
    await page.fill("form.feed-compose textarea[name='body']", shoutBody, { timeout: timeoutMs });
    await page.locator("form.feed-compose button[type='submit']").click({ timeout: timeoutMs });

    await waitForFeedBodyViaRequest(page, baseUrl, zoneName, shoutBody, timeoutMs);
  });

  await runStep(summary, "feed-zone-nav-history", page, async () => {
    await page.goto(`/feed?zone=${encodeURIComponent(zoneName)}`, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    await waitForHtmx(page, timeoutMs);

    await page.goto("/feed?zone=all", {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    await waitForHtmx(page, timeoutMs);

    await page.goBack({ waitUntil: "domcontentloaded", timeout: timeoutMs });
    await page.waitForURL(
      (value) => {
        const url = new URL(value);
        return url.pathname === "/feed" && url.searchParams.get("zone") === zoneName;
      },
      { timeout: timeoutMs },
    );
  });

  await runStep(summary, "settings-profile-update", page, async () => {
    if (!activeAccessToken) {
      throw new Error("active access token missing before settings update step");
    }
    await page.goto("/settings/profile", {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    await waitForHtmx(page, timeoutMs);

    const updatedName = `HTMX Smoke ${Date.now()}`;
    await page.fill("#settings-name", updatedName, { timeout: timeoutMs });
    await page
      .locator("form[action='/settings/profile/update'] button[type='submit']")
      .click({ timeout: timeoutMs });

    await waitForProfileNameViaApi(
      page,
      baseUrl,
      activeAccessToken,
      updatedName,
      timeoutMs,
    );
    await page.reload({ waitUntil: "domcontentloaded", timeout: timeoutMs });
    const currentName = await page.inputValue("#settings-name", { timeout: timeoutMs });
    if (currentName !== updatedName) {
      throw new Error(`settings profile update did not persist in UI (expected '${updatedName}', got '${currentName}')`);
    }
  });

  summary.outcome = "passed";
  console.log(`ok: HTMX browser smoke passed (${baseUrl})`);
  console.log(`artifacts: ${artifactDir}`);
} catch (error) {
  summary.outcome = "failed";
  summary.failure = {
    message: error instanceof Error ? error.message : String(error),
    stack: error instanceof Error ? error.stack || null : null,
  };

  if (page) {
    const failurePath = path.join(artifactDir, "final-failure.png");
    try {
      await page.screenshot({ path: failurePath, fullPage: true });
      console.error(`captured failure screenshot: ${failurePath}`);
    } catch (screenshotError) {
      console.error(
        `warning: failed to capture final screenshot (${String(screenshotError)})`,
      );
    }
  }

  if (
    summary.failure?.message &&
    summary.failure.message.toLowerCase().includes("executable doesn't exist")
  ) {
    console.error("hint: Playwright browser binaries are missing. Run: npx playwright install chromium");
  }

  console.error(`error: HTMX browser smoke failed (${baseUrl})`);
  console.error(`artifacts: ${artifactDir}`);
  process.exitCode = 1;
} finally {
  summary.completedAt = new Date().toISOString();

  const summaryPath = path.join(artifactDir, "summary.json");
  await fs.writeFile(summaryPath, `${JSON.stringify(summary, null, 2)}\n`, "utf8");

  if (context) {
    try {
      await context.close();
    } catch {
      // best effort
    }
  }
  if (browser) {
    try {
      await browser.close();
    } catch {
      // best effort
    }
  }
}


function normalizeBaseUrl(value) {
  const normalized = String(value || "").trim();
  if (!normalized) {
    throw new Error("BASE_URL/OPENAGENTS_BASE_URL must be set");
  }
  return normalized.endsWith("/") ? normalized.slice(0, -1) : normalized;
}

function parseBool(raw, fallback) {
  if (raw === undefined || raw === null || String(raw).trim() === "") {
    return fallback;
  }
  const normalized = String(raw).trim().toLowerCase();
  return normalized === "1" || normalized === "true" || normalized === "yes" || normalized === "on";
}

function parsePositiveInt(raw, fallback) {
  if (!raw) {
    return fallback;
  }
  const parsed = Number.parseInt(String(raw), 10);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    return fallback;
  }
  return parsed;
}

function nonEmpty(value) {
  const normalized = String(value || "").trim();
  return normalized.length > 0 ? normalized : null;
}

function utcCompactTimestamp() {
  return new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z");
}

function slug(value) {
  return String(value)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80);
}

async function runStep(summaryState, name, currentPage, fn) {
  const started = Date.now();
  try {
    await fn();
    summaryState.steps.push({
      name,
      status: "passed",
      durationMs: Date.now() - started,
    });
  } catch (error) {
    const screenshotPath = path.join(artifactDir, `${slug(name)}-failure.png`);
    try {
      await currentPage.screenshot({ path: screenshotPath, fullPage: true });
    } catch {
      // best effort
    }

    summaryState.steps.push({
      name,
      status: "failed",
      durationMs: Date.now() - started,
      message: error instanceof Error ? error.message : String(error),
      screenshot: screenshotPath,
    });

    throw error;
  }
}

async function waitForNoticeText(currentPage, selector, expectedText, timeout) {
  await currentPage.locator(selector).waitFor({ state: "visible", timeout });
  await currentPage.waitForFunction(
    ([targetSelector, text]) => {
      const element = document.querySelector(targetSelector);
      if (!element) {
        return false;
      }
      return (element.textContent || "")
        .toLowerCase()
        .includes(String(text).toLowerCase());
    },
    [selector, expectedText],
    { timeout },
  );
}

async function waitForHtmx(currentPage, timeout) {
  await currentPage.waitForFunction(
    () => typeof window !== "undefined" && typeof window.htmx === "object",
    undefined,
    { timeout },
  );
}

async function ensureChatComposerVisible(currentPage, timeout) {
  const composer = currentPage.locator("form.chat-send textarea[name='text']");
  try {
    await composer.waitFor({ state: "visible", timeout: Math.min(timeout, 1500) });
    return;
  } catch {
    await currentPage.reload({ waitUntil: "domcontentloaded", timeout });
    await waitForHtmx(currentPage, timeout);
    await composer.waitFor({ state: "visible", timeout });
  }
}

async function waitForThreadMessageViaApi(
  currentPage,
  baseUrlValue,
  token,
  threadId,
  expectedText,
  timeout,
) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const response = await currentPage.request.get(
      `${baseUrlValue}/api/runtime/threads/${threadId}/messages`,
      {
        headers: {
          authorization: `Bearer ${token}`,
        },
      },
    );
    if (response.ok()) {
      const payload = await response.json();
      const messages = payload?.data?.messages;
      if (
        Array.isArray(messages) &&
        messages.some(
          (message) =>
            message &&
            message.role === "user" &&
            typeof message.text === "string" &&
            message.text.includes(expectedText),
        )
      ) {
        return;
      }
    }
    await currentPage.waitForTimeout(250);
  }

  throw new Error(
    `timed out waiting for message '${expectedText}' in thread '${threadId}' via API`,
  );
}

async function waitForFeedBodyViaRequest(
  currentPage,
  baseUrlValue,
  zoneName,
  expectedBody,
  timeout,
) {
  const deadline = Date.now() + timeout;
  const route = `${baseUrlValue}/feed/fragments/main?zone=${encodeURIComponent(zoneName)}`;
  while (Date.now() < deadline) {
    const response = await currentPage.request.get(route);
    if (response.ok()) {
      const body = await response.text();
      if (body.includes(expectedBody)) {
        return;
      }
    }
    await currentPage.waitForTimeout(250);
  }

  throw new Error(
    `timed out waiting for feed shout '${expectedBody}' in zone '${zoneName}'`,
  );
}

async function waitForProfileNameViaApi(
  currentPage,
  baseUrlValue,
  token,
  expectedName,
  timeout,
) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const response = await currentPage.request.get(`${baseUrlValue}/api/settings/profile`, {
      headers: {
        authorization: `Bearer ${token}`,
      },
    });
    if (response.ok()) {
      const payload = await response.json();
      const observed = payload?.data?.name;
      if (observed === expectedName) {
        return;
      }
    }
    await currentPage.waitForTimeout(250);
  }

  throw new Error(`timed out waiting for profile name update '${expectedName}'`);
}

function threadIdFromUrl(urlValue) {
  const pathname = new URL(urlValue).pathname;
  const match = pathname.match(/^\/chat\/([^/]+)$/);
  return match ? match[1] : "";
}

function threadIdFromLocation(locationValue, baseUrlValue) {
  const normalized = String(locationValue || "").trim();
  if (!normalized) {
    return "";
  }
  const absolute = normalized.startsWith("http")
    ? normalized
    : `${baseUrlValue}${normalized.startsWith("/") ? "" : "/"}${normalized}`;
  return threadIdFromUrl(absolute);
}

function wireDiagnostics(currentPage, scope, summaryState) {
  currentPage.on("console", (msg) => {
    if (msg.type() !== "error") {
      return;
    }
    summaryState.diagnostics.consoleErrors.push({
      scope,
      type: msg.type(),
      text: msg.text(),
    });
  });

  currentPage.on("pageerror", (error) => {
    summaryState.diagnostics.pageErrors.push({
      scope,
      message: error.message,
      stack: error.stack || null,
    });
  });
}
