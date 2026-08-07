#!/usr/bin/env node

import { chromium } from "playwright";

const targetUrl =
  process.argv[2] ?? "http://127.0.0.1:8082/demo/?relay=ws%3A%2F%2F127.0.0.1%3A18080";
const relayParameter = new URL(targetUrl).searchParams.get("relay");
const expectedProviderPubkey = process.env["MARKET_DEMO_EXPECT_PROVIDER_PUBKEY"];
const projectApiRequests = [];
const relayInformationRequests = [];
const relayFrames = [];
const browserErrors = [];

if (relayParameter === null) {
  throw new Error("market demo proof requires an explicit relay query parameter");
}
const relayUrl = new URL(relayParameter).toString();
if (expectedProviderPubkey === undefined) {
  throw new Error("MARKET_DEMO_EXPECT_PROVIDER_PUBKEY is required");
}

const browser = await chromium.launch({
  headless: true,
  args: ["--enable-unsafe-webgpu", "--enable-features=SharedArrayBuffer"],
});

try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 } });
  page.on("console", (message) => {
    if (message.type() === "error") browserErrors.push(message.text());
  });
  page.on("pageerror", (error) => browserErrors.push(error.message));
  page.on("request", (request) => {
    const url = new URL(request.url());
    if (url.pathname.startsWith("/api/")) projectApiRequests.push(request.url());
    if (request.headers()["accept"] === "application/nostr+json") {
      relayInformationRequests.push(request.url());
    }
  });

  let resolveDiscovery;
  let rejectDiscovery;
  const discovery = new Promise((resolve, reject) => {
    resolveDiscovery = resolve;
    rejectDiscovery = reject;
  });
  const discoveryTimeout = setTimeout(
    () => rejectDiscovery(new Error("relay did not send discovery EOSE")),
    20_000,
  );

  let resolveSession;
  let rejectSession;
  const session = new Promise((resolve, reject) => {
    resolveSession = resolve;
    rejectSession = reject;
  });
  const sessionTimeout = setTimeout(
    () => rejectSession(new Error("signed no-spend session did not complete")),
    30_000,
  );

  page.on("websocket", (socket) => {
    if (socket.url() !== relayUrl) return;
    socket.on("framesent", (frame) => {
      relayFrames.push({ direction: "sent", ...frame });
    });
    socket.on("framereceived", (frame) => {
      relayFrames.push({ direction: "received", ...frame });
      if (typeof frame.payload !== "string") return;
      let parsed;
      try {
        parsed = JSON.parse(frame.payload);
      } catch {
        return;
      }
      if (parsed[0] === "EOSE" && parsed[1] === "market-heads-v1") {
        clearTimeout(discoveryTimeout);
        resolveDiscovery();
      }
      const receivedSessionWraps = relayFrames.filter((candidate) => {
        if (candidate.direction !== "received" || typeof candidate.payload !== "string") {
          return false;
        }
        const message = JSON.parse(candidate.payload);
        return (
          message[0] === "EVENT" &&
          message[1] === "market-no-spend-session-v1" &&
          message[2]?.kind === 1059
        );
      }).length;
      if (receivedSessionWraps >= 6) {
        clearTimeout(sessionTimeout);
        resolveSession();
      }
    });
  });

  const response = await page.goto(targetUrl, { waitUntil: "domcontentloaded" });
  if (response === null || !response.ok()) {
    throw new Error(`market demo failed to load: ${response?.status() ?? "no response"}`);
  }
  await discovery;

  const parsedFrames = relayFrames
    .filter((frame) => typeof frame.payload === "string")
    .map((frame) => ({ ...frame, message: JSON.parse(frame.payload) }));
  const discoveryRequest = parsedFrames.find(
    (frame) =>
      frame.direction === "sent" &&
      JSON.stringify(frame.message) ===
        JSON.stringify(["REQ", "market-heads-v1", { kinds: [39600, 39601], limit: 128 }]),
  );
  if (discoveryRequest === undefined) {
    throw new Error("browser sent no exact bounded discovery REQ");
  }

  const discoveryEvents = parsedFrames
    .filter(
      (frame) =>
        frame.direction === "received" &&
        frame.message[0] === "EVENT" &&
        frame.message[1] === "market-heads-v1",
    )
    .map((frame) => frame.message[2]);
  const provider = discoveryEvents.find(
    (event) =>
      event.kind === 39600 &&
      event.pubkey === expectedProviderPubkey &&
      event.tags.some((tag) => tag[0] === "d" && tag[1] === "immortal-no-spend"),
  );
  if (provider === undefined) {
    throw new Error("expected signed no-spend provider head did not arrive");
  }
  const providerAddress = `39600:${expectedProviderPubkey}:immortal-no-spend`;
  const offering = discoveryEvents.find(
    (event) =>
      event.kind === 39601 &&
      event.pubkey === expectedProviderPubkey &&
      event.tags.some((tag) => tag[0] === "d" && tag[1] === "immortal-no-spend-swaps") &&
      event.tags.some((tag) => tag[0] === "provider" && tag[1] === providerAddress),
  );
  if (offering === undefined) {
    throw new Error("expected signed no-spend offering head did not arrive");
  }

  await page.waitForTimeout(500);
  await page.mouse.click(410, 200);
  await session;
  await page.waitForFunction(
    () => document.title === "Swap Demo — zero-spend verified",
    undefined,
    { timeout: 5_000 },
  );

  const completedFrames = relayFrames
    .filter((frame) => typeof frame.payload === "string")
    .map((frame) => ({ ...frame, message: JSON.parse(frame.payload) }));
  const sessionRequest = completedFrames.find(
    (frame) =>
      frame.direction === "sent" &&
      frame.message[0] === "REQ" &&
      frame.message[1] === "market-no-spend-session-v1",
  );
  if (sessionRequest === undefined) {
    throw new Error("browser sent no bounded private-session REQ");
  }
  const sessionFilter = sessionRequest.message[2];
  if (
    JSON.stringify(sessionFilter.kinds) !== JSON.stringify([1059]) ||
    !Array.isArray(sessionFilter["#p"]) ||
    sessionFilter["#p"].length !== 1 ||
    sessionFilter.limit !== 128 ||
    "since" in sessionFilter
  ) {
    throw new Error("private-session REQ does not preserve NIP-59 timestamps");
  }
  const requesterPubkey = sessionFilter["#p"][0];
  const sentWraps = completedFrames.filter(
    (frame) =>
      frame.direction === "sent" &&
      frame.message[0] === "EVENT" &&
      frame.message[1]?.kind === 1059 &&
      frame.message[1]?.tags?.some((tag) => tag[0] === "p" && tag[1] === expectedProviderPubkey),
  );
  const receivedWraps = completedFrames.filter(
    (frame) =>
      frame.direction === "received" &&
      frame.message[0] === "EVENT" &&
      frame.message[1] === "market-no-spend-session-v1" &&
      frame.message[2]?.kind === 1059 &&
      frame.message[2]?.tags?.some((tag) => tag[0] === "p" && tag[1] === requesterPubkey),
  );
  if (sentWraps.length !== 5 || receivedWraps.length !== 6) {
    throw new Error(
      `signed session had ${sentWraps.length} requester wraps and ${receivedWraps.length} provider wraps`,
    );
  }
  const acceptedEventIds = new Set(
    completedFrames
      .filter(
        (frame) =>
          frame.direction === "received" && frame.message[0] === "OK" && frame.message[2] === true,
      )
      .map((frame) => frame.message[1]),
  );
  const unaccepted = sentWraps.filter((frame) => !acceptedEventIds.has(frame.message[1].id));
  if (unaccepted.length > 0) {
    throw new Error("relay did not accept every requester gift wrap");
  }
  if (relayInformationRequests.length === 0) {
    throw new Error("browser performed no NIP-11 relay discovery");
  }
  if (projectApiRequests.length > 0) {
    throw new Error(`market demo called an OpenAgents API: ${projectApiRequests.join(", ")}`);
  }
  if (browserErrors.length > 0) {
    throw new Error(`browser errors: ${browserErrors.join(" | ")}`);
  }

  process.stdout.write(
    `${JSON.stringify({
      targetUrl,
      relayUrl,
      providerEventId: provider.id,
      offeringEventId: offering.id,
      discoveryEoseObserved: true,
      requesterWraps: sentWraps.length,
      providerWraps: receivedWraps.length,
      acceptedRequesterWraps: sentWraps.length,
      uiCloseVerified: true,
      projectApiRequests: 0,
      browserErrors: 0,
    })}\n`,
  );
} finally {
  await browser.close();
}
