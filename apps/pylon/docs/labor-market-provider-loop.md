# Labor Market Provider Loop

Issue #4730 (epic #4726); promise `labor.nostr_negotiation_market.v1`
(registry `2026-06-10.25`). Protocol: `docs/nips/LBR.md` (workspace
root). Roadmap: `docs/labor/2026-06-10-open-agent-labor-market-roadmap.md`.

The Pylon's NIP-LBR negotiation lane: watch agentic-coding jobs
(kind 5934) on the owned relay, quote the ones this device is
capability-true for, execute on acceptance through the labor runtime on
the contributor's **own** agent, deliver output-only results
(kind 6934).

## Negotiation discipline

LBR requests are **never auto-executed**. The lane routes ahead of the
generic provider flow in `runProviderJobOnce`:

1. **Quote.** A kind-5934 request is evaluated by
   `evaluateLbrRequestForQuote`: opt-in check, job-kind allowlist,
   quote-once, capability truth (`requiredCapabilityRefs` must all be
   declared by this Pylon's runtime state), concurrency bound, and the
   contributor's price must fit the request's bid. A passing request
   gets one kind-7000 quote with the amount and refs; everything else
   gets a typed refusal and no relay traffic.
2. **Win.** A kind-7000 LBR acceptance addressed to this provider
   (carrying the requester's escrow receipt ref) triggers execution of
   the previously quoted job — acceptance without a quote is refused;
   acceptance addressed elsewhere is ignored.
3. **Execute.** Admission still applies: labor first-run operator
   approval (`pylon provider approve-labor`) is required before any
   machine runs its first market job. The workspace is bounded under
   the Pylon cache. The `claude_code` lane runs through the Claude
   Agent SDK runtime (`makeClaudeAgentLaborRuntime`): bounded sandboxed
   session, workspace-escape denial, settings isolation, turn and
   wall-clock budgets — not a CLI shell-out. Other agent kinds use the
   configured labor runtime.
4. **Verify, then deliver.** The request's `verificationCommandRef`
   maps through the bounded command registry
   (`LABOR_MARKET_VERIFICATION_COMMANDS`); unknown refs are refused.
   The command must pass in the workspace or **no result is published**
   (`refusal.labor_market.verification_failed`) — the provider never
   ships failing work. A passing job publishes the kind-6934 result
   with output-only refs and a platform closeout ref.

## Configuration (contributor-owned, opt-in)

`laborMarket` section in the Pylon config file, or env overrides:

```json
{
  "laborMarket": {
    "autoQuote": true,
    "priceMsats": 1500000,
    "maxConcurrentJobs": 1,
    "agentKind": "claude_code"
  }
}
```

- `autoQuote` defaults to **false** — quoting is opt-in
  (`PYLON_LABOR_MARKET_AUTO_QUOTE=1` to enable by env).
- `priceMsats` is the contributor's price (`PYLON_LABOR_MARKET_PRICE_MSATS`);
  the platform sets no prices.
- The local state lives in `labor-market-state.json` (quote records,
  public-safe asserted on every write).

## Boundaries

The provider never self-accepts and never sees requester funds — it
sees an escrow receipt ref. Work runs on the contributor's own agent,
own credentials, own machine; zero provider-auth material in events,
artifacts, or state (structurally scanned). The relay is transport, not
authority; settlement truth comes from the platform's escrow/receipt
systems (#4729, #4732). Raw session material, diffs, and logs stay
on-device; only refs travel.
