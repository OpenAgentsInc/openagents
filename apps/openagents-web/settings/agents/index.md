# Agents

Agents act for you with their own keys, never yours. This page lists your agents and what each may do. Each agent can do only what you allow, until its grant ends.

## Make an agent

- Run `openagents agent create` on your computer. You get an `oa_key_` API key once.
- Or an agent asks: it shows you a link like `https://openagents.com/settings/agents#request=ABCD2345`. Open it, read what it asks for, set its caps, then approve or deny.

## What a grant holds

- What the agent may do, in words: read your profile, or spend from your wallet. Nothing else is allowed.
- Spend caps you set: sats a day and sats in one payment.
- The addresses it may act from, if you limit them.
- When it ends: 90 days unless you choose 7, 30, 180 or 365. A grant never lasts more than a year.

Approving an agent that can spend, or giving one more, needs a login with your key or passkey in the last five minutes.

## Change it

- **Revoke** stops an agent at once, on every route. **Revoke all** stops every agent.
- **New API key** gives the agent a new key, shown once, and ends its old keys.
- **Log out everywhere** with agents pauses them; **Resume** turns one back on.

An account has at most 32 agents at once. Revoked agents leave the list; the audit log keeps every grant and every use.

Agents never log in and never read your data without a grant. See [identity and login methods](https://github.com/OpenAgentsInc/openagents/blob/main/docs/auth/identity-and-methods.md#agents).
