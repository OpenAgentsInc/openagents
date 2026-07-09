# NEEDS_OWNER — one session, ~30 minutes, clears 8 issues

Rewritten 2026-07-09 after the overnight burn-down (28+ issues closed, 3
production incidents fixed — full context:
`docs/fable/2026-07-09-open-issue-grid-assessment.md`). Prior content is
archived verbatim at `docs/ops/2026-07-09-needs-owner-archive.md`.

## THE ASK — one sitting, highest value-per-minute first

**1. Arm the sales pipe (~10 min)** — the entire outbound track (OB-1
#8558 → OB-4 #8561 / OB-6 #8563 live loop) is parked behind this single
gate. Pick Sarah's sending subdomain (suggested `mail.openagents.com`),
add the SPF/DKIM/DMARC records Resend shows for it, and set prod secrets
`CRM_RESEND_SEND_ENABLED=1` + `RESEND_API_KEY` + `RESEND_FROM_EMAIL`
(secret flow per `docs/DEPLOYMENT.md`). Everything downstream is built and
tested; it starts accruing the moment this arms.

**2. Codex connect tap-through (~5 min, phone)** — closes CX-2 #8546.
Khala mobile → Settings → Codex accounts → Connect → complete the browser
short-code approval → confirm `ready` → Disconnect → confirm removal.
That's the entire exit receipt.

**3. Seeded test account (~5 min)** — unblocks P0.8 #8543's unattended
E2E. Create a public-safe throwaway GitHub account (no real repos/PII)
and hand any agent the handle; we wire it in ourselves.

**4. WEB-1 landing review (~5 min)** — #8565. Review the `/new` preview +
the `/stage1` Effect Native render; give copy sign-off and the root-flip
yes/no. Structure is done; only your words and the flip remain.

**5. Firewall confirmation (~1 min)** — #8591 Phase 6 residue: the
control plane deployed tonight has `0.0.0.0/0` open on port 8787
(`oa-codex-control-1`) for cutover smoke, per its own receipt. Say
"testing done, tighten" and an agent narrows it same-day.

**6. OpenRouter decision (~1 min)** — the Khala 502 fix is live; each
request now wastes one dead-lane 402 hop (~0.5s). Top up the OpenRouter
balance, or say "drop the lane" and an agent reorders the plan.

## Deferred / standing (no action needed now)

- **effectnative.org verification** — verify the domain in the Google
  account owning project `openagentsgemini`; agents rerun the Cloud Run
  domain mapping (#8571). Whenever convenient.
- **Grok free-window check (weekly)** — confirm Grok 4.5 is still free on
  the CLI session; on expiry, say so and `auto` re-ranks (no code change).
- **CX-3 bake host** — one infra task (rootfs build on the KVM host)
  behind five issues' live exits; agents attempt it next KVM-capable
  session, nothing from you unless we hit a wall.
- **`apps/web` legacy-SPA cutover posture** — EN-4 #8573 converts safe
  routes now; the live Foldkit SPA conversion waits on your serving call.
- **Sarah `/sarah` serving cutover (#8594)** — owner gates RESOLVED
  (path confirmed `openagents.com/sarah`, no subdomain); remaining mount
  + smoke work is agent-owned, nothing from you.

## Sarah consolidation SM-5 / serving (#8594)

**Serving amendment CONFIRMED (owner, 2026-07-09):** public path is
`https://openagents.com/sarah` only — **no** `sarah.openagents.com` subdomain.
Attribution in `docs/sarah/MIGRATION.md` and MASTER_ROADMAP rev 6.8 is correct.

Agent-owned SM-5 work (shipped or shipping with this change):
- Mount `/sarah` on `openagents-monolith` Cloud Run via `handleSarahRequest`.
- Live S-12 / production smokes against `https://openagents.com/sarah`.
- Vercel project teardown after live `/sarah` oracles are green (DNS for the
  old subdomain already does not resolve).

**Obsolete:** any older framing of Sarah as a Cloudflare D1-quota host or
separate-subdomain deploy problem. Monorepo Sarah does not hold D1; Worker API
authority stays over public HTTP contracts. D1 free-tier quota remains a
general monolith CFG-4 concern only.
