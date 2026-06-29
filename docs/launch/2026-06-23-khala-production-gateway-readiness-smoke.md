# Khala Production Gateway Readiness Smoke

Date: 2026-06-23

Issue: OpenAgentsInc/openagents#6107

Purpose: prove the live OpenAI-compatible Khala gateway can advertise at least
one servable model, complete an authenticated `openagents/khala-mini` request,
and expose a dereferenceable receipt without printing secrets.

This runbook is a launch gate, not a runtime authority. The production
contracts remain the gateway routes, model catalog, metering receipts, and
owner-armed Worker configuration.

## What This Smoke Proves

- `GET /v1/gateway/readiness` is reachable and reports
  `servableModelCount > 0`.
- `GET /v1/models` includes the intended Khala model id.
- `POST /v1/chat/completions` succeeds with an authenticated agent token.
- The completion response carries the `openagents` disclosure block.
- The response exposes a public receipt pointer through either
  `openagents.receipt_url` or `openagents.telemetry.detailRef`.
- The receipt pointer dereferences with HTTP 2xx.

## Owner-Gated Inputs

The smoke refuses to run the authenticated completion unless the operator passes
`--approve-live-spend`. That flag is deliberate because a production completion
can meter credits. The command still never prints bearer tokens.

Required for the full smoke:

- `OPENAGENTS_AGENT_TOKEN`: funded agent token for the smoke account.
- Armed provider lane credentials in the deployed Worker.
- `INFERENCE_GATEWAY_ENABLED=true` in the deployed Worker.

Optional:

- `OPENAGENTS_BASE_URL`: defaults to `https://openagents.com`.
- `OPENAGENTS_KHALA_SMOKE_MODEL`: defaults to `openagents/khala-mini`.
- `OPENAGENTS_KHALA_SMOKE_PROMPT`: defaults to a short smoke prompt.
- `KHALA_GATEWAY_SMOKE_APPROVE_LIVE_SPEND=true`: equivalent to
  `--approve-live-spend`.

## Readiness-Only Check

Use this when the operator wants to check catalog arming without spending:

```bash
cd /Users/christopherdavid/work/openagents/apps/openagents.com
bun run smoke:khala:gateway-readiness -- --readiness-only
```

This mode checks readiness and model discovery only. It does not satisfy the
full production launch gate because no metered completion or receipt is proven.

## Full Production Smoke

```bash
cd /Users/christopherdavid/work/openagents/apps/openagents.com
OPENAGENTS_AGENT_TOKEN="<redacted funded token>" \
  bun run smoke:khala:gateway-readiness -- --approve-live-spend
```

Expected outcome:

- command exits `0`;
- JSON output has `"ok": true`;
- checks include:
  - `readiness_endpoint_200`;
  - `readiness_has_servable_model`;
  - `models_endpoint_200`;
  - `models_lists_requested_khala_model`;
  - `completion_endpoint_200`;
  - `completion_has_openagents_block`;
  - `completion_echoes_requested_model`;
  - `completion_has_dereferenceable_receipt_ref`;
  - `receipt_endpoint_200`.

If the command exits non-zero, keep Khala in preview/dogfood posture and use the
failed check name as the first triage pointer. Do not paste raw tokens or
provider payloads into issues, docs, commits, or chat.

## Issue Closeout Rule

Close OpenAgentsInc/openagents#6107 only after a full production smoke has
passed against the intended production base URL and the issue comment includes:

- base URL;
- model id;
- readiness status;
- servable model count;
- receipt URL or public receipt ref;
- commit hash containing this smoke tooling.
