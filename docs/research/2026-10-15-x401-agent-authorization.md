# x401: Proof's agent authorization protocol (2026-10-15)

Proof launched [x401](https://www.x401.id/) today as "a way for AI agents to
prove who they represent and what they're authorized to do"
([launch post](https://www.proof.com/blog/proof-launches-x401-giving-ai-agents-a-way-to-prove-who-they-represent---and-what-theyre-authorized-to-do)).
This note reads the spec and Proof's code, separates what the protocol does
from what the launch says it does, and decides whether OpenAgents should
support it.

Sources, read 2026-10-15:

- The spec repo [github.com/proof/x401](https://github.com/proof/x401) at
  `7e864af` (2026-09-30), cloned to a scratch directory outside this repo and
  deleted after. It holds `spec.md` (v0.2.0, 1,685 lines), the frozen
  `versions/0.1.0/spec.md`, a changelog, and the spec-up renderer. **It has no
  SDKs, examples, or tests.** License: Apache-2.0.
- Proof's SDKs, found in the same GitHub org:
  [`proof/x401-node`](https://github.com/proof/x401-node) at `5c52d55`
  (`@proof.com/x401-node` 0.3.3 on npm) and
  [`proof/proof-vc-common`](https://github.com/proof/proof-vc-common) at
  `8b421b1` (`@proof.com/proof-vc-common` and `-server`), both Apache-2.0.
  `proof/x401-ruby` exists but is empty.
- The sites [x401.id](https://www.x401.id/) and
  [proof.com/x401](https://www.proof.com/x401), and the launch post.
- The repo's 46 issues and PRs, especially
  [#41](https://github.com/proof/x401/issues/41), an independent
  implementer's report.

Nothing was installed or run.

**Short answer.** x401 is a thin, reasonable HTTP wrapper around standards
that already exist (OpenID4VP over the W3C Digital Credentials API, DCQL,
OAuth token exchange). The wrapper is open and small. The launch claims
(agent delegation, signed transaction mandates, offline verification, legal
identity) live in Proof's product, not in the protocol. Verifying Proof's own
credentials today means calling Proof's API for every presentation and paying
per presentation over x402 on EVM chains or a card on file. We should **not**
become an x401 verifier or issuer now. We **should** teach our agents to
recognize an x401 challenge and hand it to the person as a hold, which costs
little and keeps us out of the identity business. Details and an experiment
are at the end.

## 1. What x401 is

HTTP has `401` for "authenticate" and `402` for "pay". x401 adds a third
question: "present a credential that proves X before you can use this route".
X is a claim about the *holder*: over 18, resident of a country, licensed,
a customer of a bank, an employee of a company.

The pitch is aimed at agents. A shopping or banking agent calls a route; the
site wants to know whose agent it is and what that person approved. Proof
frames this as the "Agent Evidentiary Gap", citing Amazon blocking Meta's
Muse shopping agent on 2026-09-21 for not identifying itself.

What the spec actually standardizes is narrower than the pitch:

| The pitch says | The spec (v0.2.0) says |
| --- | --- |
| Agents prove who they represent | The *holder's credential manager* (a wallet) presents a credential to the site. The result is bound to the site, **not to the agent**. Binding the agent is optional and left to other protocols. |
| Agents prove what they're authorized to do | Not specified. "Fully autonomous delegation" is an open question in the spec ([#40](https://github.com/proof/x401/issues/40)); it points at Verifiable Intent or AP2 as future work. |
| Signed transaction terms ("$25,000 wire") | Not in x401. It is OpenID4VP `transaction_data`, which Proof's `proof-vc-server` exposes as "Transaction Templates" (`wire-instructions:v1`). |
| Verified legal identity (IAL2) | Not in x401. It is Proof's credential (`ProofCredentialV1`, SD-JWT VC) from Proof's CA. x401 is issuer-neutral. |
| Revocation | Spec: "Verifier MUST evaluate issuer trust, status, revocation" and Verification Tokens "SHOULD support revocation". No mechanism is defined. |

## 2. The protocol flow

Three headers, each one base64url JSON object:

| Header | Direction | Carries |
| --- | --- | --- |
| `PROOF-REQUEST` | site to agent | The x401 payload: a site-composed Digital Credentials request |
| `PROOF-RESPONSE` | agent to site | A Result Artifact (the wallet's answer, inline or as a URL) or a token |
| `PROOF-RESULT` | site to agent | An error object when the proof failed |

```
Agent  -> GET /protected
Site   -> 401 (any status allowed) + PROOF-REQUEST: <payload>
Agent  -> hand payload.credential_requirements to a wallet:
          navigator.credentials.get(...), a relay to a remote wallet,
          or a site-hosted page the person opens
Wallet -> { protocol, data }  (an OpenID4VP vp_token bound to the site)
Agent  -> GET /protected + PROOF-RESPONSE: <result artifact>
Site   -> 200, or 401 + PROOF-RESULT: <error>
(optional) Agent -> POST token_endpoint (RFC 8693 token exchange)
                    -> short-lived Bearer "Verification Token" for reuse
```

The payload, decoded:

```json
{
  "scheme": "x401",
  "version": "0.2.0",
  "credential_requirements": { "digital": { "requests": [
    { "protocol": "openid4vp-v1-signed", "data": { "request": "<JAR JWT>" } }
  ] } },
  "oauth": { "token_endpoint": "https://bank.example.com/oauth/token" },
  "request_id": "proof-template-financial-customer-v1",
  "satisfied_requirements": ["urn:example:x401:satisfaction:financial-customer:v1"],
  "payment": { "required": true, "scheme_hint": "x402" }
}
```

Points worth knowing:

- **The site composes and signs the request.** The agent must pass it on
  unchanged. Inside the signed JWT are the DCQL query (which credential and
  which claims), the `nonce`, `client_id`, and `expected_origins`. The agent
  is a courier, not the relying party.
- **Credential formats** are whatever DCQL names: `jwt_vc_json`, SD-JWT VC,
  mdoc. x401 defines no format of its own.
- **Issuer trust** is DCQL `trusted_authorities`: OpenID Federation, ETSI
  trusted lists, or X.509 key identifiers. There is **no type for DID-based
  or Nostr-key issuers**; implementer issue
  [#41](https://github.com/proof/x401/issues/41) had to omit trust
  constraints for that reason.
- **Signed vs unsigned requests.** Signed binds the answer to the site and
  the origins it pre-approves. Unsigned binds only to the invoking origin and
  nonce; the spec itself says a site that accepts that is "exposed to
  credential harvesting and relay".
- **Remote fulfillment.** Most agents have no `navigator.credentials`. The
  spec's answer is a site-hosted page the person opens; the agent later
  replays the route with a `credential_result_uri`. This is a human step by
  design.
- **Agent binding** is optional, via Web Bot Auth, mTLS, DPoP, SPIFFE, or a
  DID. Without it, whoever holds a Verification Token can use it.
- **Status code is decoupled.** `PROOF-REQUEST` may ride on a `200`, and an
  HTML `<data value="application/json;x401=proof-required" hidden>` mirror
  exists for chat assistants that drop headers (marked experimental).
- **Payment stays in `402`.** The spec is firm that proof is not payment.

## 3. How it relates to neighbours

| Protocol | Relationship |
| --- | --- |
| **x402 (HTTP 402)** | Sibling. x401 says "who", x402 says "pay". `payment.scheme_hint` can name x402. Sequencing (proof first, pay first, both) is an open question in the spec. Proof's own verify API charges per presentation over x402 with `@x402/evm` (`eip155:*`), i.e. EVM stablecoins. |
| **OAuth 2.x** | Used, not replaced: the optional token leg is RFC 8693 token exchange with a new `subject_token_type`. A Verification Token can ride in `Authorization` or in `PROOF-RESPONSE` beside an existing app token. |
| **Web Bot Auth / HTTP Message Signatures (RFC 9421)** | Complementary. Web Bot Auth says *which bot* is calling; x401 says *what its person can prove*. The spec lists Web Bot Auth as one way to bind the agent. |
| **MCP auth** | MCP authorization is OAuth 2.1 for the *client app* to act for a user on one server. x401 is a per-route claim gate. Proof says it runs an MCP server that relays x401 requests to its wallet for Claude, ChatGPT and others; we could not find its code or listing. |
| **DIDs / VCs** | x401 can carry W3C VCs and can name a DID as an agent identifier, but its issuer-trust vocabulary is X.509 and federation centric. DID issuers fall outside it today. |
| **AP2 / Verifiable Intent** | Named as the place delegation mandates belong. x401 does not define them. |
| **WebAuthn / passkeys** | Reserved for later (`credential_requirements.publicKey`), not specified. |

## 4. Maturity, governance, and who runs what

| Question | Evidence |
| --- | --- |
| Spec status | "Draft", v0.2.0, changed incompatibly from 0.1.0 (renamed headers `PROOF-REQUIRED`/`PROOF-PRESENTATION` to `PROOF-REQUEST`/`PROOF-RESPONSE`, removed the `proof` wrapper). Repo created 2026-05-07, 15 commits, 40 stars, 5 forks. Last spec change 2026-09-30 was a same-day revert. |
| Internal consistency | The changelog and README still name `presentation_requirements` and `presentation_uri`; the spec says `credential_requirements` and `credential_result_uri`. Agent rule 3 still says "process the `proof` object", which 0.2.0 removed. proof.com/x401 still documents 0.1.0 headers. |
| Governance | Editors from Proof and Circle; reviewers from Proof, Visa, MATTR, Okta, OpenAI, Lightspark. CODEOWNERS is `@proof/proof-product`, so Proof approves every change. The post says Proof "intends" to move it to neutral governance. No standards body has it; no IANA registration. |
| License | Apache-2.0 for the spec and both SDKs. |
| SDKs | `@proof.com/x401-node` (about 850 lines of TypeScript: header encode/decode and validation only; it "does not verify credentials"). x401.id advertises `@x401/client` with `fetchWithX401`: **not on npm** (404 on 2026-10-15). No Rust SDK. |
| Independent implementations | One self-reported verifier (Solidus, issue #41). Superfluid Wallet is named as a launch partner. |
| What Proof runs | The issuer (Proof's CA, Kantara IAL2 proofing, HSMs), the cloud wallet, the "agents trust list" (`api.proof.com/.well-known/agents-trust-list`), the verifier enrollment, the detached-signature service, and the MCP server. |

### The lock-in that matters

The spec is open. The credentials that give it value are Proof's, and
Proof's SDK shows how they verify:

> Presentations delivered through x401 arrive with the signature segment of
> both the Issuer-signed JWT and the Key Binding JWT detached. ... `verify`
> and `verifyVPToken` fetch them from `POST /verifiable-credentials/v1/x401-signatures`,
> authenticated with a `private_key_jwt` client assertion ...
> Verifiers can pay for x401 presentations with x402 ... otherwise Proof
> charges their default account payment method.
> ([proof-vc-server README](https://github.com/proof/proof-vc-common))

So a site cannot verify a Proof credential presented over x401 on its own.
It must be an enrolled Proof verifier, call Proof online for every
presentation, and pay Proof per presentation. That is a metering choke point,
and it contradicts the "offline verification" item in the launch post. It is
a product choice, not a protocol requirement: another issuer could ship
whole, offline-verifiable SD-JWTs over the same headers.

The launch post says basic claims (age, country, email) need no sales
process, while higher assurance and signed authorization "requires
registering with Proof".

## 5. Claims vs evidence

| Claim | Evidence | Verdict |
| --- | --- | --- |
| "Open protocol" | Apache-2.0 spec on GitHub | True for the wire format; Proof controls merges |
| "Agents prove who they represent and what they're authorized to do" | Holder presents a credential; agent binding optional; delegation unspecified | Overstated: the protocol proves holder attributes, not agent authority |
| "First in production with verified legal identity for agents" | Proof's IAL2 credential and wallet are live (per Proof); spec is a draft | Plausible for Proof's product; not checkable from outside |
| "Offline verification" | Proof's own SDK needs an online, paid call for detached signatures | Contradicted for Proof credentials today |
| "Works with Claude, ChatGPT, Grok, Muse via MCP" | Not found in Proof's public repos or registries | Unverified |
| `@x401/client` / `fetchWithX401` | npm 404 | Not published |
| "A global payments network is adopting x401" | Unnamed | Unverified |
| FinCEN accepts verifiable credentials for CIP | Cited by Proof for 2026-09-08 | Not checked here; relevant only to US banks |

## 6. What OpenAgents has today

| Area | What exists | Where |
| --- | --- | --- |
| Agent discovery | `robots.txt`, `llms.txt`, `auth.md`, RFC 9727 API catalog, ARD `ai-catalog.json`, MCP server card, A2A agent card, agent skills, WebMCP | `crates/openagents-web/src/agent_ready.rs`, `wellknown.rs`; [readiness](../agents/readiness.md) |
| DNS | DNS-AID SVCB records live for `_a2a._agents` and `_mcp._agents`, a TXT at `_catalog._agents`; DNSSEC on (DS record published) | Cloudflare DNS, checked with `dig` 2026-10-15 |
| Readiness | Is It Agent Ready level 5 "Agent-Native"; Web Bot Auth "not applicable: we do not send signed bot traffic" | [readiness](../agents/readiness.md), #11629 |
| Agent self-signup | Anonymous by design: proof-of-work (22 bits), per-IP and per-/24 limits, global cap; one `oak_` key scoped `responses`, `models:read`, `usage:read`; a person can claim and revoke it | #11630, [self-registration](../agents/self-registration.md), `crates/openagents-web/src/agent_accounts.rs` |
| Free tier | 20 small-model requests a day, then a `402` naming a Lightning top-up | `/auth.md` |
| Human auth | Our own account service; GitHub first; Nostr-only accounts first-class; OAuth 2.1 for MCP apps; never auto-link by email | [docs/auth](../auth/README.md), `crates/openagents-web/src/oauth.rs` |
| Payments | Bitcoin only: x402 v2 `exact` on Lightning (`lnbtc`), L402, MPP on Lightning, Taproot Assets later; **no USDC, Base, Solana, EVM, Tempo** | `crates/x402`, [agent payments](../payments/agent-payments.md) |
| Agent identity and budgets | NIP-98 signed requests (planned on the API, #11148 closed for V1); BuyerAttestation (NIP-SOV + NIP-CAP `spend` grant); AP2 mandates (#11145 closed for V1) | [agent payments §5](../payments/agent-payments.md) |
| Human holds | Payments, deploys, secrets, merges and DNS are floors that ask the person; with nobody there the step is held and handed back | [Coder permissions](../coder/permissions.md), `crates/coder-new/src/risk_policy.rs` |
| Decisions | Typed judgments through `/v1/decisions` | [Decision API](../decision-models/api/decision-api.md) |

There is no SD-JWT, OpenID4VP, or DCQL code in the repo.

## 7. Should we support it?

### As a verifier (agents calling our API present x401 credentials): not now

- **No route needs it.** Our gates are a key and a Lightning payment. The
  agent tier is anonymous on purpose: proof of work and hard caps, not
  identity. Asking for an ID credential would move against the owner's
  2026-10-14 decision and against Nostr-only accounts being first-class.
- **The only issuer that matters is Proof**, and verifying its credentials
  needs Proof enrollment, an online call per presentation, and payment by
  EVM x402 or card. The EVM path is ruled out by our Bitcoin-only rails; the
  card path is a per-verification vendor bill and a hard dependency in the
  request path.
- **Privacy cost.** We would receive and have to store or discard legal
  names, birth dates and nationalities. That brings retention, breach and
  KYC-adjacent duties we do not have today.
- **The useful case, later:** if a market action needs a real-world claim we
  cannot get another way (an age gate on some content, a merchant or payout
  recipient who must pass KYC), an x401 gate on that one route is the right
  shape. Do it then, issuer-neutral, with trust anchors in config, and only
  with issuers whose credentials verify offline.

### As an issuer: no

We are not an identity proofer and should not become one. The one credential
we could honestly issue is "this agent account is claimed by an OpenAgents
person" (no PII). Nobody asks for it yet, and DCQL has no way to name a
Nostr-key issuer, so it would not be discoverable anyway.

### As a holder/relay (our agents meet x401 on other sites): yes, small

This is where demand will come from first: sites gating agents. Coder,
Solver and the web chat fetch pages and call APIs. When one answers with
`PROOF-REQUEST`, today the agent sees an unexplained `401` or a page and
gives up or guesses.

The right behaviour matches our holds: an identity disclosure is
hard to undo, so it is a person's step.

1. Recognize `PROOF-REQUEST` (and the `<data>` mirror) and decode it.
2. Read what is asked from the DCQL query (which claims, which issuers) and
   say it plainly: "example.com asks you to prove you are over 18 with a
   Proof credential."
3. Never fulfil it automatically and never hold credentials. Hand the person
   the site's fulfilment link as a hold (desktop or phone approval).
4. When the person finishes, retry the route with `PROOF-RESPONSE`
   carrying the reference the site gave.
5. Keep any Verification Token in memory for that site and route only; never
   write it to chat history, traces, or logs.

If the site also answers `402` with an EVM-only `accepts`, we say we cannot
pay there; we do not add an EVM rail.

## 8. Where it would land and how big

| Piece | Place | Size |
| --- | --- | --- |
| Wire types: payload, Result Artifact, Token Object, Error Object; base64url (no padding) JSON; the "one field line, no commas" rule; JWT payload peek (no signature check) for display | new `crates/x401` (no network, no crypto), mirroring `@proof.com/x401-node` | about 400 lines of Rust plus fixtures from spec Appendix A and the examples |
| Detection in Coder's `web_fetch` | `crates/coder-new/src/web_tools.rs`: return a typed "proof required" result instead of page text | about 80 lines |
| The hold | the existing approval card and risk policy (`crates/coder-new/src/approval.rs`, `risk_policy.rs`): a new "identity disclosure" floor that a file can never lower | about 60 lines |
| CLI parity | `openagents x402 fetch` (`crates/openagents-cli/src/x402.rs`) already handles `402`; teach it to report `PROOF-REQUEST` | about 40 lines |
| Verifier (later, if ever) | gateway middleware beside the `402` path in `crates/gateway`, with an SD-JWT/OpenID4VP verifier crate | weeks, plus a privacy review |

## 9. Privacy and policy

- **Disclosure is a hold.** Add "share identity attributes with a site" to
  the floors in [Coder permissions](../coder/permissions.md). Update
  `INVARIANTS.md` in the same change if it becomes a floor.
- **We are a courier.** We never see or store the presented credential when
  fulfilment happens on the site's page; if a by-value result passes through
  us, it is not logged or traced.
- **Correlation.** A Proof credential carries a stable key; using it across
  sites links the person. The prompt to the person should say which claims
  are asked, from which issuer, and that the site may keep a signed record.
- **Unsigned requests** are phishable by the spec's own admission. Show the
  person the request's origin and refuse to relay unsigned requests by
  default.
- **Bitcoin-only rails.** If a gated site wants EVM x402 payment, we stop.
  We do not pay Proof or anyone over EVM to verify.
- **Semantic routing rule.** Detection is protocol parsing of a named header
  and a schema-tagged element, not keyword matching on page text, so it is
  within the repo's routing rules.

## 10. Experiment

Goal: learn whether x401 challenges show up for our agents and whether the
hold flow is understandable, without becoming a verifier.

1. Build `crates/x401` (wire types only) with the spec's examples as
   fixtures, plus the spec inconsistencies in §4 (field names, the leftover `proof` rule) as named tests.
2. Stand up a local test verifier route in the gateway's test harness that
   answers `PROOF-REQUEST` with an unsigned, fake-issuer request (no real
   credentials, no network), and accepts a canned `PROOF-RESPONSE`.
3. Wire detection into `web_fetch` and the hold card.
4. Point Coder at the test route and, separately, at x401.id's public demos
   and the Solidus demo (`auth.solidus.network/v1/x401/demo`) in read-only
   mode: detect and describe, never fulfil.

Acceptance checks:

- [ ] `crates/x401` round-trips every example payload, Result Artifact,
      Token Object and Error Object in spec v0.2.0, and rejects a doubled
      header line and a comma-joined value.
- [ ] `web_fetch` on the test route returns "proof required" with the
      claims and issuers named, and no page text.
- [ ] With nobody at the terminal the step is held and handed back; with a
      person, the card shows the site, the claims, and the issuer, and Deny
      leaves the run going without the page.
- [ ] After Allow and a simulated fulfilment, the retry carries
      `PROOF-RESPONSE` and gets `200`.
- [ ] No credential, result, or Verification Token appears in traces,
      chat history, or logs (grep the run's artifacts).
- [ ] Against the public demos, Coder detects and describes the challenge
      and makes no POST to any wallet, issuer, or Proof endpoint.
- [ ] No new payment rail, and no new dependency on a Proof service.

Revisit the verifier question when either a route of ours needs a
real-world claim, or a second issuer ships offline-verifiable credentials
over x401.
