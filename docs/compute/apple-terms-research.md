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
