# Omega Nostr self-provisioning: abuse bounds

Date: 2026-07-28.
Surface: `POST /api/omega/auth/session`.
Owner file: `apps/openagents.com/workers/api/src/auth/omega-nostr-self-provision.ts`.

## 1. What changed

Before this change the endpoint accepted exactly one Nostr public key. The key
was the configured `SARAH_NOSTR_OWNER_PUBKEY`. The endpoint then minted a
session for the primary OpenAgents administrator account. Each other install
signed a correct NIP-98 proof and received a 401 response. The 401 response was
permanent. This behavior was an owner backdoor. It was not a free tier.

After this change each valid NIP-98 proof can mint a session. The session is
bound to a user record that is keyed on the signing public key. The user
identifier is `nostr:<pubkey>`. The owner key keeps the previous behavior.

The new behavior is off by default. The environment flag
`OMEGA_NOSTR_SELF_PROVISION_ENABLED` arms it.

## 2. The security boundary did not change

The NIP-98 checks are the same checks. The handler keeps the event kind check,
the Schnorr signature check, the empty content check, the `u` tag check against
the request URL, the method tag check, the empty payload hash check, the
integer `created_at` check, the 60-second clock skew check, and the one-time
proof check.

The handler removed one term only. The removed term was the equality test
between the event public key and the configured owner public key. That term was
an allowlist of one install. It was not a cryptographic control.

## 3. What a new install receives

A self-provisioned install receives one `users` row with `kind='human'` and one
`auth_identities` row with `provider='nostr'`. It receives no administrator
email address, no team membership, no entitlement, and no scope.

Administrator authority in this API is an email allowlist. The synthetic
address for a self-provisioned install is `<pubkey>@nostr.invalid`. The
`.invalid` top-level domain is reserved by RFC 2606. That address cannot
receive a sign-in code. That address cannot match the administrator allowlist.

## 4. The bounds

| Bound | Default | Environment variable |
| --- | --- | --- |
| Kill switch | off | `OMEGA_NOSTR_SELF_PROVISION_ENABLED` |
| New accounts for each client IP for each hour | 3 | `OMEGA_NOSTR_SELF_PROVISION_IP_HOURLY_LIMIT` |
| New accounts for the deployment for each 24 hours | 200 | `OMEGA_NOSTR_SELF_PROVISION_GLOBAL_DAILY_LIMIT` |
| Served tokens for each `nostr:` identity for each day | 1000000 | `OMEGA_NOSTR_SELF_PROVISION_DAILY_TOKEN_CEILING` |
| Session mints for each public key for each hour | 20 | not configurable |
| Session mints for each client IP for each hour | 60 | not configurable |

The counters are durable. They are stored in the owned key-value store that
Cloud SQL backs. A restart of the service does not clear them.

The rate limit runs after the full NIP-98 verification. Unsigned traffic
therefore cannot consume the shared global budget.

The creation budget is charged only when the public key has no user record. A
returning install charges the mint budget only. Many installs behind one
network address translation gateway are therefore not blocked after the third
device.

## 5. What an attacker extracts in one hour

Assume the flag is on and the defaults are unchanged.

- One source address gives 3 new identities for each hour. Each identity has a
  daily allowance of 1000000 tokens. The result is 3000000 tokens for each
  hour, and 72000000 tokens for each day.
- Unlimited source addresses give 200 new identities for each 24 hours. The
  result is a ceiling of 200000000 tokens for each 24 hours.
- The daily ceiling is the only aggregate bound. Key pairs are free to make.
  Source addresses are cheap to rent. The per-address bound is friction. It is
  not an identity bound.

Two weaknesses are honest and are not repaired here.

- The `x-forwarded-for` first hop is prependable. A client can therefore rotate
  the value that the per-address bucket uses. The Google front end appends the
  real peer address, so a future repair can read the last trusted hop instead.
- The key-value store has no atomic increment. The read-modify-write counter
  can undercount during a burst of concurrent requests inside one window. The
  counters bound sustained abuse. They do not bound one instantaneous burst.

Set `OMEGA_NOSTR_SELF_PROVISION_GLOBAL_DAILY_LIMIT` to `0` to stop each new
account while each existing install continues to work. Set
`OMEGA_NOSTR_SELF_PROVISION_ENABLED` to `false` to return the endpoint to the
previous owner-only behavior. Each control takes effect on the running revision
after an environment update. Neither control needs a new container image.

## 6. Two larger, pre-existing holes

The bounds above are correct for this endpoint. They are not the largest risk
in this area. Two conditions already exist on `main` and already permit the
same class of abuse without this endpoint.

### 6.1 `POST /api/agents/register` is unauthenticated

`apps/openagents.com/workers/api/src/index.ts` routes `/api/agents/register` to
`handleProgrammaticAgentRegistration`. That handler needs no credential. Each
call creates a new user record and returns a new `oa_agent_` token. The token
satisfies `requireHostedComputeActor`.

Anonymous account creation against owner-funded hosted compute is therefore
already possible. It has no kill switch, no per-address limit, and no global
ceiling. The advertised limit in `agent-rate-limit-policy.ts` only writes
response headers. It counts nothing and refuses nothing.

### 6.2 The hosted Gemini proxy had no ceiling

`POST /api/provider-accounts/google-gemini/models/{model}:streamGenerateContent`
sends the owner key to Google for each authorized caller. Before this change the
handler read no quota, no grant reference, and no ceiling. Metering ran after
the response through `waitUntil`. Nothing read the resulting ledger.

The `dailyTokenCeiling` value of 1000000 was therefore advisory. The builtin
grant returned it in the response body and wrote it to a column. No code
compared it against real usage.

This change enforces that ceiling for `nostr:` identities only. The narrow
scope is deliberate. A ceiling for each identity class would cut off the owner
Pylon runners and the existing GitHub, email, and agent callers during live
work. Self-provisioned installs are a new class. They have no live workflow to
break. They are also the only class that a stranger can create through this
change.

The ceiling reads the same `token_usage_events` ledger that the proxy writes.
The number is exact for each completed request. It is not a reservation. A
burst of concurrent requests that starts before any of them settles can
overshoot the ceiling by approximately one burst of tokens.

The ledger read fails closed. If the database read raises an error, the request
fails and the owner key is not sent to Google. A self-provisioned install
therefore stops during a database outage. That result is deliberate for a
spend path.

### 6.3 Decisions for the owner

1. Decide whether `/api/agents/register` stays unauthenticated. It is a larger
   version of the door that this change opens under bounds.
2. Decide whether the daily token ceiling becomes binding for each identity
   class on the hosted Gemini proxy, and which value each class receives.
3. Decide the value of `OMEGA_NOSTR_SELF_PROVISION_GLOBAL_DAILY_LIMIT` against
   an acceptable daily spend on Gemini Flash.

## 7. Test evidence

- `apps/openagents.com/workers/api/src/auth/omega-nostr-self-provision.test.ts`
  covers the kill switch, the identity derivation, the environment overrides,
  each rate-limit bucket, the window rotation, the absence of raw identifiers
  in the stored keys, and the stated arithmetic bound.
- `apps/openagents.com/workers/api/src/auth/omega-nostr-session.test.ts` covers
  the preserved owner path, the self-provisioned path, the disarmed path, each
  remaining NIP-98 rejection, the replay rejection, the 429 response, and the
  503 response.
- `apps/openagents.com/workers/api/src/provider-account-gemini-free-tier-ceiling.test.ts`
  covers the enforced ceiling, the environment override, and the unchanged
  behavior for the GitHub, email, and agent identity classes.
