# Sarah Vercel Production Runbook

Target: `https://sarah.openagents.com`

## Owner Gates

- Create or grant access to the Vercel project for `OpenAgentsInc/sarah`.
- Add DNS for `sarah.openagents.com`.
- Set production secrets:
  - `AI_GATEWAY_API_KEY`
  - `SARAH_OPENAGENTS_CRM_MCP_TOKEN`
  - `SARAH_OPENAGENTS_OPERATOR_TOKEN`
  - `SARAH_EMAIL_WEBHOOK_TOKEN`
  - Resend key once the AI SDK 7 compatible email adapter is selected.
- Confirm AI Gateway dashboard caps and alerts for realtime token spend.
- Run a real browser voice smoke after the production deploy is live.

## Production Env

Required:

```sh
AI_GATEWAY_API_KEY=...
SARAH_REALTIME_VOICE=shimmer
SARAH_PUBLIC_BASE_URL=https://sarah.openagents.com
SARAH_OPERATOR_TOKEN=...
SARAH_EMAIL_WEBHOOK_TOKEN=...
```

OpenAgents integration, armed only when the owner approves live writes:

```sh
SARAH_OPENAGENTS_BASE_URL=https://openagents.com
SARAH_OPENAGENTS_LIVE_WRITES=1
SARAH_OPENAGENTS_CRM_MCP_TOKEN=...
SARAH_OPENAGENTS_OPERATOR_TOKEN=...
SARAH_OPENAGENTS_CHECKOUT_ENDPOINT=...
```

Token guardrails:

```sh
SARAH_ALLOWED_ORIGINS=
SARAH_REALTIME_RATE_WINDOW_MS=60000
SARAH_REALTIME_MAX_TOKENS_PER_IP=20
SARAH_REALTIME_MAX_TOKENS_PER_PROSPECT=10
SARAH_REALTIME_MAX_ACTIVE_SESSIONS_PER_IP=5
SARAH_REALTIME_MAX_ACTIVE_SESSIONS_PER_PROSPECT=2
SARAH_REALTIME_SESSION_TTL_MS=120000
SARAH_REALTIME_DAILY_TOKEN_CAP=500
SARAH_REALTIME_SPEND_ALERT_THRESHOLD=0.8
SARAH_REALTIME_SPEND_ALERT_WEBHOOK_URL=...
```

Leave `SARAH_ALLOWED_ORIGINS` empty for the normal same-origin Vercel
deployment. Set it only when a preview/staging host must be allowed.

## Model Pin

The production realtime token route pins:

- Primary: `openai/gpt-realtime-2`
- Documented swap: `xai/grok-voice-think-fast-1.0`

The public browser session config pins the feminine voice:

- `SARAH_REALTIME_VOICE=shimmer`

## Verification

Before deploy:

```sh
pnpm lint
pnpm build
SARAH_EVAL_BASE_URL=http://localhost:3000 pnpm test:s12-evals
```

After deploy:

```sh
SARAH_PRODUCTION_BASE_URL=https://sarah.openagents.com pnpm smoke:production
SARAH_EVAL_BASE_URL=https://sarah.openagents.com pnpm test:s12-evals
```

To include a real AI Gateway realtime token mint:

```sh
SARAH_PRODUCTION_BASE_URL=https://sarah.openagents.com \
SARAH_PRODUCTION_SMOKE_MINT_TOKEN=1 \
pnpm smoke:production
```

Then complete a browser voice smoke:

1. Open `https://sarah.openagents.com`.
2. Confirm the disclosure is visible before connect.
3. Connect and grant microphone access.
4. Confirm Sarah speaks with the configured feminine voice.
5. Ask for an unruled discount and confirm Sarah refuses or escalates.
6. Ask for a configured credit-package quote and confirm the quote traces to
   deal-rule refs.

Do not close S-11 until DNS, Vercel env, Gateway caps, and the real voice
smoke are all confirmed.
