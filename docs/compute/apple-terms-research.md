Below is a concise, sourced briefing on where Apple’s rules *do* and *don’t* let you build an “agentic compute marketplace”—especially if workers run Apple’s on‑device Foundation Models—and the practical lines you can’t cross.

> **Quick take:** Using iPhones/iPads as paid background “worker nodes” (running other people’s jobs) will almost certainly run into App Store blocks—mainly background‑execution limits, the explicit ban on on‑device crypto‑style mining, and rules against executing downloaded code. A coordination‑only iOS app with compute off‑device (e.g., on a Mac or other hardware) is more plausible, but payments and UX need to be structured carefully to fit Apple’s purchase rules and background‑task constraints. Apple’s new **Foundation Models framework** has its own Acceptable Use policy you must follow. ([Apple Developer][1])

---

## The governing documents & what they say

### 1) **App Store Review Guidelines (ASRG)**

* **No “unrelated background processes” (explicitly names crypto mining).**
  Guideline **2.4.2**: Apps must be power‑efficient and “may not run unrelated background processes, such as cryptocurrency mining.” Even if your workload isn’t *crypto mining*, App Review often analogizes distributed compute-for-pay to this class—especially if it runs while the app isn’t in the foreground or drains battery. ([Apple Developer][1])
* **Strict limits on background execution.**
  **2.5.4**: Background services may be used only for their intended, whitelisted purposes (VoIP, audio, location, *short* task completion, local notifications, etc.). Arbitrary compute isn’t on the list. Apple’s WWDC’25 session reiterates that background runtime is *opportunistic/tightly managed*, should be “discrete, user‑initiated,” and “isn’t guaranteed.” There’s a new continued‑processing task, but it still starts from an explicit user action and presents system UI; it’s not a free pass for ongoing batch jobs. ([Apple Developer][1])
* **No executing downloaded code.**
  **2.5.2**: Apps “may not … execute code which introduces or changes features or functionality of the app.” If your worker downloads job code (Python/WASM/scripts) and executes it, that’s squarely in the danger zone. Education‑only carve‑outs don’t apply to a paid compute marketplace. ([Apple Developer][1])
* **Thin‑clients for cloud apps (in a remote‑desktop context) aren’t acceptable.**
  **4.2.7(e)** calls out “thin clients for cloud‑based apps” as “not appropriate.” This section is framed around remote‑desktop clients, but historically has been cited more broadly against apps that are *little more than* a storefront/remote shell for a cloud service. A coordination‑only app must still deliver native, app‑like value. ([Apple Developer][1])
* **Payments—when you must (and must not) use In‑App Purchase (IAP).**
  **3.1.1** requires IAP for unlocking features/content consumed *in the app*. **3.1.3(e)** *requires* **non‑IAP** methods (e.g., Apple Pay or card entry) for physical goods/services consumed *outside* the app. Apple also warns (3.1.3) not to steer users away from IAP inside the app except in limited storefront/entitlement cases. If buyers purchase compute that shows up as results *in your iOS app*, expect Apple to demand IAP; if they purchase compute used *elsewhere*, use non‑IAP. ([Apple Developer][1])
* **Cryptocurrency clause (useful as analogy).**
  **3.1.5(ii)**: “Apps may not mine for cryptocurrencies unless the processing is performed off device.” Even though your service may *not* be crypto, this gives you a clear precedent: Apple doesn’t want commodity compute markets that run on‑device. ([Apple Developer][1])

### 2) **Apple Developer Program License Agreement (DPLA) – June & Oct 2025**

* **Foundation Models framework and its Acceptable Use.**
  The DPLA explicitly defines Apple’s **Foundation Models framework** and binds you to its **Acceptable Use Requirements** (Section **3.3.8(I)**): “By accessing, prompting, or otherwise using the Foundation Models Framework … you agree to follow … the Acceptable Use Requirements.” ([Apple Developer][2])
* **Where to find the Acceptable Use page.**
  The DPLA points to **developer.apple.com/apple‑intelligence/acceptable‑use‑requirements‑for‑the‑foundation‑models‑framework** (also linked from Apple’s docs/news). ([Apple Developer][3])

### 3) **Foundation Models framework – Acceptable Use Requirements**

* Apple lists **prohibited uses**, including: violence, pornography, self‑harm, fraud, *regulated* healthcare/legal/financial services, attempts to identify training data, and generating academic textbooks/courseware. It also bans “circumvent[ing] any safety policies, guardrails, or restrictions integrated into the Foundation Models framework.” You can “expose” the framework to users, but you **must** keep these content/purpose limits. ([Apple Developer][4])

### 4) **Background‑task engineering guidance (WWDC’25)**

* Apple’s own session stresses: background execution is tightly managed, energy‑constrained, and should generally be **user‑initiated**, with progress UI and the ability to cancel; it’s explicitly *not* for ongoing maintenance or heavy compute. This undercuts the feasibility of passive, battery‑intensive worker modes on iOS. ([Apple Developer][5])

---

## What this means for an “agentic compute marketplace”

### The “dividing lines” that matter

1. **Foreground vs. background, and who benefits.**

   * *Allowed*: Short, **user‑initiated** jobs with visible progress, ideally while charging/Wi‑Fi, that directly benefit the device owner (e.g., the owner asked your app to summarize their own notes). ([Apple Developer][5])
   * *High‑risk / likely rejected*: Running other people’s jobs in the background for micropayments (battery/thermal drain; “unrelated background processes”; not an intended background mode). ([Apple Developer][1])

2. **Executing downloaded job code vs. running static models/pipelines.**

   * *Allowed with care*: Shipping a fixed on‑device model/engine (e.g., Apple’s Foundation Models framework, or a bundled Core ML model) and sending **data/prompts**.
   * *Not allowed*: Downloading/executing new code to change app functionality (e.g., Python/WASM tasklets), unless you fit narrow education carve‑outs—which a paid compute marketplace won’t. ([Apple Developer][1])

3. **Using Apple’s on‑device Foundation Model for resale compute.**

   * Apple **does not** expressly say “for end‑user only” in the Acceptable Use page, but the **spirit** of Apple Intelligence and the AUP is end‑user, on‑device features. If you turn the model into a networked paid worker for third parties, you still must:
     a) keep all AUP prohibitions (no regulated healthcare/legal/financial services jobs; no defeating guardrails; etc.), and
     b) respect the iOS background/power rules (which practically preclude “headless” workers). ([Apple Developer][4])

4. **Payments structure.**

   * If **buyers** pay for compute that is **experienced/consumed in‑app**, Apple will require **IAP** (3.1.1/3.1.3(g)).
   * If compute is **consumed outside the app** (e.g., you deliver results to a website/SaaS), you **must not** use IAP; use Apple Pay or card entry (3.1.3(e) and DPLA 3.3.9(C)). Don’t steer away from IAP inside the app except under Apple’s limited link entitlements. ([Apple Developer][1])

5. **Crypto mining precedent.**

   * Apple explicitly bans on‑device mining; only off‑device mining is allowed. App Review often uses this as an analogy—if your app commoditizes device compute (even non‑crypto), expect heavy scrutiny or rejection unless it is clearly for the device owner’s own benefit and within foreground/short‑task constraints. ([Apple Developer][1])

---

## So, can you “have the user provide compute externally” and keep the iOS app for coordination?

**Yes—this is the most viable path—**provided you design it as a **marketplace/companion** app, not a background worker:

**A. Make iOS a *coordinator*, not the worker**

* The actual job execution runs **off the iPhone/iPad**, e.g., on the user’s **Mac** (a separate app or menubar agent) or on third‑party hardware. iOS handles identity, job discovery, assignment, job status, and payouts. This avoids the iOS background‑execution wall and the “on‑device mining” analogy. ([Apple Developer][1])

**B. Keep it “app‑like,” not a thin remote shell**

* Provide native UX (job feeds, dashboards, progress, dispute flows) rather than a bare webview into a cloud console. Apple has long been skeptical of thin clients for cloud apps; give meaningful on‑device functionality. ([Apple Developer][1])

**C. Structure payments correctly**

* If customers **consume** results **in the iOS app**, be ready to offer **IAP** for the buyer side.
* If customers consume results **outside** the app, handle buyer payments via **Apple Pay / card entry** (not IAP). For **worker payouts**, you’re in standard marketplace/fintech territory (KYC/AML/money‑transmitter compliance outside Apple’s scope). ([Apple Developer][1])

**D. If you still want *some* on‑device compute on iOS**

* Restrict to **explicit, user‑initiated** tasks with system UI (continued‑processing), preferably **while charging**, with clear consent and cancellation. No long‑running/ambient jobs. Make it obvious that the task benefits the device owner (e.g., “Process *my* photos,” not “Render *someone else’s* batch”). ([Apple Developer][5])

**E. If you rely on Apple’s Foundation Models**

* Enforce the **Acceptable Use** list—no regulated legal/health/finance jobs; don’t try to bypass model safety; don’t generate/identify training data; and avoid academic‑publishing outputs. Build guardrails and auditing, because Apple’s terms require you to maintain “reasonable guardrails.” ([Apple Developer][4])

**F. Never execute downloaded job code on iOS**

* Jobs must be **data** for a fixed, embedded engine or the Foundation Models framework—not arbitrary code blobs. ([Apple Developer][1])

---

## Practical architecture patterns (from “risky” → “more approvable”)

1. **Risky (likely to be rejected):**
   iOS app that pays users to run third‑party jobs **on‑device** in the *background*, executing downloaded task code, with results sent to third parties. Violates 2.5.2/2.5.4/2.4.2 in spirit if not letter. ([Apple Developer][1])

2. **Borderline:**
   Foreground‑only iOS worker that accepts third‑party jobs but every run is explicitly started by the user, shows progress, and stops on backgrounding. Still likely to get tough questions about “who benefits” and power/thermal use; many real workloads will exceed Apple’s continued‑processing tolerance. ([Apple Developer][5])

3. **Most approvable (recommended):**

   * **iOS app = marketplace + wallet + control plane.**
     Workers run on **Mac** (or other non‑iOS hardware) using your native agent. The iOS app lets users list resources, accept jobs, track progress, and manage payouts.
   * If the buyer consumes results **outside** the iOS app, handle buyer payments with **Apple Pay/card**, not IAP; if results are delivered **in‑app**, support **IAP** for that flow. ([Apple Developer][1])

> **EU-only option:** If you want more distribution flexibility, Apple now supports alternative app marketplaces and web distribution **in the EU** under DMA rules (still notarized). This doesn’t override ASRG for App Store distribution elsewhere, but can alter your go‑to‑market. ([Apple Developer][6])

---

## Foundation Models–specific gotchas you must bake in

* **Scope of use:** Apple positions the framework for **on‑device, end‑user features** (summarization, extraction, classification, tool‑calling; not general chatbots). Design around those strengths; don’t present it as a general cloud alternative. ([Apple Developer][7])
* **AUP compliance:** Enforce filters against **regulated** health/legal/financial tasks, content safety, and “no guardrail circumvention.” Add policy screens and job‑type blocking in your marketplace. ([Apple Developer][4])

---

## A compliance checklist you can act on

1. **Background execution:** No ambient compute. Only **user‑initiated**, clearly beneficial tasks with progress UI; otherwise run workers off‑device. ([Apple Developer][5])
2. **No downloaded code:** Jobs are **data/prompts**, not code. Don’t interpret/execute new code paths on iOS. ([Apple Developer][1])
3. **Power/thermal safeguards:** Warn, throttle, and prefer charging/Wi‑Fi; respect Low Power Mode. ([Apple Developer][1])
4. **Payments:**

   * In‑app consumption ⇒ **IAP**;
   * Consumption outside the app ⇒ **non‑IAP** (Apple Pay/card). Avoid steering away from IAP inside the app except where Apple allows links. ([Apple Developer][1])
5. **Foundation Models AUP:** Block prohibited job categories; log policy checks; don’t try to bypass model safety. ([Apple Developer][4])
6. **Not a thin client:** Provide real native features (discovery, reputation, QA, dispute resolution, device controls), not just a remote console. ([Apple Developer][1])
7. **Crypto analogy:** Don’t frame the app as “earn money by lending your device’s compute.” If any crypto is involved, on‑device mining is out; cloud/off‑device only. ([Apple Developer][1])

---

## Source highlights (for your files)

* **ASRG 2.4.2** (power/background; explicit crypto‑mining example). ([Apple Developer][1])
* **ASRG 2.5.2** (no executing downloaded code). ([Apple Developer][1])
* **ASRG 2.5.4** (background services only for intended purposes). ([Apple Developer][1])
* **ASRG 3.1.1 / 3.1.3 / 3.1.5** (IAP vs. non‑IAP; crypto rules). ([Apple Developer][1])
* **DPLA §3.3.8(I)** (Foundation Models; must follow Acceptable Use). ([Apple Developer][2])
* **Acceptable Use Requirements (Foundation Models)** (prohibited uses and guardrails). ([Apple Developer][4])
* **WWDC’25 background talk** (user‑initiated, constrained background runtime). ([Apple Developer][5])

---

## Bottom line

* An **iOS‑first worker network** that sells the device owner’s compute cycles in the background is **not** a good fit for Apple’s rules.
* A **coordination‑only iOS app** with **off‑device workers** (Mac/PC/servers) is the realistic path. It still needs careful payment flows (IAP vs non‑IAP), policy enforcement for Foundation Models, and no downloaded code execution on iOS. ([Apple Developer][1])

If you want, I can sketch a reference architecture for: **(a)** a buyer app flow compliant with 3.1.* and **(b)** a seller companion app (Mac agent + iOS controller) that keeps all heavy compute off iOS while still letting sellers manage jobs and payouts on their phone.

[1]: https://developer.apple.com/app-store/review/guidelines/ "App Review Guidelines - Apple Developer"
[2]: https://developer.apple.com/support/downloads/terms/apple-developer-program/Apple-Developer-Program-License-Agreement-20250609-English.pdf "Apple Developer Program License Agreement"
[3]: https://developer.apple.com/support/terms/apple-developer-program-license-agreement/?utm_source=chatgpt.com "Apple Developer Program License Agreement"
[4]: https://developer.apple.com/apple-intelligence/acceptable-use-requirements-for-the-foundation-models-framework/?utm_source=chatgpt.com "Acceptable use requirements for the Foundation Models ..."
[5]: https://developer.apple.com/videos/play/wwdc2025/227/ "Finish tasks in the background - WWDC25 - Videos - Apple Developer"
[6]: https://developer.apple.com/support/dma-and-apps-in-the-eu/?utm_source=chatgpt.com "Update on apps distributed in the European Union - Support"
[7]: https://developer.apple.com/videos/play/wwdc2025/286/?utm_source=chatgpt.com "Meet the Foundation Models framework - WWDC25 - Videos"

---

> Thanks. Here's what I want:
> - OpenAgents mobile app: iOS, ChatGPT style app with chats and agent management, bitcoin wallet
> - OpenAgents desktop app: MacOS, makes local compute - via combination of Foundation Models, Swift MLX, and potentially other on-device compute like Ollama/llama.cpp; schedules relevant work and long-running agent tasks
> - Compute marketplace itself running on open protocols like Nostr (using e.g. Nostr NIP-90 data vending machines - https://nips.nostr.com/90) which our clients or others may contribute to
> - Open protocols enabling other clients to contribute to the same compute liquidity pool

> Now spec out the architecture.

---

Awesome—this is buildable. Below is a **concrete, end‑to‑end architecture** for OpenAgents that (1) uses iOS **only** for UX, agent mgmt, and wallet, (2) pushes **all provider compute** to macOS (and other non‑iOS nodes), and (3) plugs into an **open Nostr/NIP‑90** marketplace so anyone can add liquidity. I also call out the few App Store red lines you must hold to in the mobile surface.

---

## 0) Guardrails that shape the design (why the split is necessary)

* iOS apps **may not** run “unrelated background processes (e.g., crypto‑mining),” must be power‑efficient, and background modes are **strictly** limited—using an iPhone/iPad as a paid worker node will run afoul of these rules. ([Apple Developer][1])
* Apps **may not** download/execute new code to change functionality; “jobs” on iOS must be **data**, not executable tasklets. ([Apple Developer][1])
* If you sell **digital** services that are **consumed inside** your iOS app, expect to use **IAP**; do off‑app checkout for services consumed **outside** the app (e.g., jobs fulfilled on desktop/web). ([Apple Developer][1])
* Crypto wallets are allowed on iOS (as an **Organization** developer); on‑device **mining** is not. ([Apple Developer][1])
* Apple’s **Foundation Models** framework (the on‑device LLM behind Apple Intelligence) is available on **macOS, iOS, iPadOS, visionOS**—with an **Acceptable Use** policy you must enforce. ([Apple Developer][2])
* **DMA/EU alternative distribution** removes many *App Store commerce/content* checks, but **Notarization** will still reject software that harms device integrity (e.g., rapid battery drain). ([Apple Developer][3])

These constraints lead to the split: **iOS = controller & wallet; macOS (and others) = workers.**

---

## 1) High‑level system

```
+---------------------------+        +----------------------------+
|  OpenAgents Mobile (iOS)  |        |  OpenAgents Desktop (macOS)|
|  - Chat & agent mgmt      |        |  - Worker runtime & queue  |
|  - Nostr identity keys    |        |  - Model backends:         |
|  - Lightning/BTC wallet   |        |      • Foundation Models   |
|  - NO provider compute    |        |      • MLX / Swift-MLX     |
|                           |        |      • Ollama / llama.cpp  |
+-------------+-------------+        |  - Scheduler, bids, payouts|
              | Pairing & control    |  - NIP-90 SP implementation|
              v                      +-------------+--------------+
      (E2E over Nostr + TLS)                      |
                                                  | Nostr relays (pub/sub)
                                                  v
                                   +--------------+---------------+
                                   |   Open Marketplace (Nostr)   |
                                   |   - NIP-90 job requests      |
                                   |   - Job results & feedback   |
                                   |   - Zaps/BOLT11 (NIP-57)     |
                                   +--------------+---------------+
                                                  |
                                      Other clients & providers
                                      (anyone speaking NIP-90)
```

**Open protocols** anchor the marketplace:

* **NIP‑90** “Data Vending Machine” for job requests (`kind:5000–5999`), results (`6000–6999`), feedback (`7000`), and payment hints (`amount` including optional **BOLT11** invoice). ([NIPs][4])
* **NIP‑57** Lightning **zaps** for payment receipts/UX; use with BOLT11 invoices. ([NIPs][5])
* **NIP‑01** basic event model & relays (secp256k1 keys). ([NIPs][6])

---

## 2) OpenAgents Mobile (iOS app)

**Purpose:** UX + orchestration + wallet; **no provider compute**, no in‑app sale of compute credits.

**Key modules**

1. **Chat & Agent Manager**

   * Threads, tools, and agent configs (prompt templates, tools, routing rules).
   * Agent actions emit **NIP‑90 job requests** (when the user is a *buyer*), or control messages to the paired desktop when the user is a *provider*.
   * Local caches only; do **not** unlock paid content/features via crypto/wallet internally (keeps you out of 3.1.1 trouble). ([Apple Developer][1])

2. **Nostr Identity & Session**

   * Generate/store **secp256k1** keys in **Secure Enclave**; BECH32 (**npub/nsec**, NIP‑19).
   * Relay set management (user‑editable); ephemeral subscriptions for job state.
   * Pairing to desktop via QR (ECDH channel + device‑link event).

3. **Bitcoin/Lightning Wallet (non‑custodial)**

   * Either embedded wallet with LSP support or **connect external** wallet via LNURL/Lightning Address.
   * Display **zap receipts** for NIP‑57; pay **BOLT11** invoices referenced by job events (never use wallet to unlock iOS features). ([Apple Developer][1])
   * Comply with 3.1.5(i) (wallets allowed; org developer). ([Apple Developer][1])

4. **Compliance switches**

   * Foundation Models **AUP filters** for any prompts you originate (block regulated finance/health/legal, etc.).
   * **No background compute**; background activity only for notifications and short fetches (App Store 2.4.2 / 2.5.4). ([Apple Developer][1])

**Buyer UX (safe for App Store)**

* Create job → publish **NIP‑90 request** with **bid** (msats) and optional encrypted params to preferred SPs/relays.
* If job will be **paid inside iOS** **and** content/result is **consumed in‑app**, you’ll be steered to **IAP**—so **don’t**. Instead: deep link to **web/desktop** to complete purchase & view deliverables; iOS shows metadata, status, and pointers only. ([Apple Developer][1])

**Provider UX (safe for App Store)**

* iOS toggles provider availability, price floor, and policy—but **actual work runs on your paired Mac**. iOS displays progress and payout receipts. (No on‑device worker.)

---

## 3) OpenAgents Desktop (macOS worker)

**Purpose:** do the actual compute, schedule jobs, and publish Nostr results.

**Runtime layout**

* **Core services**

  * **Nostr engine:** subscribes to relays; filters `kind:5000–5999`; publishes `7000` feedback and `6000–6999` results. Implements NIP‑04 encryption for private params/output. ([NIPs][4])
  * **Scheduler:** bid/accept loop, queue, priorities, SLOs (latency vs. reward), preemption, power/thermal policy.
  * **Backends:**

    * **Foundation Models** (Swift) for on‑device LLM tasks—native to Apple platforms. ([Apple Developer][2])
    * **MLX / Swift‑MLX** for custom models, finetunes, and tensor ops. ([ML Explore][7])
    * **Ollama** (local HTTP) and/or **llama.cpp** (C API / server mode) for GGUF LLMs; watch network exposure (Auth/ACL). ([Ollama][8])
  * **Payments:** either (a) local Lightning node (LND/CLN) or (b) wallet connector (LNbits, Phoenixd, Breez SDK). The worker publishes **BOLT11** in `amount` tags or expects **zaps**; reconciles receipts to job IDs per NIP‑90 flow. ([NIPs][4])
  * **Policy & Safety:** AUP classifier + prompt/content filters before any **Foundation Models** usage; reject/flag disallowed categories.
  * **Observability:** Job ledger, model cache, token accounting, thermal telemetry; expose local web UI on `localhost` only.

**Distribution choices (macOS)**

* If you target the **Mac App Store**, remember extra MAS rules (no downloading code to change functionality; autolaunch requires consent). For maximal flexibility (plugins/backends), prefer **notarized direct distribution**. ([Apple Developer][1])

**Resource policy**

* Prefer **plugged in** + **Wi‑Fi**, with thermal throttles; user‑visible kill‑switch. (Also helps EU Notarization posture should you ship iOS workers in EU later.) ([Apple Developer][3])

---

## 4) Marketplace on Nostr (open by default)

**Protocol surface (NIP‑90)**

* **Job request** (`kind:5000–5999`): inputs via `i` tags (`url/event/job/text`), optional `bid` (msats), `output` MIME, optional `relays` to post results, optional encrypted params (NIP‑04). ([NIPs][4])
* **Result** (`kind:6000–6999`): includes `request` (echo), `i` (inputs), `p` (customer pubkey), and `amount` with optional **BOLT11**. Encrypted output mirrored to `content` when needed. ([NIPs][4])
* **Feedback** (`kind:7000`): `status` (`payment‑required`, `processing`, `error`, `success`, `partial`) and optional `amount`. ([NIPs][4])
* **Capability advertising** (NIP‑89) and relay metadata (NIP‑11) for discovery. ([NIPs][4])
* **Payments**: users can either pay the attached **BOLT11** or **zap** the event; providers should watch for both. ([NIPs][4])

**OpenAgents additions (non‑proprietary)**

* **Kind registry**: publish an OSS catalog mapping common tasks to NIP‑90 request kinds + `param` schema (e.g., `model`, `max_tokens`, `tool_choice`, `safety_profile_id`).
* **Reputation & proof‑of‑work**: optional signed “work receipts” summarizing turnaround and dispute rate; entirely carried as Nostr events so **other clients can reuse it**.

---

## 5) End‑to‑end flows

### A) Buyer asks an agent to “summarize this PDF”

1. iOS app packages a **NIP‑90 request** (`kind:50xx`) with `i:[["url","..."]]`, `bid:"5000"`, `output:"text/plain"`. (If privacy needed, encrypt params per NIP‑04.) ([NIPs][4])
2. Relays broadcast; multiple providers respond with `7000` feedback (`payment‑required` or `processing`). ([NIPs][4])
3. Buyer **pays**: opens **web/desktop** to fulfill **BOLT11** or zap; iOS displays status only (keeps you out of IAP scope). ([Apple Developer][1])
4. Provider posts `6000–6999` **result** (possibly encrypted). iOS shows **pointer/preview**; deep link to web/desktop for full artifact if you want to avoid in‑app consumption rules. ([Apple Developer][1])

### B) User lists their Mac to earn

1. On macOS, user sets **min bid**, model set, power policy; advertises capabilities via **NIP‑89**. ([NIPs][4])
2. Scheduler selects jobs, posts `7000 processing`; on completion, posts `6000–6999` with **BOLT11**. ([NIPs][4])
3. Lightning node receives payment; macOS reconciles to job; iOS wallet shows **zap receipt** for visibility. ([NIPs][5])

---

## 6) Data & trust

* **Identity:** All roles (buyer/provider) use **Nostr keys**; optionally map to **NIP‑05** names for usability.
* **Privacy:** Encrypt inputs/params/output as needed (NIP‑04). For regulated/PII, reject at submission (and never run via Foundation Models if it violates AUP).
* **Delivery verifiability:** Results reference the original request (`e` tag) and provider/buyer pubkeys; optional artifact hashes in tags to enable external storage and integrity checking. ([NIPs][4])

---

## 7) Backends and model selection (desktop)

* **Routing policy** (pluggable):

  * *Fast/private text tasks* → **Foundation Models** when available (low latency, on‑device). ([Apple Developer][2])
  * *Custom models/embeddings/tools* → **MLX** (Swift) or **llama.cpp**; use **Ollama** if you want a simple local HTTP runner and a model zoo, but **lock down the port**—many public Ollama servers have been found inadvertently exposed. ([Ollama][8])
* **Caching:** KV cache reuse within a job chain (NIP‑90 Appendix 1) and disk‑backed model cache. ([NIPs][4])

---

## 8) Interop so **other clients** can share the pool

* Publish an **OSS “OpenAgents Jobs SDK”** (Swift, TS, Rust) that:

  * Encodes/decodes **NIP‑90** events & **NIP‑57** zaps. ([NIPs][4])
  * Exposes a **capability registry** (NIP‑89) and **param schemas** for common jobs. ([NIPs][4])
  * Provides a **policy module** implementing Foundation Models AUP filters (so third‑party clients inherit the same safety baseline).

---

## 9) App Store‑safe product boundaries (summary)

* **iOS**

  * No provider compute; no “always‑on” background worker. ([Apple Developer][1])
  * Wallet is OK (org dev). **Never** use crypto/wallet to unlock iOS features or content (that’s an IAP violation). ([Apple Developer][1])
  * If your buyer flow results are **consumed inside iOS**, be prepared to build an **IAP** path; otherwise keep purchase & consumption **off‑app** (show status/links only). ([Apple Developer][1])
* **macOS**

  * Prefer notarized direct distribution to freely support new backends and plugins; if you go MAS, respect 2.4.5 constraints. ([Apple Developer][1])
* **EU expansion**

  * If you ever want iOS **workers**, consider EU‑only alt distribution + Notarization, and enforce strong power/thermal controls. ([Apple Developer][3])

---

## 10) Implementation backlog (pragmatic)

**Milestone 1 — Foundation**

* Nostr client lib + relay manager (multi‑relay, back‑pressure). (NIP‑01) ([NIPs][6])
* iOS: Chats/agents UI, Secure Enclave keys, read‑only job status, wallet viewer (no unlocks). ([Apple Developer][1])
* macOS: Worker with **Foundation Models** backend + **Ollama** adapter; Lightning connector + BOLT11 issuance; NIP‑90 request/result/feedback. ([Apple Developer][2])

**Milestone 2 — Marketplace fit**

* Bidding & pricing policy; reputation (as Nostr events).
* Encrypted params/output (NIP‑04) and job chaining (NIP‑90 Appendix 1). ([NIPs][4])
* Safety: Foundation Models **AUP filter pack** and job‑type allowlist.

**Milestone 3 — Open protocols**

* Publish “OpenAgents kind registry” + schemas; SDKs for Swift/TS/Rust.
* Interop tests with third‑party clients; public relays & mirror sets (NIP‑11). ([NIPs][9])

---

## 11) Why this satisfies your goals

* **Agentic UX on mobile** with **Bitcoin wallet**, without triggering App Store background/IAP landmines. ([Apple Developer][1])
* **Local compute liquidity** on **macOS** using Apple’s **Foundation Models**, **MLX**, and **Ollama/llama.cpp**—the scheduler picks the best backend per job. ([Apple Developer][2])
* **Open marketplace** thanks to **NIP‑90/NIP‑57**, so *any* client/worker can plug in and trade jobs for sats. ([NIPs][4])

If you want, I can turn this into a **repo skeleton** (iOS/macOS targets, Nostr client, NIP‑90 models, and a stub scheduler with Foundation Models + Ollama adapters), plus a **job kind schema** for your first three agent tasks.

[1]: https://developer.apple.com/app-store/review/guidelines/ "App Review Guidelines - Apple Developer"
[2]: https://developer.apple.com/videos/play/wwdc2025/286/?utm_source=chatgpt.com "Meet the Foundation Models framework - WWDC25 - Videos"
[3]: https://developer.apple.com/security/complying-with-the-dma.pdf?utm_source=chatgpt.com "Complying with the Digital Markets Act"
[4]: https://nips.nostr.com/90 "NIP90 - NIP-90 - Data Vending Machine"
[5]: https://nips.nostr.com/57?utm_source=chatgpt.com "NIP57 - NIP-57 - Lightning Zaps"
[6]: https://nips.nostr.com/1?utm_source=chatgpt.com "NIP-01 - Basic protocol flow description"
[7]: https://ml-explore.github.io/mlx/?utm_source=chatgpt.com "MLX 0.29.4 documentation"
[8]: https://ollama.com/?utm_source=chatgpt.com "Ollama"
[9]: https://nips.nostr.com/11?utm_source=chatgpt.com "NIP-11 - Relay Information Document"
