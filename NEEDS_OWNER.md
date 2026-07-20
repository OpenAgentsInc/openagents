# NEEDS-OWNER

## IDE-13 owner-managed environment authority (2026-07-20)

IDE-13 requires an owner-managed execution target. The current target binding
proves the owner and the live Pylon. It does not prove that the owner enrolled
the Pylon as an owner-managed environment. The current server also cannot open
an owner-managed checkpoint without an owner key.

The recommended design keeps the checkpoint key on the enrolled Pylon. The
server stores and moves ciphertext only. A new server enrollment record binds
the owner, target, Pylon, target class, adapter, compatibility, isolation, data
posture, health, expiry, update receipt, and revoke state. The Pylon uses its
local owner key to open the checkpoint after it verifies that enrollment. Raw
key bytes do not go to Sync or the API.

**Smallest owner decision:** Approve this owner-held-key and server-enrollment
design, or require OpenAgents KMS custody for an owner-managed target. Work on
other IDE-13 requirements continues while this decision is open.

## Previous decisions

## RESOLVED — Desktop local-usage telemetry consent copy (#8911, 2026-07-16)

The owner reviewed the disclosure and approved it with one required rewording:
the original phrasing "sends exact input, cached-input, output, reasoning, and
total token counts" read as if the input text itself were sent. Owner statement
(2026-07-16): "sounds like i send exact input - fix that, reword then yes
approved."

The approved copy now shipped in Settings:

> **Share local Codex usage**
>
> When on, OpenAgents reports how many tokens each turn used — the input,
> cached-input, output, reasoning, and total token counts — plus the model
> name and a one-time turn reference. Only those numbers are sent: never your
> prompts, responses, files, paths, account names, or credentials. This
> updates the aggregate public tokens-served counter. Turn it off any time.
> queued reports are deleted.

Consent-copy approval and the agent-driven live proof are COMPLETE. The proof
used the supported typed `session.sign_in` runtime-gateway command to complete
GitHub OAuth + PKCE without restoring the intentionally retired renderer
account-link surface. Ordinary, restart/retry, opt-out, Full Auto, exact-row,
idempotency, and public-counter checks passed. No owner action remains for
#8911. Detailed public-safe receipts are in
`docs/sol/2026-07-16-desktop-local-usage-opt-in-verification.md`.

## Decisions taken this session (all in execution, agents working)

1. **Sales pipe: GO → ARMED AND PROVEN.** Prod rev `00047-kzh`:
   `CRM_RESEND_SEND_ENABLED=1`, Sarah's own from/reply-to (Sites sender
   untouched), and a real CRM-route proof send delivered from
   `Sarah <sarah@openagents.com>` to your inbox (Resend `ccf31693`,
   receipt on #8558). Live outbound is ON. Warm-up caps enforced.
2. **Seeded test account: AgentFlampy → E2E WIRED, both platforms green
   on all runnable legs.** iOS sim + Android emulator both pass sign-in →
   repo-picker → live dispatch/reply (591bc110be). Blocked legs are typed
   skips, never faked: push/writeback waits on CX-3, and ONE optional
   human step remains for the fork-bind+credits legs — a single GitHub
   sign-in as AgentFlampy (agent tokens cannot do user-session routes by
   design). Not urgent. The runner auto-detects the session when captured.
   (Side win: this lane found + fixed prod chat Send 500ing for ALL
   accounts — Cloud SQL was missing khala-sync migrations 0047–0050. Now
   applied, `chat.appendMessage` → 200 verified.)
3. **CX-2 tap-through: FIXED — READY FOR YOUR RE-TEST (~3 min).** Both
   bugs root-caused and fixed (2bf644f994, live on main): Disconnect now
   terminally removes (it was soft-updating status while the list showed
   every non-deleted row — hence the "reordering"), and the list shows
   ONLY live accounts. Your phone should now show just your 5 connected
   accounts (was 26 rows incl. 16 dead + 5 expired). Behavior contract
   with your words landed + 2 oracles. **Try again:** Settings → Codex
   accounts → confirm the list is clean → Connect (short-code) → confirm
   ready → Disconnect → confirm it is GONE. That closes #8546.
4. **WEB-1: not ready — full Effect Native conversion mandated.** All
   landing sections convert to EN with logical components upstreamed to
   the standard catalog: effect-native #46–#51 filed (Hero/Section/
   Announcement/CTA/Footer, NavBar, Accordion/FAQ, PricingTable,
   LogoRow/StatsBand, Glow/Mockup), conversion program filed as
   openagents #8595. Root-flip decision returns to you AFTER that lands
   and you re-review.
5. **Firewall: DEFERRED by you** ("not done testing") — `0.0.0.0/0:8787`
   stays open on `oa-codex-control-1` until you say tighten.
6. **OpenRouter: DROPPED + Gemma4 IS NOW PRIMARY.** Both done and live:
   OpenRouter out of every plan (prod rev 00049), then a real **Gemma 4**
   lane (`gemma-4-31b-it` via our gcloud Generative Language API) built and
   made the PRIMARY conversational lane (prod rev 00050-7sp). Verified:
   normal completions serve `worker=google-gemma4`, tool-bearing requests
   correctly stay on GLM (Gemma has no tools — airtight two-layer guard),
   canary green, exact reasoning-token accounting. Reused the existing
   Gemini secret — no credential action needed. BYOK preserved. Your
   "Gemma4 via gcloud primarily" directive is fully satisfied.

## Standing / will ping you when ready

- **CX-2 re-test** — after the accounts audit+fix lands.
- **WEB-1 re-review** — after the EN landing conversion (#8595) lands.
- **Firewall tighten** — whenever you finish testing, say the word.
- **effectnative.org domain verification** — verify the domain in the
  Google account owning `openagentsgemini`. Agents rerun the Cloud Run
  mapping (#8571). Whenever convenient.
- **Grok free-window check (weekly)** — on expiry say so. `auto` re-ranks.
- **Gemma4 lane** — if the current gcloud lane turns out not to serve
  Gemma4 yet, the drop-OpenRouter agent will report exactly what a
  Gemma4 adapter needs. That may come back as one small decision.

## OpenAgents Desktop identity freeze — resolved by DIST-01

Source issues: OpenAgentsInc/openagents#8574 and #8914

No owner action is currently ready. The normative identity set is now frozen
in `docs/deploy/openagents-desktop-cross-platform-release.md` §2: stable and RC
are separate installs/state roots. Stable uses `com.openagents.desktop` and RC
uses `com.openagents.desktop.rc`, with exact Windows/Linux/protocol identities
recorded there. Retired Khala Code and Autopilot identities remain prohibited.

Apple, Windows, release-key, runner-enrollment, DNS, certificate, or account
actions are added here only when the corresponding implementation has landed,
the least-privilege UI action is exact, and an agent can verify it immediately
under ProductSpec §17. Generic signing or runner requests are intentionally not
owner tasks.

---

# Contract-anchored owner gates (kept verbatim for QAM oracles — do not delete, update state inline)

The two sections below are asserted by `clients/khala-mobile/tests/launch-readiness.test.ts` and `store-submissions.test.ts` (owner-gate-documented oracles). They stay here until their receipts flip. Current state is annotated.

## Khala Mobile P0.8 Launch Readiness {#khala-mobile-p08-launch-readiness}

Source issue: OpenAgentsInc/openagents#8543

> **SUPERSEDED 2026-07-09:** the Khala mobile app is deprecated and frozen. Do
> not complete these actions to launch it. The checklist remains verbatim for
> legacy QAM oracles and parity/migration evidence. New OpenAgents mobile owner
> gates belong to #8597 and must target `com.openagents.app`.

State 2026-07-09 (E2E wiring landed): the test account exists (**AgentFlampy** + fork, recorded on #8543) and is wired into the unattended harness (`clients/khala-mobile/scripts/straight-line-e2e-run.sh`, typed leg registry `clients/khala-mobile/src/qa/straight-line-e2e.ts`). The runnable iOS-simulator legs are green (signed-in smoke, repo-picker reachability, dispatch → live hosted_khala reply). Receipt: `docs/khala-mobile/2026-07-09-straight-line-e2e-agentflampy-receipt.md`. Remaining actions:

- Create or approve a public-safe GitHub test account for Khala Mobile launch readiness. *(DONE — AgentFlampy)*
- Grant only the repo scopes needed for the smoke repo and writeback proof.
- Seed a visible $10 launch credit grant and record the public-safe grant receipt ref.
- **Owner tap (~2 min, unblocks two legs): complete one GitHub sign-in as AgentFlampy** in any Khala Code mobile build, then have an agent capture that session's `{ ownerUserId, syncToken }` into `~/work/.secrets/khala-mobile-session.env` (never printed/committed, ~400-day token). The repo-list/bind and credits legs are mobile-USER-session-only by invariant (the seeded agent token 401s them by design). The harness probes the gate each run and picks the session up automatically.
- Run the full straight-line E2E on iOS simulator and Android emulator. *(iOS runnable legs green 2026-07-09, repo-bind + credits legs wait on the session tap above, the push/writeback leg waits on CX-3 #8547.)*
- Review the launch promises/copy pass only after both platform E2E receipts exist.

## Khala Mobile P0.9 Store Submissions {#khala-mobile-p09-store-submissions}

Source issue: OpenAgentsInc/openagents#8544

> **CANCELLED FOR THIS APP 2026-07-09:** do not submit or create store records
> for `com.openagents.khala.mobile`. These actions remain only because legacy
> tests anchor the text. The greenfield OpenAgents app uses the owner-designated
> existing identifier `com.openagents.app`. Its real store records, signing,
> provisioning, and monotonic build numbers must be verified under #8597.

State 2026-07-09: per the owner decision on #8544, public review submissions are deferred to broad-release readiness. Artifacts (TestFlight build 20, Android APK/AAB) exist. Deferred actions:

- Create or confirm the App Store Connect app record for com.openagents.khala.mobile.
- Upload the final locally built iOS archive through Apple Transporter or Xcode Organizer.
- Enter current App Store metadata, screenshots, privacy answers, age rating, and review notes.
- Submit the iOS build for review and record the App Store Connect submission ID and review state.
- Create or confirm the Play Console app record for com.openagents.khala.mobile.
- Upload the final locally signed Android App Bundle to the intended Play track.
- Enter current Play listing, data-safety, content-rating, tester/release notes, and review answers.
- Submit the Play release and record the Play Console release/submission ID and review state.

Consoles: https://appstoreconnect.apple.com/apps and https://play.google.com/console
