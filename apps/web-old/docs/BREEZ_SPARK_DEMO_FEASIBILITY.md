# Breez Spark Demo Replication: Feasibility Report

**Purpose:** Assess whether we can replicate the [Breez SDK Spark (Nodeless) demo](https://sdk-doc-spark.breez.technology/) — implemented as [Glow Web](https://glow-app.co) in the [glow-web](https://github.com/breez/breez-sdk-spark-example) codebase — inside the OpenAgents `apps/web` codebase.

**References:**
- Breez SDK Nodeless (Spark) docs: [https://sdk-doc-spark.breez.technology/](https://sdk-doc-spark.breez.technology/)
- Glow Web app (demo): [https://glow-app.co](https://glow-app.co) — repo: `breez-sdk-spark-example` (referred to locally as `glow-web`)
- OpenAgents web app: `apps/web` (Astro + Convex + Cloudflare Pages)

---

## 1. What the Breez Spark Demo Is

### 1.1 Breez SDK – Nodeless (Spark)

From the [Breez SDK Nodeless documentation](https://sdk-doc-spark.breez.technology/):

- **What it is:** A nodeless integration that provides a **self-custodial, end-to-end** solution for integrating Lightning (and Spark) payments.
- **Features:** Send/receive via Lightning address, LNURL-Pay, Bolt11, BTC address, Spark address (BTKN); on-chain interoperability; keys held only by users; multi-app/multi-device sync; payment persistency and restore; automatic claims; **WebAssembly support**; compatible with external signers.
- **Pricing:** Free for developers.

### 1.2 Glow Web (Demo App)

Glow Web is the reference implementation that shows how to use the Breez SDK in a **browser** via WebAssembly:

- **Stack:** React 18, TypeScript, Vite 4, Tailwind CSS.
- **SDK:** `@breeztech/breez-sdk-spark` (npm), loaded as WASM at app startup.
- **User flows:**
  - **Generate** a new wallet (bip39 mnemonic), **Restore** from mnemonic.
  - **Send** payments: Lightning address, LNURL-Pay, Bolt11, Bitcoin address, **Spark address**.
  - **Receive** payments: Lightning (Bolt11), Bitcoin address, **Spark address**, Lightning address (optional).
  - **Deposits:** Unclaimed deposits, claim, refund.
  - **Settings:** Fiat currencies, backup, logs.
- **Security (demo only):** Mnemonic stored in `localStorage`; not for production.
- **Deployment:** Vercel (e.g. glow-app.co, breez-sdk-spark-example.vercel.app).

---

## 2. Glow-Web Architecture (Relevant to Replication)

### 2.1 Key Directories and Files

| Path | Role |
|------|------|
| `src/services/wasmLoader.ts` | Initializes `@breeztech/breez-sdk-spark` WASM once at startup. |
| `src/services/WalletAPI.ts` | TypeScript interface for all wallet operations (lifecycle, send, receive, parse, events, storage, Lightning address, settings, fiat, logs). |
| `src/services/walletService.ts` | Implements `WalletAPI` by wrapping the Breez SDK (`connect()`, `receivePayment()`, `sendPayment()`, etc.). |
| `src/contexts/WalletContext.tsx` | React context exposing `WalletAPI` via `useWallet()`. |
| `src/features/send/` | Send flows: `SendPaymentDialog`, workflows for Bolt11, Lnurl, Bitcoin, **Spark** (`SparkWorkflow.tsx`). |
| `src/features/receive/` | Receive flows: `ReceivePaymentDialog`, amount panel, **SparkAddressDisplay**, **BitcoinAddressDisplay**, LightningAddressDisplay. |
| `src/pages/` | GeneratePage, RestorePage, WalletPage, GetRefundPage, BackupPage, SettingsPage, FiatCurrenciesPage. |
| `vite.config.ts` | WASM and browser environment configuration (see below). |

### 2.2 Build and Runtime Requirements (Glow-Web)

**Dependencies (package.json):**

- `@breeztech/breez-sdk-spark`: ^0.7.10 (WASM build).
- `bip39`: mnemonic generation.
- Vite plugins:
  - `vite-plugin-wasm`
  - `vite-plugin-top-level-await`
  - `vite-plugin-node-polyfills` (for crypto, etc. used by SDK).
- `optimizeDeps.exclude: ['@breeztech/breez-sdk-spark']` so Vite does not pre-bundle the WASM module.

**Vite config (vite.config.ts):**

- `wasm()`, `topLevelAwait()`, `nodePolyfills()`.
- **Dev server headers** (required for SharedArrayBuffer / cross-origin isolation):
  - `Cross-Origin-Embedder-Policy: require-corp`
  - `Cross-Origin-Opener-Policy: same-origin`
- `build.target: 'esnext'`.
- Path alias `@` → `/src`.

**Initialization sequence (main.tsx):**

1. Call `initWasm()` (loads and initializes the Breez SDK WASM module).
2. Then mount React (`<App />`). App then connects wallet (Generate/Restore) via `connect({ config, seed: { type: "mnemonic", mnemonic }, storageDir })`.

**Storage:** SDK uses a `storageDir` (e.g. `"spark-wallet-example"`). In the browser this typically backs onto IndexedDB or similar; the demo also persists mnemonic in localStorage for convenience (insecure).

---

## 3. OpenAgents apps/web Stack

- **Framework:** Astro 5, with React islands (`client:load`, etc.).
- **Backend:** Convex (queries, mutations, actions); Better Auth on Convex HTTP for user auth.
- **Hosting:** Cloudflare Pages (static + SSR via `@astrojs/cloudflare`).
- **Build:** Astro uses Vite under the hood; current `astro.config.mjs` only adds `tailwindcss` in `vite.plugins`.
- **No** existing WASM, node polyfills, or COOP/COEP configuration.

---

## 4. Can We Replicate the Breez Spark Demo in apps/web?

### 4.1 Summary

**Technically:** Yes, we can replicate a Breez Spark (WASM) wallet experience inside `apps/web`, **provided** we:

1. Add the same Vite plugins and build/runtime constraints (WASM, top-level await, node polyfills).
2. Set **COOP/COEP** headers so the page is cross-origin isolated (required for SharedArrayBuffer used by the SDK).
3. Isolate the wallet UI (and WASM init) so it runs in a context where those headers and WASM loading are guaranteed (e.g. a dedicated route or sub-app).

**Operationally:** Replication is a significant feature add: new dependencies, new security surface (mnemonic/keys), and deployment constraints (headers). It is **feasible** but non-trivial.

### 4.2 Requirement-by-Requirement

| Requirement | Glow-Web | apps/web today | Feasible? |
|-------------|----------|----------------|------------|
| **WASM load** | Vite + `vite-plugin-wasm` + `topLevelAwait` | Astro/Vite, no WASM plugins | Yes — add same Vite plugins in `astro.config.mjs` and exclude SDK from optimizeDeps. |
| **Node polyfills** | `vite-plugin-node-polyfills` | None | Yes — add plugin for crypto/buffer used by SDK. |
| **COOP/COEP** | Dev server headers | None | Yes — see below for Cloudflare Pages. |
| **React** | Full React app | React islands | Yes — wallet can be a React subtree (e.g. one or more islands) using a shared WalletProvider. |
| **Initialization order** | WASM init before React | N/A | Yes — WASM init can run in a top-level script or in a root React component that gates rendering of wallet UI. |
| **Storage** | IndexedDB + localStorage (demo) | N/A | Yes — browser storage is available; we’d define our own policy (e.g. no mnemonic in localStorage in any production path). |
| **Deployment** | Vercel | Cloudflare Pages | Yes — with header configuration (see below). |

### 4.3 COOP/COEP on Cloudflare Pages

- **Why:** The Breez SDK WASM module may use `SharedArrayBuffer`, which in modern browsers requires [cross-origin isolation](https://web.dev/articles/coop-coep) (COOP + COEP).
- **Glow-web:** Sets these in Vite’s `server.headers` for **dev**. For **production**, the host (e.g. Vercel) must send the same headers on the HTML (and/or relevant routes).
- **Cloudflare Pages:** You can set headers via:
  - **`_headers`** file in the build output (e.g. in `public/`), or
  - A **Pages Function** (e.g. `functions/[[path]].ts`) that adds headers to responses.
- **Recommendation:** Add a `_headers` file or a Function that sends:
  - `Cross-Origin-Opener-Policy: same-origin`
  - `Cross-Origin-Embedder-Policy: require-corp`
  for the route(s) that serve the wallet (e.g. `/wallet` or `/spark-demo`). If the whole site enables these, third-party scripts that don’t opt in to CORP may break; so scoping to a sub-path is safer.

### 4.4 Where the Wallet Could Live in apps/web

- **Option A – Same site, dedicated route:** e.g. `/wallet` or `/spark-demo`. One Astro page that loads a React root which: (1) runs WASM init, (2) renders the wallet UI (generate/restore, send, receive, Spark/Bitcoin/Lightning). COOP/COEP applied only to that route if possible.
- **Option B – Subpath with stricter headers:** Same as A but deploy or configure so only that subpath gets COOP/COEP (e.g. different `_headers` rules or a Function that matches path).
- **Option C – Separate app (subdomain or repo):** Keep glow-web (or a fork) as a separate Vite app deployed elsewhere; link from apps/web “Try Spark demo” → that URL. No changes to apps/web build or headers; replication is “by reference” not in-repo.

---

## 5. Scope Options

| Scope | Description | Effort | Risk |
|-------|-------------|--------|------|
| **Full replication** | Generate, Restore, Send (Bolt11, LNURL, Bitcoin, Spark), Receive (Lightning, Bitcoin, Spark), deposits, settings, fiat. Same feature set as Glow-Web inside apps/web. | High | High (key handling, headers, testing). |
| **Spark-only demo** | One route that only does: connect (restore/generate), receive via Spark address, send to Spark address. Minimal UI. | Medium | Medium (WASM + headers). |
| **Link-out** | No WASM in apps/web. Add a “Try Breez Spark demo” link to glow-app.co or our own deployed fork. Optional short copy in KB. | Low | Low. |
| **Embedded iframe** | Embed Glow (or our fork) in an iframe. Requires Glow to be served with COOP/COEP and possibly X-Frame-Options/CSP that allow embedding; cross-origin. | Low–Medium | Medium (iframe limitations, postMessage if we need interaction). |

---

## 6. Implementation Outline (If We Replicate In-Repo)

1. **Dependencies**
   - Add `@breeztech/breez-sdk-spark`, `bip39`, `vite-plugin-wasm`, `vite-plugin-top-level-await`, `vite-plugin-node-polyfills` (or Astro/Vite equivalents where applicable).

2. **Vite (Astro) config**
   - Register `vite-plugin-wasm`, `vite-plugin-top-level-await`, `vite-plugin-node-polyfills`.
   - Set `optimizeDeps.exclude: ['@breeztech/breez-sdk-spark']`.
   - Use `build.target: 'esnext'` if not already.
   - For **local dev**, set `server.headers` with COOP/COEP on the dev server (or document that wallet route must be tested with headers).

3. **Headers (production)**
   - Add `_headers` (or a Pages Function) so the wallet route returns COOP/COEP.

4. **App structure**
   - **WASM init:** One small module (e.g. `src/lib/breez-wasm.ts`) that calls the SDK’s init and is invoked once before any wallet UI (e.g. from a root layout for `/wallet` or from the wallet page’s script).
   - **Wallet API layer:** Port or adapt `WalletAPI` + `walletService` (or a minimal subset) into `apps/web` (e.g. under `src/lib/wallet/` or `src/services/`).
   - **React context:** `WalletProvider` + `useWallet()` for React islands that need the SDK.
   - **Pages:** e.g. `/wallet` (or `/spark-demo`) with Astro page that mounts a React “wallet app” (generate/restore, send/receive, Spark/Lightning/BTC).
   - **Security:** Do **not** store mnemonic in localStorage in any production path; document key-handling policy and consider read-only demo (e.g. restore-only with user-provided mnemonic) for first iteration.

5. **Testing**
   - Verify WASM loads and `connect()` succeeds (e.g. with testnet or a throwaway mnemonic).
   - Verify send/receive flows for at least one method (e.g. Spark address) in staging.
   - Run existing site tests and `npm run test:site` to ensure Convex/auth and rest of site still work.

---

## 7. Relationship to Existing OpenAgents Spark / Breez Docs

- **`docs/nostr/SPARK_AUDIT.md`** and **`crates/spark`** describe the **Rust** Breez SDK and in-repo Spark wallet (e.g. for Pylon/desktop). That is separate from the **browser WASM** SDK used by Glow-Web.
- **`apps/spark-api`** is a Cloudflare Worker that stubs balance/invoice/pay until “Breez SDK + KV adapter”; that backend is server-side, not the in-browser WASM demo.
- Replicating the Breez Spark **demo** in apps/web is therefore about the **front-end, in-browser wallet experience** (WASM), not about replacing the Rust crate or the Worker API.

---

## 8. Risks and Recommendations

**Risks:**

- **Security:** Any in-browser wallet that handles mnemonics/keys is sensitive. Glow-Web’s localStorage mnemonic is explicitly demo-only. We must define and implement a strict key-handling policy (e.g. never persist mnemonic, or use a dedicated secure UX).
- **Headers:** Enabling COOP/COEP site-wide can break third-party embeds or scripts; prefer scoping to the wallet route.
- **Bundle size and load:** WASM and polyfills increase payload and init time; consider lazy-loading the wallet route so the rest of the site stays fast.
- **Maintenance:** Tracking `@breeztech/breez-sdk-spark` releases and adapting to API changes.

**Recommendations:**

1. **Short term:** Add a **link-out** from apps/web (e.g. KB or “Try Spark” CTA) to [glow-app.co](https://glow-app.co) or to a deployed fork, and document in this doc that full in-repo replication is feasible but optional.
2. **If we want an in-repo demo:** Start with a **Spark-only, minimal route** (e.g. restore + receive Spark + send Spark) and COOP/COEP only on that route; expand later if needed.
3. **Do not** store mnemonics in localStorage in any production-facing path; treat the first in-repo version as a demo with clear disclaimers.

---

## 9. Conclusion

We **can** replicate the Breez Spark (Nodeless) demo in the OpenAgents `apps/web` codebase by:

- Adding the same Vite plugins and build settings used by Glow-Web (WASM, top-level await, node polyfills),
- Enabling COOP/COEP (at least for the wallet route) on Cloudflare Pages,
- Implementing a dedicated route and React-based wallet UI that uses `@breeztech/breez-sdk-spark` and a WalletAPI abstraction.

Full replication is **feasible** but non-trivial; a **link-out** or a **minimal Spark-only demo** are lower-risk first steps. This report can be used as the basis for an ADR or implementation plan if we decide to proceed with in-repo replication.
