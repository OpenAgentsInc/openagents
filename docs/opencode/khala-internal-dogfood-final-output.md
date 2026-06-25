Here is the full planning memo:

---

# Planning Memo: Routing Internal Agent Work Through Khala

## 1. Thesis

Khala is live at `POST https://openagents.com/api/v1/chat/completions`, model `openagents/khala`, with a free tier (200 req/200k tok/day) and a public tokens-served counter. The GTM strategy (`docs/inference/2026-06-25-khala-inference-gtm-push.md`) names **tokens served per day** as the north-star metric — it is the demand proxy, the dogfood proxy, the distribution proxy, and (once the paid loop is collectable) the economy proxy. Pillar 1 is **internal dogfood**: routing every agent we already run through Khala to harden the product while moving the counter. The GTM doc makes clear this is the only lever we fully control — we can move the counter meaningfully *before* a single external developer adopts us, and every internal token is a real test that makes the product better for the external developers we court in Pillar 2.

This memo translates that strategy into concrete routing work for the internal OpenAgents agent systems. Pillar 1 alignment: all token volumes below count toward the public `khala-tokens-served` counter but must be distinguishable from external demand in analytics (per the GTM doc's honesty discipline, §6).

## 2. Dogfood Targets — Ordered by Impact

| Priority | System | What to Route | Token Volume | Status |
|---|---|---|---|---|
| **P0** | **qa-runner** | All agent inference (browser automation, verifier probes) | High — runs continuously | Ready: `apps/qa-runner/package.json` already has `openai-compatible` keyword; just needs base URL + key config |
| **P0** | **OpenCode** | All coding-agent inference (edit/run loop) | High per session | Recipe exists (see §3); #6232 fix is shipped and live — tool-call compatibility verified, text-only content arrays accepted, streaming SSE confirmed; model selector shows `openagents/openagents/khala` (cosmetic display only, server-side model id is single `openagents/khala`; no gateway change needed) |
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

**Status: LIVE.** The `#6232` fix is **shipped and production-verified** (deployed from `origin/main`, cost model + raised quota + owner-gated analytics all live). Direct Khala API smoke confirmed: chat-completions `200` with `usage.total_tokens` reported, public counter delta matches token usage exactly. End-to-end OpenCode smoke with tool-call edit/run loop confirmed working. The model selector shows `openagents/openagents/khala` in the TUI (cosmetic doubling of `openagents` prefix — `providerId/modelKey` rendering in OpenCode's selector). The server-side model id remains the single `openagents/khala` segment; no gateway alias change is needed. The cosmetic display concern is purely OpenCode's concatenation of provider id + model key; if it bothers users, the fix would be a shorter model key alias or a display hint in the OpenCode config, not a gateway change.

**Verification checklist** (all PASSED):
- [x] Direct chat completion `200` with `usage.total_tokens` reported
- [x] Public counter delta matches token usage
- [x] Text-only content arrays accepted (fix shipped)
- [x] Tool-call finish reason includes payload (fix shipped)
- [x] Streaming SSE works for interactive sessions
- [x] Free-tier quota boundaries produce clean `402`/`429` errors
- [x] Second-turn tool replay preserves `tool_calls` + `tool_call_id` metadata

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
3. Connect the M4 dispatch to the existing `PylonFabricHttpTransport` (`apps/openagents.com/workers/api/src/inference/pylon-fabric-http-transport.ts` — a secret-backed HTTP transport speaking the Psionic serve response contract, already wired with admission guards, heartbeat TTL, and the `OPENAGENTS_NETWORK_ADAPTER_ID` boundary). The HTTP transport layer is real; the remaining gap is end-to-end smoke with a live Pylon gateway route

## 5. Observability — What Exists and What's Needed

**Already shipped:**
- `KhalaTelemetryRecord` schema (`khala-telemetry.ts`): request-class, tokens, TTFT, wall-clock, verification class + verdict, detailRef
- Block-vs-receipt split: small `openagents` block in immediate response, full depth behind `/api/public/inference/receipts/<ref>`
- Public tokens-served counter: `GET /api/public/khala-tokens-served` and `/history`
- Public activity timeline: `khala_inference_served` event kind
- Honest `measured` vs `not_measured` discipline (never fabricates zeros)

**Needed for dogfood routing:**
- **Per-client attribution** — tag tokens by consumer system (qa-runner, opencode, autopilot, probe, etc.) so the internal-vs-external split is measurable. The `KhalaTelemetryRecord` schema (`khala-telemetry.ts`) records `requestClass` (interactive_stream, async_job, batch, verifier_run) but does NOT have a `consumer: string` field for the originating system. **Action needed:** add `consumer` field to `KhalaTelemetryRecord` and propagate it through the chat-completions routes and served-tokens recorder. Without this, all internal dogfood tokens are indistinguishable from external traffic in the telemetry ledger
- **Dogfood dashboard** — query the telemetry ledger for internal-system tokens/day, separate from the public counter (the GTM doc explicitly requires distinguishing internal from external demand)
- **Cost tracking** — internal inference consumes real upstream cost (Fireworks/Vertex, Hydralisk GPT-OSS, Gemini Flash overflow); track this separately from customer-facing spend to understand the dogfood budget. The cost model and owner-gated analytics already shipped under `#6232` (`docs/inference/2026-06-25-khala-cost-model-and-analytics.md`), with per-provider cost stored in `token_usage_events.stored_cost` — but currently aggregated only, per-provider, not per-consumer-system

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

### Phase 1 — DONE (week of 2026-06-25)

1. ✅ **#6232 compatibility fix shipped** — tool-call fix for OpenCode deployed, cost model + raised quota + owner-gated analytics all live
2. 🔲 **Route qa-runner through Khala** — config change only (already `openai-compatible`); owner needs to set env vars
3. ✅ **OpenCode end-to-end smoke verified** — dedicated `oa_agent_` token, tool calls confirmed working, counter moves confirmed
4. 🔲 **Publish internal OpenCode recipe** — in repo docs; not public copy until promise gate clears

### Phase 2 — Next

5. **Route qa-runner through Khala** — set `OPENAI_BASE_URL`, `OPENAI_API_KEY` env vars in the qa-runner deployment config (P0, highest steady token volume)
6. **Add per-client attribution** — add `consumer: string` to `KhalaTelemetryRecord`, propagate through `chat-completions-routes.ts` → `served-tokens-recorder.ts` so the telemetry ledger tags every request by originating system (`qa-runner`, `opencode`, `autopilot`, `probe`, `forum`, etc.)
7. **Route Autopilot/Raynor through Khala** — config change to default provider in the agent's runtime config
8. **Build dogfood dashboard** — D1 query over telemetry ledger filtered by consumer tag for internal tokens/day, tracked separately from the public counter
9. **Add `openagents` backend to Probe** — new backend profile in `packages/runtime/src/backends/`

### Phase 3 — Medium-term

10. **Route Forum/Sites inference through Khala** — default lane for agent flows
11. **Pylon HTTP transport end-to-end smoke** — `PylonFabricHttpTransport` exists; needs live Pylon gateway route to complete the end-to-end path
12. **Run the owner-armed benchmark sweep over realistic dogfood traffic** — decision-grade report from the GYM harness, using shapes sourced from observed Khala dogfood traffic (see §8)
13. **Broaden ecosystem tools** — Aider, Cline/Continue, Vercel AI SDK, LiteLLM recipes

### Phase 4 — Long-term

14. **Wire the gym → Khala** — training eval runs through the serving endpoint
15. **Wire Verse → Khala** — NPC/narration inference through the public counter
16. **Arm the settlement loop** — owner-gated Spark/Bitcoin payout to guinea-pig Pylon

## 8. Benchmark-Shape Generation from Dogfood Traffic

The GTM doc's Pillar 3 says "internal dogfood IS the realistic traffic the benchmark needs." The benchmark harness (`apps/openagents.com/workers/api/src/inference/benchmark/`) is typed, fixture-driven, and supports a fixture lane (deterministic, spend-free) and an owner-gated real lane. To produce `decisionGrade: true` reports, shapes must be sourced from observed traffic, not synthetic fixtures.

**How to extract benchmark shapes from dogfood traffic (recommended approach):**

1. **Capture shapes at the telemetry ledger** — Once per-client attribution (`consumer` field) is added to `KhalaTelemetryRecord`, query the ledger for:
   - `requestClass` distribution (what % are interactive_stream vs batch vs verifier_run)
   - Token length distribution per request class (P50/P90/P99 input tokens, output tokens)
   - Prompt length distribution (for realistic context windows)
   - Tool-call density (how many tool_calls per coding session, average arguments length)
   - Time-of-day traffic patterns (for load testing the gym)

2. **Generate shape fixtures** — Write a script (`scripts/extract-benchmark-shapes.ts`) that:
   - Reads recent `token_usage_events` rows (filtered to a given consumer, e.g. `qa-runner` or `opencode`)
   - Anonymizes prompts (strip auth/session/codebase identifiers, keep structural patterns)
   - Outputs JSON fixtures matching the benchmark harness's `BenchmarkWorkload` schema
   - Outputs a shape summary (token percentiles, request class mix, tool-call density)

3. **Produce the first decision-grade report** — After ≥5 days of continuous qa-runner dogfood traffic:
   - Run the gym harness with shapes sourced from the qa-runner telemetry
   - Compare Khala vs Fireworks/Vertex on:
     - Cost-per-accepted-outcome (C/PAO)
     - Verification rate
     - P50/P90/P99 latency
     - Cache hit rate
   - Label report `decisionGrade: true` only if the real lane seam was armed with a live executor

4. **Automate recurring shapes** — Once the extract script is stable, wire it into the gym's recurring pipeline so every benchmark run uses shapes synced from the most recent N days of dogfood traffic, not frozen fixtures.

## 9. Open Questions

1. **Operator exemption strategy** — Should internal dogfood use a separate API key class (e.g., `oa_internal_` prefix) with different rate limits, or the same `oa_agent_` keys with operator exemptions? The exemption module already supports model-level bypass; decide the key-level policy.

2. **Token attribution granularity** — The telemetry schema records request class but not consumer system id. **Action item:** add `consumer: string` to `KhalaTelemetryRecord` in `khala-telemetry.ts` and propagate through the chat-completions routes and served-tokens recorder. Without this, all dogfood tokens are indistinguishable from external traffic.

3. **OpenCode model selector UX** — The doubled `openagents/openagents/khala` in the TUI is cosmetic (OpenCode's `providerId/modelKey` concatenation). **Resolved:** no server-side change needed. If users complain, address via a shorter model key alias or OpenCode display hint.

4. **Free tier vs internal budget** — High-volume internal systems (qa-runner running continuously) will blow through free-tier limits in minutes. Recommendation: route through the paid lane against an internal credit budget with operator exemption, metering still runs but tagged with `consumer: "qa-runner"` in the telemetry record for distinguishable counting. The exemption module (`inference-operator-exemption.ts`) already supports model-level bypass.

5. **Pylon transport readiness** — The `PylonFabricHttpTransport` (`pylon-fabric-http-transport.ts`) exists as a real secret-backed HTTP transport. The remaining gap is a live Pylon gateway route to complete end-to-end smoke. Depends on the Pylon guinea-pig deployment timeline.

6. **Benchmark timing** — The GTM doc says "internal dogfood IS the realistic traffic the benchmark needs." How many days of dogfood traffic needed before the first `decisionGrade: true` benchmark report? Recommend: at least 5 days of continuous qa-runner traffic before arming the real benchmark seam, plus the shapes-extract script from §8 must exist.
