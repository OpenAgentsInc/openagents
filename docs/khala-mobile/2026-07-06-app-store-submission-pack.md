# Khala Mobile — App Store submission pack (MM-I2, #8491)

Date: 2026-07-06

Status: **submission-readiness documentation pass.** This drafts every
agent-doable artifact needed to file an App Store Connect (and, for parity,
Play Console) submission for the mobile-only MVP. It does not and cannot
itself create the ASC app record, upload a build, or file a submission — see
the "Owner-gated" table at the end and `~/work/NEEDS_OWNER.md`. Every copy
claim below is grounded in the current `khala_code.mobile_mvp.v1` promise
record's `safeCopy` (`apps/openagents.com/workers/api/src/product-promises.ts`)
and real, grepped app behavior — nothing here invents a capability the app
does not have.

## 1. Listing copy

App name, subtitle, and full description must not claim anything the promise
registry's `safeCopy` doesn't back. Per `khala_code.mobile_mvp.v1`
(state: `planned`), the app **exists and installs**, but the full straight
line (cloud-executed turn → credit drain → push on completion) is not yet
end-to-end proven on a real device from a store build. Listing copy for a
**first internal/TestFlight submission** must describe what is actually shipped
today, not the eventual vision:

- **App name:** Khala Code
- **Subtitle (≤30 chars for iOS):** "Code from your phone"
- **Promotional text (≤170 chars, iOS-only, editable without re-review):**
  "Sign in with GitHub, pick a repo, and let an agent work on it — from your
  phone. New accounts start with free credit."
- **Description (draft, safe-copy bounded):**

  > Khala Code is a mobile coding agent. Sign in with your GitHub account,
  > pick a repository, and describe what you want done — Khala runs the turn
  > on OpenAgents Cloud and streams updates back to your phone.
  >
  > - Sign in with GitHub (no desktop or separate account required).
  > - Pick any repo you have access to, or start a repo-less chat.
  > - New accounts start with free credit to try it.
  > - Configure which model backs your requests.
  > - Get notified when a turn finishes or needs your input.
  >
  > Khala Code is under active development. Some capabilities (in-app credit
  > purchases, full push-notification delivery) are still rolling out —
  > check openagents.com/docs/product-promises for the current, honest
  > status of every claim we make about this app.

  The last paragraph is a deliberate, standing "don't overclaim" clause
  mirroring this repo's product-promise discipline — keep it in the
  description until `khala_code.mobile_mvp.v1` reaches `yellow` or `green`.
- **Keywords (iOS, comma-separated, ≤100 chars):** `coding agent,github,ai
  code,dev tools,pair programming,remote coding`
- **Do not say** (per the promise's `unsafeCopy`): the app is "the fastest
  way to ship code," that purchases/credit packs are available today (IAP is
  postponed for the first MVP build — 2026-07-06 owner decision, audit
  §12), that push notifications are guaranteed to arrive (the Expo project id
  is unset — see `NEEDS_OWNER.md` — so push registration currently no-ops),
  or that cloud execution has unlimited/production-grade throughput.

## 2. Screenshot shot-list

No device screenshots were captured in this pass (this environment has no
signed-in test account with seeded credits — see §6). This is the exact
shot list to capture once one exists, in submission order (iOS needs at
minimum one 6.9" and one 6.5" or 5.5" set; Android needs at minimum a phone
set):

1. **Sign-in screen** ("Nexus Beam" redesign, commit `83ad352bd3`) — hero
   shot, shows "Sign in with GitHub" and the app's visual identity.
2. **Onboarding — welcome step** (`onboarding-flow.tsx`'s `WelcomeStep`) —
   shows the `$10` grant chip (`CreditsBalanceChip`) visible immediately
   after sign-in, matching the audit's "land signed in with $10 credit
   already granted (visible immediately)" straight-line claim.
3. **Onboarding — repo picker step** — shows a real repo list (redact any
   private repo names not owned by the demo account before publishing).
4. **Onboarding — task step** — shows the suggested-task chips and the free-
   text task field.
5. **Thread list** (`thread-list-screen.tsx`) — shows at least one active
   thread.
6. **Thread messages / live turn stream** (`thread-messages-screen.tsx`) —
   shows a turn in progress or completed, ideally with the model/lane
   picker visible in the composer.
7. **Settings — Credits section** — shows balance + "View history" +
   the (currently disabled) "Buy more credits" affordance, honestly labeled
   "coming soon."
8. **Settings — Models section** — shows the per-user model picker (#8484).
9. *(Android only, once real device push is proven)* a push-notification
   banner for "turn finished."

Each shot must be captured from a **real signed-in session** (not a mock/
stub render) per Apple's screenshot-accuracy requirement — do not stage
mock data that misrepresents shipped behavior.

## 3. Privacy nutrition label draft (App Store "App Privacy" + Play "Data
   safety" — same underlying facts, two different forms)

Grounded in a fresh grep of `clients/khala-mobile/src` for every network
call, native module, and stored field (2026-07-06, current `main`):

| Data category | Collected? | Linked to identity? | Used for | Notes |
|---|---|---|---|---|
| **GitHub identity** (GitHub user id, login, avatar via OAuth) | Yes | Yes | App functionality (sign-in, repo access) | Stored server-side per `apps/openagents.com` OpenAuth; the mobile app itself only ever holds a bearer session token in `expo-secure-store`, never a raw GitHub token. |
| **Email address** | Yes (from GitHub's `user:email` scope, server-side only) | Yes | App functionality, account identification | Not directly read/stored by the mobile client; server-side per the OpenAuth issuer. |
| **User content (chat messages, task descriptions, repo names/paths you choose to work in)** | Yes | Yes | App functionality (the core coding-agent product) | Synced via Khala Sync (`/api/sync/*`); this is the product itself, not incidental collection. |
| **Purchase history** | **Not applicable today.** IAP is postponed for the first MVP build (2026-07-06 owner decision); the server IAP rail (#8482) exists but is dormant, and no client purchase UI exists (`credits-history-screen.tsx` is transaction-history for the free grant only). Re-visit this row when IAP client work (#8481) resumes. |
| **Usage data / diagnostics** | **No third-party analytics or crash SDK exists.** `src/diagnostics/crash-reporting.ts` is a local, in-app error boundary with a pluggable reporter defaulting to a no-op (`noopKhalaCrashReporter`) — verified by grep: no `crashReporter` prop is ever passed at the app root (`src/app.tsx`), so nothing leaves the device today. Token/turn usage IS recorded server-side (`token_usage_events`) for billing purposes — that's "App functionality," not "Analytics." |
| **Device/push token** | Yes, once a user opts in | Yes (tied to the account) | App functionality (push notifications) | `push_device_tokens` server table (#8485); currently non-functional in practice because the Expo project id is unset (see `NEEDS_OWNER.md`) — registration no-ops with `project_id_missing` until that's linked. |
| **Precise/coarse location** | No | — | — | No location API imported anywhere in `clients/khala-mobile`. |
| **Contacts, photos, calendar** | No | — | — | Not requested; no corresponding Expo module installed. |
| **Microphone** | Requested (permission declared), **not functionally captured yet** | — | Planned: push-to-talk transcription | `NSMicrophoneUsageDescription`/`RECORD_AUDIO` are declared because the push-to-talk STT module exists, but per the QA swarm audit it is an "always-reject/always-unavailable shell" — no real audio capture ships yet. Declare the permission honestly (Apple requires disclosure for the *capability*, not just active use) but do not claim voice input works in listing copy. |
| **Advertising / tracking (IDFA, ad networks)** | No | — | — | No ad SDK anywhere in the tree. App Tracking Transparency prompt is NOT needed. |

**Recommended App Store "App Privacy" answers:** Contact Info (Email — linked,
app functionality), User Content (linked, app functionality), Identifiers
(Device ID for push — linked, app functionality). Everything else: **Data Not
Collected.** No "Data Used to Track You" categories apply (no ATT prompt
needed).

**Recommended Play "Data safety" answers:** mirror the above; Play's form
additionally asks about data *sharing* with third parties — answer "No" (the
only external processor today would be a payment processor, and IAP is
dormant).

## 4. Required-reason API declarations / privacy manifests

Apple requires an aggregated `PrivacyInfo.xcprivacy` manifest at the final
app-binary level, covering "required reason" API categories (UserDefaults,
file timestamps, system boot time, disk space, active keyboard). For an Expo
managed workflow using CocoaPods, **Xcode aggregates each pod's own
`PrivacyInfo.xcprivacy`** into the final app bundle automatically at archive
time — Expo's core SDK modules (secure-store, sqlite, notifications, updates,
crypto, device, haptics, linking, web-browser, auth-session) have shipped
their own manifests upstream since Expo SDK 50, so **no manual manifest
authoring is expected** for this dependency set. What IS an agent-doable,
concrete verification step (not done in this pass, since it requires a full
local iOS prebuild + archive, not just the Android emulator work done for
#8490): after the next `expo prebuild --platform ios` + a Release archive,
inspect the produced `.xcarchive`'s
`Products/Applications/KhalaCode.app/PrivacyInfo.xcprivacy` and confirm it
lists a required-reason entry for each API category actually exercised (most
likely: `NSPrivacyAccessedAPICategoryUserDefaults`, reason `CA92.1`, from one
of the storage-adjacent Expo modules). Track this as a concrete pre-submission
checklist item (§7) rather than assuming it is fine — "should be automatic"
is not the same as "verified."

## 5. Age rating draft

Recommended: **4+ (iOS) / Everyone (Android/Play)**. The app has no
user-generated public content surface, no chat with other users, no mature
content, no gambling, no unmoderated open web browsing beyond the OS-native
GitHub OAuth browser tab. It is a developer tool; content is limited to code
and the user's own task descriptions.

## 6. App Review notes

Draft text for the "Notes for Review" field:

> Khala Code is a coding-agent mobile app. Sign in with GitHub (OAuth,
> `repo` scope) to grant repo access, pick a repository, and describe a task
> — the agent executes the task on our cloud infrastructure and streams
> progress back to the thread.
>
> **Demo account:** [NEEDS_OWNER — provision a demo GitHub account and link
> it through Khala's normal sign-in flow; it will receive the standard $10
> signup grant automatically (#8478), no manual seeding required for that
> part]. **Credits beyond the automatic $10 grant are currently assigned
> manually by an OpenAgents team member through our internal Aiur admin
> console** (`aiur.openagents.com`, #8499-#8501, in progress) rather than
> in-app purchase — in-app purchases are intentionally postponed for this
> first submission. If App Review needs a balance top-up during review,
> contact [NEEDS_OWNER — reviewer contact email] and we will grant credit to
> the demo account within one business day.
>
> **What to expect during review:** sign-in, repo picking, and task
> composition are fully live. Cloud-executed turns and push notifications on
> completion are close to prod ready as of Codex Lane 0's landed org-executor
> spine (#8473) but the full dispatch/metering chain (#8474-#8479) is still
> being finished — a turn may complete slower than a production SLA, or in
> rare cases return a typed "not yet available" error rather than a result.
> This is expected; please retry or contact us if the demo account appears
> stuck.
>
> No in-app purchases exist in this build.

Mark clearly in ASC that IAP is NOT present in this build (there is no SKU
to configure) — this avoids an automatic "app declares IAP but none configured"
rejection.

## 7. Account deletion path

**Not yet built — an explicit, tracked compliance gap**, not something this
pass invents a shortcut for. Per Apple Guideline 5.1.1(v), an app that
supports account creation must let the user initiate deletion in-app. Current
state (verified 2026-07-06, unchanged since the #8483 audit): `AccountSection`
in `clients/khala-mobile/src/screens/settings-screen.tsx` offers only "Sign
out"; no `DELETE /api/mobile/account`-shaped route exists anywhere in
`apps/openagents.com/workers/api/src`. The plain-language policy this future
mechanism should honor was already drafted in #8483
(`docs/khala-code/2026-07-06-mobile-iap-store-compliance-checklist.md` §3) —
reproduced here for submission-pack completeness:

> Deleting your Khala account permanently removes your GitHub sign-in link,
> your chat threads and turn history, and your device's push notification
> registration. Any remaining credit balance is forfeited and is not
> refunded — credits are non-transferable and have no cash value.

**This must be built before a real App Store submission can pass review** —
recommend a follow-up implementation issue (new WS item: a
`DELETE /api/mobile/account` route in the main Worker plus a confirmation
screen in `AccountSection`) filed against the Worker/API-owning lane, since
it is out of this launch-ops lane's scope (`clients/khala-mobile` test/QA
surfaces + docs only) and touches auth/credits/sync data across the Worker.
Do not file a first real ASC submission until this lands, or file only to
**TestFlight internal testing** (Apple does not enforce 5.1.1(v) for internal
TestFlight builds the same way it does for App Review) as the interim rung —
see §8.

## 8. TestFlight external-testing group (staging rung)

Recommended sequence, cheapest-first:

1. **Internal TestFlight** (up to 100 App Store Connect team members, no
   review needed) — usable today once a build is archived and uploaded; the
   fastest way to get the app on a real device ahead of the account-deletion
   gap being closed.
2. **External TestFlight** (up to 10,000 testers, requires one Beta App
   Review — lighter-weight than a full App Store review, but Apple has
   started enforcing 5.1.1(v) here too for apps with account creation, so
   the account-deletion mechanism should land before this rung, not just
   before full App Store submission).
3. **Full App Store submission** — after (2) proves out, and after IAP
   status is finalized (postponed copy in place, no dangling SKU
   declarations) and account deletion ships.

## 9. Submission-readiness summary

| Area | Status |
|---|---|
| Listing copy (name/subtitle/description/keywords) | ✅ Drafted this pass, promise-gated |
| Screenshot shot-list | ✅ Drafted; ❌ no screenshots captured (needs a seeded demo account) |
| Privacy nutrition label (App Store + Play) | ✅ Drafted, grounded in a real grep pass |
| Required-reason API / privacy manifest | ✅ Documented expected auto-aggregation; ❌ not verified against a real archived `.xcarchive` (needs a full iOS prebuild+archive pass, out of this pass's Android-focused environment) |
| Age rating | ✅ Drafted (4+ / Everyone) |
| App Review notes | ✅ Drafted; demo account provisioning is owner-gated |
| Account deletion | ❌ **Compliance gap** — mechanism unbuilt (known since #8483); policy copy ready; recommend a follow-up Worker-side implementation issue before a real App Store (or external TestFlight) submission |
| TestFlight staging plan | ✅ Drafted (internal → external → full submission) |
| ASC app record / build upload / actual submission | ❌ **Owner-gated** — no ASC account access in this environment |

## 10. Owner-gated (see `~/work/NEEDS_OWNER.md`)

- Create/confirm the App Store Connect app record for
  `com.openagents.khala.mobile` (separate from the existing native `Khala`
  app, `com.openagents.khala`).
- Provision a demo GitHub account for App Review and share reviewer-contact
  email for a manual credit top-up request during review.
- Decide and execute the account-deletion follow-up implementation
  (owner call: build it now vs. gate submission on TestFlight-internal-only
  until it lands).
- Link an Expo project id (`expo.extra.eas.projectId`) so push notifications
  actually register — already tracked in `NEEDS_OWNER.md` from the push-lane
  work; repeated here because it affects the App Review notes' honesty.
- Perform the actual ASC metadata entry, screenshot upload (once captured),
  and first TestFlight/App Store submission.
