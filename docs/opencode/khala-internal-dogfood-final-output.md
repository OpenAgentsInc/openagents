Here is the full planning memo:

---

# Planning Memo: Routing Internal Agent Work Through Khala

## 1. Thesis

Khala is live at `POST https://openagents.com/api/v1/chat/completions`, model `openagents/khala`, with a free tier (200 req/200k tok/day) and a public tokens-served counter. The GTM strategy (`docs/inference/2026-06-25-khala-inference-gtm-push.md`) names **tokens served per day** as the north-star metric, and Pillar 1 is **internal dogfood** — routing every agent we already run through Khala to harden the product while moving the counter.

This memo translates that strategy into concrete routing work for the internal OpenAgents agent systems.

## 2. Dogfood Targets — Ordered by Impact

| Priority | System | What to Route | Token Volume | Status |
|---|---|---|---|---|
| **P0** | **qa-runner** | All agent inference (browser automation, verifier probes) | High — runs continuously | Ready: `apps/qa-runner/package.json` already has `openai-compatible` keyword; just needs base URL + key config |
| **P0** | **OpenCode** | All coding-agent inference (edit/run loop) | High per session | Recipe exists (see §3); blocking on #6232 deploy for tool-call fix |
| **P1** | **Autopilot / Raynor** | Coding sessions, forum/progress posting, product agent reasoning | High — anchor buyer per business doc | Needs config change to default provider |
| **P1** | **Probe runtime** | All local coding-agent LLM calls | Medium | `packages/probe/docs/probe-llm-core.md` defines provider-neutral LLM core; add `openagents` as a backend profile |
| **P2** | **Forum agent flows** | Forum inference for moderation, summarization, agent interactions | Medium | Currently routes through other providers |
| **P2** | **Sites generation** | Template/agent model calls for site generation | Medium | Needs addition to `apps/openagents.com/` inference routing |
| **P3** | **Pylon serving nodes** | Inference served through Pylon → Khala gateway | Variable | Directional: needs Pylon transport seam wired (M4 fabric dispatch exists in `khala-loop-integration.ts`) |
| **P3** | **Verse 3D visualization** | NPC/scene/narration inference | Low initially | Directional per GTM doc |
| **P3** | **The gym** | Training eval inference, benchmarks | Medium | Directional: dual-purpose (trains AND uses Khala) |

## 3. OpenCode — The Critical Path

OpenCode is the first external ecosystem target AND a major internal dogfood surface. The integration recipe is verified:

**Config** (from `docs/inference/2026-06-25-opencode-khala-runbook-and-audit.md`):
```json
{
  "provider": {
    "openagents": {
      "npm": "@ai-sdk/openai-compatible",
      "name": "OpenAgents",
      "options": {
        "baseURL": "https://openagents.com/api/v1",
        "apiKey": "{env:OPENAGENTS_API_KEY}"
      },
      "models": {
        "openagents/khala": {
          "name": "Khala",
          "tool_call": true,
          "limit": { "context": 128000, "output": 65536 }
        }
      }
    }
  },
  "model": "openagents/openagents/khala"
}
```

**Blockers:**
- **#6232** — OpenCode sends typed message content arrays and streamed tool-call deltas. Khala rejects text-only content arrays with `400` and drops `tool_calls` payloads while returning `finish_reason: "tool_calls"`. Fix is deployed but needs production smoke.
- **Model selector path** — `openagents/openagents/khala` (doubled segment) is a cosmetic UX issue; may want server-side alias to shorten to `openagents/khala` cleanly in the TUI.
- **Tool-call reliability** — Must be confirmed end-to-end with OpenCode's edit/run loop before publishing the recipe publicly.

**Verification checklist** (pre-publication):
- [ ] Direct chat completion `200` with `usage.total_tokens` reported
- [ ] Public counter delta matches token usage
- [ ] Text-only content arrays accepted (after #6232)
- [ ] Tool-call finish reason includes payload (after #6232)
- [ ] Streaming SSE works for interactive sessions
- [ ] Free-tier quota boundaries produce clean `402`/`429` errors
- [ ] Second-turn tool replay preserves `tool_calls` + `tool_call_id` metadata

## 4. Pylon & Probe — The Serving Node Wiring

Pylon (`apps/pylon/`, v1.0.5 on npm as `@openagentsinc/pylon`) is the contributor node that will eventually serve inference through Khala. The `khala-loop-integration.ts` module owns the seam:

- **M4 fabric dispatch** (`psionic-fabric-serve.ts`): ask-plan → execute → exact-parity receipt
- **M3 settlement** (`khala-verified-work-settlement.ts`): ARMED decision + receipt → Spark payout
- **Double-gated**: `OPENAGENTS_KHALA_LOOP_ARMED` (must equal `armed`) AND `OPENAGENTS_REAL_SETTLEMENT_GATE` (owner JSON gate). Both default OFF.
- **Pluggable transport**: `PylonServeTransport` alias over `PsionicServeTransport`; a real HTTP transport is a drop-in.

Probe (`packages/probe/`) is the coding agent runtime that will be the Pylon work executor. Its LLM core (`packages/runtime/src/llm/`) is provider-neutral. To route through Khala, add an `openagents` backend profile using `@ai-sdk/openai-compatible` or a direct OpenAI-compatible HTTP client.

**Wiring sequence:**
1. Add `openagents` backend to Probe's `backends/registry.ts` (reuses existing provider-neutral contracts)
2. Wire Pylon spawns Probe as local/remote executor (per Probe README: "Pylons should eventually spawn Probes")
3. Connect the M4 dispatch to a live Pylon HTTP transport (currently local/fake only)

## 5. Observability — What Exists and What's Needed

**Already shipped:**
- `KhalaTelemetryRecord` schema (`khala-telemetry.ts`): request-class, tokens, TTFT, wall-clock, verification class + verdict, detailRef
- Block-vs-receipt split: small `openagents` block in immediate response, full depth behind `/api/public/inference/receipts/<ref>`
- Public tokens-served counter: `GET /api/public/khala-tokens-served` and `/history`
- Public activity timeline: `khala_inference_served` event kind
- Honest `measured` vs `not_measured` discipline (never fabricates zeros)

**Needed for dogfood routing:**
- **Per-client attribution** — tag tokens by consumer system (qa-runner, opencode, autopilot, probe, etc.) so the internal-vs-external split is measurable
- **Dogfood dashboard** — query the telemetry ledger for internal-system tokens/day, separate from the public counter (the GTM doc explicitly requires distinguishing internal from external demand)
- **Cost tracking** — internal inference consumes real upstream cost (Fireworks/Vertex); track this separately from customer-facing spend to understand the dogfood budget

## 6. Safety Gates — What Each Routing Target Must Respect

| Gate | Where Enforced | What It Does | Routing Impact |
|---|---|---|---|
| **Identity signature** | `khala-identity.ts` — system prompt + verify + correct | Never reveals underlying model/provider | ALL routes: must inject identity system prompt |
| **Refusal posture** | `khala-identity.ts` — system prompt + verify + re-ask | Never bare-refuses; always offers guide path | ALL routes: must inject refusal posture clause |
| **Fair-share limits** | `inference-abuse-controls.ts` — per-window request + token ceiling | One customer can't starve shared quota | Internal systems get same limits unless exempted via operator exemption |
| **Spend caps** | `inference-abuse-controls.ts` — per-account msat ceiling | Compromised key can't drain balance | Internal keys need high caps or operator exemption |
| **Free-tier quota** | `inference-free-tier-key.ts` — 200 req / 200k tok per UTC day | Free lane bounded | Internal dogfood should use registered `oa_agent_` tokens (not free mint) to avoid hitting free-tier limits |
| **Code verifier** | `khala-code-verifier.ts` — prescreen → headless execution | Honest `unverified` unless executed | QA-runner and OpenCode paths: prescreen passes, but verdict stays `unverified` unless the acceptance runner executes the artifact |
| **Settlement loop** | `khala-loop-integration.ts` — double-gated (loop flag + owner gate) | Inert by default; ZERO sats move without both gates | Pylon payout path only; not relevant for dogfood routing itself |
| **Operator exemption** | `inference-operator-exemption.ts` — model-level bypass for `openagents/khala` | Internal operator keys can bypass certain gates | Needed for high-volume internal routing to avoid hitting fair-share limits |

**Key safety question:** Should internal dogfood traffic use the same free-tier quota path as external users, or should there be an operator-exempt internal lane? The GTM doc says internal tokens are real served tokens but should be distinguishable. Recommendation: use registered `oa_agent_` tokens with operator exemption for high-volume systems (qa-runner, Autopilot), and free-tier keys for ad-hoc usage (Probe dev sessions).

## 7. What To Do First — Ordered Execution Plan

### Phase 1 — Immediate (this week)

1. **Deploy #6232 compatibility fix** — production smoke the tool-call fix for OpenCode compatibility
2. **Route qa-runner through Khala** — config change only (already `openai-compatible`)
3. **Verify end-to-end OpenCode smoke** — dedicated `oa_agent_` token, confirm tool calls work, confirm counter moves
4. **Publish internal OpenCode recipe** — in repo docs; not public copy until promise gate clears

### Phase 2 — Short-term (next week)

5. **Add `openagents` backend to Probe** — new backend profile in `packages/runtime/src/backends/`
6. **Route Autopilot/Raynor through Khala** — config change to default provider
7. **Add per-client attribution** — tag telemetry records with consumer system id
8. **Build dogfood dashboard** — query internal tokens/day vs external

### Phase 3 — Medium-term

9. **Wire Pylon transport seam** — connect `khala-loop-integration.ts` to a live Pylon HTTP transport
10. **Route Forum/Sites inference through Khala** — default lane for agent flows
11. **Run the owner-armed benchmark sweep** — decision-grade report over realistic dogfood traffic
12. **Broaden ecosystem tools** — Aider, Cline/Continue, Vercel AI SDK, LiteLLM recipes

### Phase 4 — Long-term

13. **Wire the gym → Khala** — training eval runs through the serving endpoint
14. **Wire Verse → Khala** — NPC/narration inference through the public counter
15. **Arm the settlement loop** — owner-gated Spark/Bitcoin payout to guinea-pig Pylon

## 8. Open Questions

1. **Operator exemption strategy** — Should internal dogfood use a separate API key class (e.g., `oa_internal_` prefix) with different rate limits, or the same `oa_agent_` keys with operator exemptions? The exemption module already supports model-level bypass; decide the key-level policy.

2. **Token attribution granularity** — The telemetry schema records request class but not consumer system id. Add a `consumer: string` field to `KhalaTelemetryRecord` so we can query "how many tokens did OpenCode consume today?"

3. **OpenCode model selector UX** — The doubled `openagents/openagents/khala` path in the TUI. Server-side short alias or accept it? Affects the published recipe.

4. **Free tier vs internal budget** — High-volume internal systems (qa-runner running continuously) will blow through free-tier limits in minutes. Do we route them through the paid lane against an internal credit budget, or create an exempt lane that bypasses metering? The GTM doc says internal tokens count toward the public counter but should be distinguishable — this implies metering still runs but with a different attribution tag.

5. **Pylon transport readiness** — The M4 fabric dispatch exists but only has a local/fake `PylonServeTransport`. Who builds the real HTTP transport? Depends on the Pylon guinea-pig deployment timeline.

6. **Benchmark timing** — The GTM doc says "internal dogfood IS the realistic traffic the benchmark needs." How many days of dogfood traffic needed before the first `decisionGrade: true` benchmark report? Recommend: at least 5 days of continuous qa-runner traffic before arming the real benchmark seam.
