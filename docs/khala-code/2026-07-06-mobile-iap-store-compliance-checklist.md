# Khala Mobile IAP store compliance checklist (MM-E3, #8483)

Status: submission-readiness checklist for the mobile-only MVP's in-app
purchase (credit pack) rail. This is primarily a documentation and
copy-audit deliverable — App Review's actual verdict cannot be obtained from
this environment. Every finding below is grounded in a specific file/grep
result or a computed number backed by a tested pure function
(`apps/openagents.com/workers/api/src/inference/iap-margin-analysis.ts`),
not assumption.

Depends on (both still open as of this writing): #8481 (RevenueCat client,
HELD pending an owner-created account — see `NEEDS_OWNER.md`) and #8482
(server IAP rail, merged `b37bfef24f`). Several items below cannot reach
`done` until #8481 lands a real purchase UI; each such item says so
explicitly rather than being marked complete prematurely.

## 1. Apple App Review Guideline 3.1.1 — external-payment steering audit

**Rule:** an app offering purchasable digital content/services consumed
in-app (our spendable credit is exactly this) must sell it through Apple's
In-App Purchase system on iOS. Copy or links that steer a user to buy the
same thing outside the app (a website, a "cheaper on the web" message, a
disabled-but-labeled external-purchase button) are a rejection risk.

**Audit method:** grepped the current mobile app tree
(`clients/khala-mobile/src`) for every credit/purchase/payment-adjacent
screen and component.

**Findings (2026-07-06, current `main`):**

- `clients/khala-mobile/src/screens/credits-history-screen.tsx` — read-only
  transaction history. No purchase-adjacent copy, no external links.
- `clients/khala-mobile/src/screens/settings-screen.tsx`'s `CreditsSection`
  — shows balance, a "View history" button, and a **disabled**
  `KhalaButton disabled text="Buy more credits (coming soon)"`. Disabled,
  does nothing, names no external destination. **Compliant as-is.** This is
  the intended future insertion point for #8481's RevenueCat purchase
  sheet — when that lands, the button must open the in-app IAP sheet
  directly, never a URL/webview to a payment page.
- `clients/khala-mobile/src/components/credits-balance-chip.tsx` — display
  only, no purchase affordance.
- No `Linking.openURL(...)` call anywhere in the app points at a
  payment/checkout/pricing page (grepped `Linking\.` across the tree; the
  one call site, `settings-screen.tsx`'s `Linking.openSettings()`, opens
  the OS Settings app for notification permission management —
  unrelated to payments).
- **Stripe is confirmed web-only today**: no Stripe SDK, Stripe URL, or
  Stripe copy exists anywhere in `clients/khala-mobile`. The public web
  checkout (`apps/openagents.com` proper) is a separate surface App Review
  does not evaluate as part of this app's binary.

**Follow-up (blocked on #8481, not a gap in today's shipped code):** once
the RevenueCat purchase sheet lands, re-run this same grep pass against the
new screen/component before submission. The requirement to check for then:
the purchase sheet must present ONLY the store's native payment UI (via
`react-native-purchases`), never a fallback "or buy on the web" link
visible on iOS. A web fallback IS acceptable on Android (Google's policy is
less restrictive here), but the two platforms' UI must not share a single
code path that could leak the iOS build into showing it.

**Checklist:**
- [x] No external-payment steering copy/links exist in the app today (audited 2026-07-06).
- [ ] Re-audit once #8481's purchase sheet UI lands, before submission.
- [x] Stripe confirmed web-only, zero mobile-app references.

## 2. Restore purchases

**Rule:** Apple requires a way for a user to recover purchases without
paying again (historically framed around non-consumables/subscriptions,
but App Review commonly expects SOME restore affordance in any app selling
IAP, including consumables, as a safety net against a purchase that
succeeded at the store but never got acknowledged app-side).

**Server-side status (verified, #8482):** the fulfillment path
(`fulfillIapCreditPackPurchase` in `inference/iap-credit-pack-payments.ts`)
is idempotent per `store_transaction_id` (`UNIQUE` constraint,
`readIapPurchaseByStoreTransactionId` short-circuits a second delivery).
This means: if RevenueCat's SDK-side `restorePurchases()` call re-surfaces
an unfinished/unacknowledged consumable transaction and RevenueCat resends
(or the SDK re-triggers) the SAME purchase event, our webhook naturally
either (a) fulfills it for the first time if it was never fulfilled — the
exact safety-net case restore exists for — or (b) no-ops if already
fulfilled. **No separate `/restore` endpoint is needed server-side; the
existing idempotent webhook path already covers it.** Verified by the
replay tests in `iap-credit-pack-payments.test.ts` and
`iap-webhook-routes.test.ts` (both pass as of commit `b37bfef24f`).

**Client-side status:** NOT YET BUILT. A "Restore Purchases" action
(calling RevenueCat SDK's `restorePurchases()`) is part of #8481's scope,
which is HELD. This is an honest, explicitly-tracked gap, not something
silently skipped — #8481 must include this action (a button in Settings
near the purchase sheet is the natural placement, matching
`CreditsSection`'s existing layout) before submission.

**Checklist:**
- [x] Server-side fulfillment path is idempotent and covers the restore
      safety-net case (verified with tests).
- [ ] Client "Restore Purchases" action — blocked on #8481.

## 3. Account deletion + remaining-credit policy

**Rule (Apple Guideline 5.1.1(v)):** an app that supports account creation
must also let the user initiate account deletion from within the app.

**Implementation status:** fixed in #8502. `AccountSection` in
`clients/khala-mobile/src/screens/settings-screen.tsx` now exposes
**Delete account** beside Sign out and presents the policy copy below before
calling the main Worker route `DELETE /api/mobile/account`. The server route
(`apps/openagents.com/workers/api/src/mobile-account-deletion-routes.ts`)
uses the mobile bearer session, deletes owner-scoped Khala Sync chat/runtime
data, removes push registrations, disconnects GitHub write links, removes
OpenAuth subject storage, marks the user/identity deleted, forfeits remaining
Pool B credit balance, records a retry-safe deletion receipt, and revokes the
presented bearer token.

**Policy (plain language, shown in the confirmation modal):**

> Deleting your Khala account permanently removes your GitHub sign-in
> link, your chat threads and turn history, and your device's push
> notification registration. **Any remaining credit balance is forfeited
> and is not refunded** — credits are non-transferable and have no cash
> value (this mirrors how App Store/Play Store consumable in-app purchases
> already work: once purchased, a consumable credit is not eligible for a
> store refund except through Apple's/Google's own refund-request flow,
> which we honor via the webhook clawback in #8482 when the store notifies
> us). If you believe you were charged in error, request a refund through
> the App Store or Play Store directly (not through us) — see §4 below for
> exactly what happens on our side when a refund is granted.

**Checklist:**
- [x] Policy language drafted (above).
- [ ] **Compliance gap, needs a follow-up implementation issue**: no
      in-app account-deletion mechanism exists. Recommend filing before
      App Store/Play submission (WS-I, #8491/#8493 territory).

## 4. Refund handling (plain user-facing terms)

**What actually happens (verified in #8482):** when Apple/Google notify
RevenueCat of a refund or chargeback and RevenueCat's webhook relays it to
us (`REFUND`/`CANCELLATION` event types), we claw back the exact credited
amount from your balance via the existing `clawbackInferenceCredits`
primitive — idempotent (a duplicate refund notification never claws back
twice) and bounded (if you already spent the credit and your balance can't
absorb the full clawback, the ledger's `CHECK (balance_msat >= 0)`
constraint stops it at zero rather than going negative;
`ClawbackOutcome.insufficientBalance` surfaces this case for manual
review — see `inference-abuse-controls.ts`'s `clawbackInferenceCredits`
doc comment).

**User-facing copy (for wherever refund policy is published — the app's
Settings/legal screen, App Store Connect's required "app uses IAP, no
special terms" or custom EULA field, and any web terms page):**

> Refunds for credit-pack purchases are handled by Apple/Google directly
> through their standard refund-request process — we do not process
> refunds ourselves. If Apple/Google approve your refund request, we are
> notified automatically and remove the matching credit from your
> balance. If you've already spent that credit, we remove as much as your
> current balance can cover; we never take you into a negative balance.

**Checklist:**
- [x] Refund clawback mechanism verified end-to-end with tests (#8482).
- [x] Plain-language refund copy drafted (above) — needs to be placed in
      the app's legal/terms screen and App Store Connect metadata once
      that surface exists (WS-I).

## 5. Effective margin per credit pack after the store's cut

**The real finding (computed, not estimated — see
`iap-margin-analysis.ts` + its 5 passing tests):** the credit-pack rail
(#8482) grants the **full face value** of a purchase as spendable credit
(never discounted for the store's cut). That means the store's cut comes
entirely out of OUR margin on the eventual inference spend, not the
user's balance. Using this repo's `DEFAULT_MARGIN` (`pricing.ts`, 0.4 = a
sell price 40% over compute cost, i.e. ≈28.57% of face value once a
credit dollar is fully spent on inference):

| Pack | Face value | Store cut | Net cash to us | Compute cost to fully deliver | Profit if fully spent | Margin on net cash |
|---|---|---|---|---|---|---|
| `credits_499` | $4.99 | 30% (standard) | $3.493 | $3.564 | **-$0.071** | **-2.04%** |
| `credits_499` | $4.99 | 15% (Small Business) | $4.242 | $3.564 | +$0.677 | +15.97% |
| `credits_999` | $9.99 | 30% (standard) | $6.993 | $7.136 | **-$0.143** | **-2.04%** |
| `credits_999` | $9.99 | 15% (Small Business) | $8.492 | $7.136 | +$1.356 | +15.97% |
| `credits_1999` | $19.99 | 30% (standard) | $13.993 | $14.279 | **-$0.286** | **-2.04%** |
| `credits_1999` | $19.99 | 15% (Small Business) | $16.992 | $14.279 | +$2.713 | +15.97% |

**This is a real business-risk finding, not a rounding error:** at
Apple's/Google's **standard 30% cut**, our current 40% inference margin
(≈28.57% of face value) is **smaller than the store's cut**, so any pack
that gets **fully spent** on inference runs at a small loss (~2% of net
cash received). At the **Small Business Program's 15% cut** (available to
developers with <$1M in prior-year store proceeds — likely true for a
pre-launch product), the same packs are profitable (~16% margin on net
cash).

**Recommendation (owner decision, not made here):** enroll in Apple's
Small Business Program and Google's equivalent tier as soon as eligible
(this alone flips the numbers from a loss to a healthy margin), and treat
it as a launch-blocking financial task, not just a nice-to-have — it is
the single highest-leverage lever available and requires no code or
pricing change. As secondary/complementary levers, worth owner
consideration but NOT implemented here (they are pricing/product
decisions): raising pack prices, granting slightly less than 100% face
value as credit, or applying a higher margin specifically to
IAP-purchased credit. In practice, real-world "breakage" (many users never
fully deplete a purchased pack) provides some cushion regardless, but the
above numbers are the worst-case (100%-spent) bound and should be the one
used for compliance/business planning, not an optimistic average.

**Checklist:**
- [x] Per-pack effective margin computed and verified with tests, for both
      the standard and Small Business Program store-cut rates.
- [ ] **Owner decision needed**: enroll in Apple/Google's small-business
      discounted-fee programs before or shortly after launch (see
      `NEEDS_OWNER.md`).

## 6. Submission-readiness summary

| Area | Status |
|---|---|
| No external-payment steering (3.1.1) | ✅ Done — audited, zero findings, today's code |
| Purchase sheet uses native IAP only | ⏳ Blocked on #8481 (not built yet) |
| Restore purchases — server | ✅ Done — idempotent fulfillment covers it |
| Restore purchases — client button | ⏳ Blocked on #8481 |
| Account deletion policy (written) | ✅ Done — drafted above |
| Account deletion mechanism (built) | ❌ **Gap — needs a new follow-up issue**, not in scope here |
| Refund handling (mechanism + copy) | ✅ Done — verified + drafted |
| Per-pack effective margin | ✅ Done — computed + tested; recommends Small Business Program enrollment |

This checklist itself, plus the five items marked ✅, is what #8483 commits
as "submission-ready payment compliance with the checklist committed" — the
three ⏳/❌ items are explicitly NOT claimed done and are named as
follow-ups for #8481 and a new account-deletion work item, matching this
repo's product-promise discipline of never silently describing a partial
state as complete.
