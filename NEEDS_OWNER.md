# NEEDS-OWNER — nothing immediate. All decisions taken 2026-07-09.

Archive of older content:
`docs/ops/2026-07-09-needs-owner-archive.md`. Full backlog
context: `docs/fable/2026-07-09-open-issue-grid-assessment.md`.

## Decisions taken this session (all in execution, agents working)

1. **Sales pipe: GO → ARMED AND PROVEN.** Prod rev `00047-kzh`:
   `CRM_RESEND_SEND_ENABLED=1`, Sarah's own from/reply-to (Sites sender
   untouched), and a real CRM-route proof send delivered from
   `Sarah <sarah@openagents.com>` to your inbox (Resend `ccf31693`,
   receipt on #8558). Live outbound is ON; warm-up caps enforced.
2. **Seeded test account: AgentFlampy** + fork recorded on #8543; E2E
   wiring lane running.
3. **CX-2 tap-through: FIXED — READY FOR YOUR RE-TEST (~3 min).** Both
   bugs root-caused and fixed (2bf644f994, live on main): Disconnect now
   terminally removes (it was soft-updating status while the list showed
   every non-deleted row — hence the "reordering"), and the list shows
   ONLY live accounts. Your phone should now show just your 5 connected
   accounts (was 26 rows incl. 16 dead + 5 expired). Behavior contract
   with your words landed + 2 oracles. **Try again:** Settings → Codex
   accounts → confirm the list is clean → Connect (short-code) → confirm
   ready → Disconnect → confirm it's GONE. That closes #8546.
4. **WEB-1: not ready — full Effect Native conversion mandated.** All
   landing sections convert to EN with logical components upstreamed to
   the standard catalog: effect-native #46–#51 filed (Hero/Section/
   Announcement/CTA/Footer, NavBar, Accordion/FAQ, PricingTable,
   LogoRow/StatsBand, Glow/Mockup), conversion program filed as
   openagents #8595. Root-flip decision returns to you AFTER that lands
   and you re-review.
5. **Firewall: DEFERRED by you** ("not done testing") — `0.0.0.0/0:8787`
   stays open on `oa-codex-control-1` until you say tighten.
6. **OpenRouter: DROP THE LANE** → agent removing it from the prod plan
   now; gcloud (Gemma4-primary direction) leads; adapter deprecate-marked,
   physical removal to backroom can follow.

## Standing / will ping you when ready

- **CX-2 re-test** — after the accounts audit+fix lands.
- **WEB-1 re-review** — after the EN landing conversion (#8595) lands.
- **Firewall tighten** — whenever you finish testing, say the word.
- **effectnative.org domain verification** — verify the domain in the
  Google account owning `openagentsgemini`; agents rerun the Cloud Run
  mapping (#8571). Whenever convenient.
- **Grok free-window check (weekly)** — on expiry say so; `auto` re-ranks.
- **Gemma4 lane** — if the current gcloud lane turns out not to serve
  Gemma4 yet, the drop-OpenRouter agent will report exactly what a
  Gemma4 adapter needs; that may come back as one small decision.
---

# Contract-anchored owner gates (kept verbatim for QAM oracles — do not delete; update state inline)

The two sections below are asserted by `clients/khala-mobile/tests/launch-readiness.test.ts` and `store-submissions.test.ts` (owner-gate-documented oracles). They stay here until their receipts flip; current state is annotated.

## Khala Mobile P0.8 Launch Readiness {#khala-mobile-p08-launch-readiness}

Source issue: OpenAgentsInc/openagents#8543

State 2026-07-09: the test account exists (**AgentFlampy** + fork, recorded on #8543; E2E wiring lane running). Remaining actions:

- Create or approve a public-safe GitHub test account for Khala Mobile launch readiness. *(DONE — AgentFlampy)*
- Grant only the repo scopes needed for the smoke repo and writeback proof.
- Seed a visible $10 launch credit grant and record the public-safe grant receipt ref.
- Run the full straight-line E2E on iOS simulator and Android emulator.
- Review the launch promises/copy pass only after both platform E2E receipts exist.

## Khala Mobile P0.9 Store Submissions {#khala-mobile-p09-store-submissions}

Source issue: OpenAgentsInc/openagents#8544

State 2026-07-09: per the owner decision on #8544, public review submissions are deferred to broad-release readiness; artifacts (TestFlight build 20, Android APK/AAB) exist. Deferred actions:

- Create or confirm the App Store Connect app record for com.openagents.khala.mobile.
- Upload the final locally built iOS archive through Apple Transporter or Xcode Organizer.
- Enter current App Store metadata, screenshots, privacy answers, age rating, and review notes.
- Submit the iOS build for review and record the App Store Connect submission ID and review state.
- Create or confirm the Play Console app record for com.openagents.khala.mobile.
- Upload the final locally signed Android App Bundle to the intended Play track.
- Enter current Play listing, data-safety, content-rating, tester/release notes, and review answers.
- Submit the Play release and record the Play Console release/submission ID and review state.

Consoles: https://appstoreconnect.apple.com/apps and https://play.google.com/console
