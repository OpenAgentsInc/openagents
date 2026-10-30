// The API's Spark wallet (NIP-X402 openagents:spark-receiver:1): the one
// process that imports the Breez Spark SDK on the server, the way
// SparkHost.swift is the only file that does on iOS. It receives and
// never sends. inference.c asks it, over HTTP on Cloud Run with IAM-only
// access, for three things and nothing else:
//
//   POST /invoice {sats, memo, expiry}  -> {invoice, payment_hash, amount_msat, expiry, pay_to}
//   POST /paid    {invoice}             -> {state: "completed"|"pending"|"failed"|"none",
//                                           amount_msat, payment_hash, preimage}
//   GET  /rate                          -> {usd_per_btc, source}   (Breez's fiat rate, else mempool.space)
//   GET  /health                        -> {ok, balance_sat}
//
// Mainnet only, Bitcoin only: no cross-chain, stable-balance or real-time
// sync. The seed is SPARK_MNEMONIC (Secret Manager oa-spark-mnemonic), read
// once at start and never logged. The SDK's store is SQLite under /tmp:
// the wallet's state is the operators', so a new instance syncs it again.
"use strict";
const http = require("node:http");
const breez = require("@breeztech/breez-sdk-spark/nodejs");

const BREEZ_API_KEY = "MIIBfjCCATCgAwIBAgIHPYzgGw0A+zAFBgMrZXAwEDEOMAwGA1UEAxMFQnJlZXowHhcNMjQxMTI0MjIxOTMzWhcNMzQxMTIyMjIxOTMzWjA3MRkwFwYDVQQKExBPcGVuQWdlbnRzLCBJbmMuMRowGAYDVQQDExFDaHJpc3RvcGhlciBEYXZpZDAqMAUGAytlcAMhANCD9cvfIDwcoiDKKYdT9BunHLS2/OuKzV8NS0SzqV13o4GBMH8wDgYDVR0PAQH/BAQDAgWgMAwGA1UdEwEB/wQCMAAwHQYDVR0OBBYEFNo5o+5ea0sNMlW/75VgGJCv2AcJMB8GA1UdIwQYMBaAFN6q1pJW843ndJIW/Ey2ILJrKJhrMB8GA1UdEQQYMBaBFGNocmlzQG9wZW5hZ2VudHMuY29tMAUGAytlcANBABvQIfNsop0kGIk0bgO/2kPum5B5lv6pYaSBXz73G1RV+eZj/wuW88lNQoGwVER+rA9+kWWTaR/dpdi8AFwjxw0=";  // the committed key (AGENTS.md, Secrets)
const MAX_SATS = 1000000;
const MAX_BODY = 4096;

let sdk = null, opening = null, identity = "";
function open() {
  if (sdk) return Promise.resolve(sdk);
  if (opening) return opening;
  const words = process.env.SPARK_MNEMONIC || "";
  if (words.trim().split(/\s+/).length < 12) return Promise.reject(new Error("no seed"));
  const config = breez.defaultServerConfig("mainnet");
  config.apiKey = BREEZ_API_KEY;
  config.preferSparkOverLightning = false;
  delete config.crossChainConfig; delete config.stableBalanceConfig; delete config.realTimeSyncServerUrl;
  opening = breez.connect({ config, seed: { type: "mnemonic", mnemonic: words.trim() }, storageDir: process.env.SPARK_DIR || "/tmp/spark" })
    .then(async (s) => { sdk = s; identity = (await s.getInfo({})).identityPubkey; return s; })
    .finally(() => { opening = null; });
  return opening;
}

function reply(res, status, obj) {
  const b = JSON.stringify(obj, (k, v) => (typeof v === "bigint" ? Number(v) : v));
  res.writeHead(status, { "Content-Type": "application/json", "Cache-Control": "no-store", "Content-Length": Buffer.byteLength(b) });
  res.end(b);
}
function body(req) {
  return new Promise((ok, no) => {
    let n = 0; const parts = [];
    req.on("data", (c) => { n += c.length; if (n > MAX_BODY) { no(new Error("too big")); req.destroy(); } else parts.push(c); });
    req.on("end", () => { try { ok(JSON.parse(Buffer.concat(parts).toString("utf8") || "{}")); } catch (e) { no(e); } });
    req.on("error", no);
  });
}
const printable = (s) => typeof s === "string" && s.length <= 200 && /^[\x20-\x7e]*$/.test(s);

async function invoice(q) {
  const sats = q.sats, expiry = q.expiry;
  if (!Number.isSafeInteger(sats) || sats < 1 || sats > MAX_SATS) return [400, { error: "sats" }];
  if (!Number.isSafeInteger(expiry) || expiry < 60 || expiry > 3600) return [400, { error: "expiry" }];
  if (!printable(q.memo)) return [400, { error: "memo" }];
  const s = await open();
  const r = await s.receivePayment({ paymentMethod: { type: "bolt11Invoice", description: q.memo, amountSats: sats, expirySecs: expiry } });
  const inv = r.paymentRequest;
  const p = await s.parse(inv);
  if (p.type !== "bolt11Invoice" || !inv.startsWith("lnbc") || p.amountMsat !== sats * 1000 || !/^[0-9a-f]{64}$/.test(p.paymentHash)) return [502, { error: "the invoice is not what was asked" }];
  return [200, { invoice: inv, payment_hash: p.paymentHash, amount_msat: p.amountMsat, expiry: p.timestamp + p.expiry, pay_to: identity }];
}

async function paid(q) {
  if (typeof q.invoice !== "string" || !q.invoice.startsWith("lnbc") || q.invoice.length > 2000) return [400, { error: "invoice" }];
  const s = await open();
  await s.syncWallet({});
  for (let offset = 0; offset < 1000; offset += 100) {
    const page = (await s.listPayments({ typeFilter: ["receive"], offset, limit: 100, sortAscending: false })).payments;
    for (const p of page) {
      const d = p.details;
      if (!d || d.type !== "lightning" || d.invoice !== q.invoice) continue;
      const h = d.htlcDetails || {};
      return [200, { state: p.status, amount_msat: Number(p.amount) * 1000, payment_hash: h.paymentHash || "", preimage: p.status === "completed" ? (h.preimage || "") : "" }];
    }
    if (page.length < 100) break;
  }
  return [200, { state: "none" }];
}

// Breez's fiat rate first; when its service fails (it answered EOF from
// Cloud Run on 2026-10-30), mempool.space's public price.
const sane = (v) => typeof v === "number" && v > 1000 && v < 100000000;
async function rate() {
  try {
    const s = await open();
    const usd = (await s.listFiatRates()).rates.find((r) => r.coin === "USD");
    if (usd && sane(usd.value)) return [200, { usd_per_btc: Math.round(usd.value), source: "breez" }];
  } catch (e) { console.error("spark: breez rate:", (e && e.message || "").slice(0, 120)); }
  const r = await fetch("https://mempool.space/api/v1/prices", { signal: AbortSignal.timeout(5000) });
  const usd = r.ok ? (await r.json()).USD : 0;
  if (!sane(usd)) return [502, { error: "no USD rate" }];
  return [200, { usd_per_btc: Math.round(usd), source: "mempool.space" }];
}

http.createServer(async (req, res) => {
  try {
    let out;
    if (req.method === "GET" && req.url === "/health") { const s = await open(); out = [200, { ok: true, balance_sat: (await s.getInfo({})).balanceSats }]; }
    else if (req.method === "GET" && req.url === "/rate") out = await rate();
    else if (req.method === "POST" && req.url === "/invoice") out = await invoice(await body(req));
    else if (req.method === "POST" && req.url === "/paid") out = await paid(await body(req));
    else out = [404, { error: "not here" }];
    reply(res, out[0], out[1]);
  } catch (e) {
    console.error("spark:", e && e.message ? e.message.slice(0, 300) : "failed");   // never the seed: it is not in any message we make
    reply(res, 502, { error: "the wallet failed" });
  }
}).listen(Number(process.env.PORT || 8080), () => { console.log("spark: listening"); open().catch((e) => console.error("spark: open:", e.message)); });
