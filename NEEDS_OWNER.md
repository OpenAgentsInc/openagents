# NEEDS-OWNER — 4 items left, ~12 minutes

Updated 2026-07-09 (post-GO). Archive of everything older:
`docs/ops/2026-07-09-needs-owner-archive.md`. Full backlog
context: `docs/fable/2026-07-09-open-issue-grid-assessment.md`.

**Resolved this session:** sales-pipe GO given → an agent is arming prod
CRM sends now (Sarah identity, Sites sender untouched; receipt will land
on #8558). Seeded test account confirmed: **AgentFlampy** +
github.com/AgentFlampy/openagents (recorded on #8543; agents wire the E2E
themselves). Sarah `/sarah` serving gates resolved (live on the monolith,
S-12 6/6).

## THE ASK — 4 items, ~12 minutes

**1. Codex connect tap-through (~5 min, phone) — closes CX-2 #8546.**
Khala mobile → Settings → Codex accounts → Connect → complete the browser
short-code approval → confirm `ready` → Disconnect → confirm removal.
That's the entire exit receipt.

**2. WEB-1 landing review (~5 min) — #8565 root cutover.** Look at the
`/new` preview and the `/stage1` Effect Native render; say yes/no/changes
on the sales copy and whether to flip the root route. Structure is done;
only your words + the flip decision remain.

**3. Firewall confirmation (~1 min) — #8591 residue.** The Phase 6
control plane has `0.0.0.0/0` open on port 8787 (`oa-codex-control-1`,
`35.223.189.76`) from cutover smoke. Say "testing done, tighten it" and
an agent narrows it same-day.

**4. OpenRouter decision (~1 min).** The Khala 502 fix is live; each
request wastes one dead-lane 402 hop (~0.5s). Top up the OpenRouter
balance, or say "drop the lane" and an agent reorders the plan.

## Deferred / standing (no action needed now)

- **effectnative.org domain verification** — verify the domain in the
  Google account owning `openagentsgemini`, then agents rerun the Cloud
  Run domain mapping (#8571). Whenever convenient.
- **Grok free-window check (weekly)** — confirm Grok 4.5 is still free on
  the CLI session; on expiry say so and `auto` re-ranks itself.
- **CX-3 bake host** — the one infra task behind five issues' live exits
  (rootfs build on `agent-computer-gce-1`). Agents attempt it next
  KVM-capable session; nothing from you unless we hit a wall.
- **`apps/web` legacy-SPA cutover posture** — EN-4 (#8573) converts safe
  routes now; the live Foldkit SPA conversion waits on your serving call.
- **Optional Vercel teardown** — old Sarah project; `sarah.openagents.com`
  already NXDOMAIN, teardown is cleanup only.
