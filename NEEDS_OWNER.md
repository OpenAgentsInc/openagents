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
3. **CX-2 tap-through: attempted, FAILED** — your bug report (stale
   account pile-up; Disconnect only reorders) recorded verbatim on #8546
   with a behavior contract; audit+fix lane running. You'll get a "try
   again" ping when it's actually fixed.
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
