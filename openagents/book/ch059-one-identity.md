# 59. One identity

> **Status: partly implemented.** The `#authority` identity provisioning policy
> ([`coax/web/api/auth-identity-policy.coax`](../../coax/web/api/auth-identity-policy.coax)),
> Sans-I/O WorkOS hybrid `acct_` PKCE and multi-org state machine
> ([`coax/lib/workos/flow.coax`](../../coax/lib/workos/flow.coax), `#14099`),
> and Nostr NIP-05/NIP-06 key tooling
> ([`coax/lib/nostr/nip05.coax`](../../coax/lib/nostr/nip05.coax),
> [`nip06.coax`](../../coax/lib/nostr/nip06.coax)) run on `cpu`, grounded by
> [`openagents/examples/one-identity-policy.coax`](../examples/one-identity-policy.coax)
> and [`openagents/examples/one-identity-policy.out`](../examples/one-identity-policy.out),
> and are consumed by [`coax/web/api/auth-identity.c`](../../coax/web/api/auth-identity.c),
> [`apps/openagents-cli/account.coax`](../../apps/openagents-cli/account.coax),
> and [`apps/openagents-web/nostr-lab-keys.coax`](../../apps/openagents-web/nostr-lab-keys.coax).
> Remaining: end-to-end 72-hour recovery cancellation UI and cross-surface
> profile pages unifying web, native apps, and Metaverse.

## Premise

Authors, checkers and payers have appeared as "a key". This chapter makes that
concrete: one identity that is yours on the web, in every app and in the
OpenAgents Metaverse, with keys for your devices and your agents under it.

## Draft

Episode 289 promises reputation, identity and payments "all figured out",
once, for every plugin. Identity comes first, because the other two attach to
it.

On the network, your identity is a key pair. The public half is your name. The
private half signs what you publish: a release, a check, an approval. Nobody
can take a key from you, and a record you signed stays yours on any relay.

On our service, your identity is an account. It is an opaque `acct_...` id in
our own Cloud SQL Postgres database, and the ways you prove you are that
account attach to it: an email address, a social OAuth or Enterprise SSO login,
a Nostr key, or a first-party WebAuthn passkey. Under the locked **WorkOS
hybrid `acct_` spine** (`#14097`–`#14103`,
[`docs/auth/2026-11-26-workos-coax-sdk.md`](../../docs/auth/2026-11-26-workos-coax-sdk.md)),
WorkOS mediates human login, social OAuth (Google, GitHub, Apple), Enterprise
SSO, and Optional Organizations behind our **Custom Coax UI** (no Hosted
AuthKit redirect), while our own `acct_...` spine stays canonical for all
product tables, Nostr-only accounts (D8), self-custodial wallets, NIP-VAULT
backups, Lightning Addresses, and sat-capped agent grants. WorkOS data is
mirrored into our `workos` Postgres schema by a pure Coax/C Events API poller
and verified webhook handler inside `oa-api` and backed up in our locked dumps.
Every WorkOS login enters at `L1` and receives an OpenAgents opaque session
(`__Host-oa_session` on web, device-bound `oa_at_`/`oa_rt_` on native);
wallet backups, spend limits, key removal, and agent grants still require our
own first-party Passkey or Nostr `L2`/`L2-fresh` step-up. There are no
passwords, and we never collect a phone number (D3). Chapter 5 builds the
account.

[Episode 207](../../docs/transcripts/207.md) is the first design: a seed
phrase you write down, with your Nostr key and your wallet key both derived
from it. The owner narrowed that on 2026-11-11: one seed is an opt-in for
people and the default for agents.

One key on every device would be a risk, since losing a phone would lose
everything. So each device gets its own key, which never leaves its hardware
and is linked to your account by a session you already hold. Your agents get
keys of their own. Both are built. An agent
signs its own work and can hold its own wallet. What it may do in your name is
written in a grant you approve and can revoke: scopes, spend caps and an
expiry.

A readable name helps people find you. A name at a domain maps to your key,
and anyone can check the mapping. The same name can be your Lightning Address
(chapter 32).

Many people will arrive without knowing what a key is. They log in with a
code sent to their email, and add a passkey or a key when they want one. If
you lose every strong way in, recovery takes 72 hours and you can cancel it
from any channel. No member of staff can skip the wait.

The command line already keeps an identity this way, in a key file on your
computer.

## What we build

A plan for what this chapter ships once the draft is approved:

- One account across the web, the apps and the Metaverse, with a Nostr key as a
  first-class way in.
- Device keys bound to hardware, and agent keys that act for you under a
  grant you can revoke.
- Readable names that map to keys, and a profile that shows your work.

It builds on what exists today:

- [`docs/auth/2026-11-26-workos-coax-sdk.md`](../../docs/auth/2026-11-26-workos-coax-sdk.md) (locked WorkOS hybrid `acct_` architecture, `#14097`–`#14103`)
- [`coax/lib/workos/`](../../coax/lib/workos) and [`tools/oagen-coax/`](../../tools/oagen-coax) (Sans-I/O `#authority` Coax WorkOS SDK & OpenAPI emitter)
- [`coax/lib/nostr/keys.coax`](../../coax/lib/nostr/keys.coax)
- [`coax/lib/nostr/nip06.coax`](../../coax/lib/nostr/nip06.coax)
- [`coax/lib/nostr/nip05.coax`](../../coax/lib/nostr/nip05.coax)
- [`coax/lib/nostr/nipkeys.coax`](../../coax/lib/nostr/nipkeys.coax)
- [`coax/lib/nostr/nip46.coax`](../../coax/lib/nostr/nip46.coax)
- [`coax/lib/access`](../../coax/lib/access)
- [`apps/openagents-cli/nostr.coax`](../../apps/openagents-cli/nostr.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Account and its authenticators | `openagents.account.v1`, `openagents.credential.v1` ([spec 15.1](../../docs/protocol/openagents-protocol-spec.md#151-openagentsaccountv1), [spec 15.2](../../docs/protocol/openagents-protocol-spec.md#152-openagentscredentialv1)) | HTTP | The login is built with hand-written JSON ([docs/auth](../../docs/auth/README.md)). The spec's sketch predates it; the [protocol review](../../docs/protocol/2026-11-11-protocol-review.md) orders its rewrite |
| Agent account, key and grant | `openagents.account.v1alpha1` ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, as binary protobuf | In use ([chapter 5](ch005-logging-in-when-a-send-needs-it.md)) |
| A key's name and its owner | `openagents.identity.v1` `KeyIdentity`, `OwnerAttestation` ([spec 15.4](../../docs/protocol/openagents-protocol-spec.md#154-openagentsidentityv1)) | Nostr: kind 0, NIP-05, and an `auth` tag (NIP-OA) | Spec sketch; no `.proto` file. `coax/lib/nostr/nip05.coax` and `coax/lib/nostr/nipoa.coax` handle the events |

### The chapter's check

- **Source:** [`coax/web/api/auth-identity-policy.coax`](../../coax/web/api/auth-identity-policy.coax),
  [`coax/lib/workos/flow.coax`](../../coax/lib/workos/flow.coax),
  [`coax/lib/nostr/nip05.coax`](../../coax/lib/nostr/nip05.coax),
  [`openagents/examples/one-identity-policy.coax`](../examples/one-identity-policy.coax),
  and [`openagents/examples/one-identity-policy.out`](../examples/one-identity-policy.out)
  (remaining: end-to-end 72-hour email recovery timer UI case).
- **Trace:** plans identity provisioning (`AI_CREATE` for a live keyless
  account, existing `AI_LINKED`/`AI_MANAGED` preserved, `AI_SKIP` for frozen),
  runs WorkOS PKCE authorize → callback (zeroing transient verifier/state) →
  HTTP 403 `organization_selection_required` → org selection → authenticated
  `user_oa`/`org_oa`, and verifies `bob@example.com` via NIP-05 well-known URL
  against Bob's pubkey (`check=0`).
- **Refusal:** frozen accounts refuse provisioning (`AI_SKIP`); limited
  sessions refuse account project alias (`alias=false`); OAuth state mismatch
  fails closed and zeroes the PKCE verifier; NIP-05 refuses HTTP 301 redirects
  (`N5_REDIRECT`), pubkey mismatches (`N5_MISMATCH`), and uppercase identifiers
  (`N5_IDENTIFIER`).
- **Independent check:** `coax/bin/coax run openagents/examples/one-identity-policy.coax`
  and `--release` match [`openagents/examples/one-identity-policy.out`](../examples/one-identity-policy.out),
  NIP-06 derivation matches published vectors in
  [`coax/lib/nostr/nip06.coax`](../../coax/lib/nostr/nip06.coax), and NIP-OA
  `auth` tags verify under [`coax/tests/nostr/nipoa_reference.py`](../../coax/tests/nostr/nipoa_reference.py).

Planned paths: none. Identity extends the login in
[`coax/web/api/`](../../coax/web/api) (chapter 5) and the keys in
[`coax/lib/nostr/`](../../coax/lib/nostr).

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 3:30, identity figured out
  - [Episode 82, *Nostr Login*](../../docs/transcripts/082.md): logging in with Nostr
  - [Episode 207, *Your Keys, Your Coins, Your Identity*](../../docs/transcripts/207.md): keys, coins and identity
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 28:30, a key that lets an agent act for you within limits
  - [Episode 211, *Autopilot Online*](../../docs/transcripts/211.md): a personalized identity
- **Old code at `coder-v1.0.0`, crates:** `oa-auth`, `openagents-login`, `oa-tokens`, `tenancy`, `oa-connections`, `coder-connect`
- **NIPs:** [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-VAULT](../../nostr/nips/openagents/NIP-VAULT.md)
- **Also:**
  - Nostr From Scratch covers the network of keys, and Bitcoin From Scratch covers signing for agent treasuries. See [the books' shared topics](../../docs/books/README.md).
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P14

## Open questions for the owner

1. When does a person who logged in by email get a Nostr key for signing
   public records, where is it made, and how do they take it with them?

## Source listing

Run the Chapter 59 example in [`openagents/examples/one-identity-policy.coax`](../examples/one-identity-policy.coax) (committed transcript [`openagents/examples/one-identity-policy.out`](../examples/one-identity-policy.out)):

<!-- source: openagents/examples/one-identity-policy.coax one-identity-policy -->
```jai
oip_run :: () -> bool {
  // 1. Account identity provisioning & human session project alias policy.
  p_create := ai_plan(true, false, false, false);
  p_linked := ai_plan(true, false, false, true);
  p_managed := ai_plan(true, false, true, true);
  p_frozen := ai_plan(true, true, true, true);
  alias_human := ai_project_alias(2, true, false, false, false);
  alias_limited := ai_project_alias(2, true, true, false, false);
  print("identity plan=(create=%,linked=%,managed=%,frozen=%) alias=(human=%,limited=%)\n",
    p_create, p_linked, p_managed, p_frozen, alias_human, alias_limited);

  // 2. WorkOS hybrid acct_ PKCE login + multi-org selection -> authenticated session.
  f: [1] WosAuthFlow;
  ver_ent: [32] u8; st_ent: [32] u8;
  for i: 0..<32 { ver_ent[i] = cast(u8) (i + 1); st_ent[i] = cast(u8) (i + 65); }
  url_buf: [1024] u8; scratch: [4096] u8;
  un := wos_flow_pkce_start(f[..], "", "client_01", "openagents://auth/callback", "GoogleOAuth", "", "", "", "", ver_ent[..], st_ent[..], url_buf[..]);
  st_copy: [WOS_STATE_LEN] u8;
  for i: 0..<43 { st_copy[i] = f[0].oauth_state[i]; }
  req := wos_flow_pkce_callback(f[..], "", "client_01", "", "code_ok", cast(string) st_copy[..], "", scratch[..]);
  ver_zeroed := f[0].verifier_n == 0 && f[0].oauth_state_n == 0;
  org_st := wos_flow_on_auth_response(f[..], 403, "{\"code\":\"organization_selection_required\",\"pending_authentication_token\":\"pat_42\"}", "", "");
  org_req := wos_flow_select_org(f[..], "", "client_01", "", "org_oa", "203.0.113.7", "OpenAgents/1", scratch[..]);
  auth_st := wos_flow_on_auth_response(f[..], 200, "{\"access_token\":\"at_oa\",\"refresh_token\":\"rt_oa\",\"organization_id\":\"org_oa\",\"user\":{\"id\":\"user_oa\"}}", "", "");
  print("workos url_len=% cb_ok=% ver_zeroed=% org_step=% auth=% user=% org=%\n",
    un, req.error == WOS_GEN_OK && req.public_auth, ver_zeroed,
    org_st == WOS_FLOW_NEED_ORG_SELECT && org_req.error == WOS_GEN_OK,
    auth_st == WOS_FLOW_AUTHENTICATED,
    cast(string) f[0].user_id[0..<f[0].user_id_n],
    cast(string) f[0].org_id[0..<f[0].org_id_n]);

  // 3. NIP-05 identifier mapping and verification.
  id := nip05_parse("bob@example.com");
  n5_url_buf: [128] u8;
  n5_un := nip05_url(id, n5_url_buf[..]);
  bob_key := n5_key(N5_BOB);
  ok_body := "{\"names\":{\"bob\":\"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\"}}";
  n5_ok := nip05_check(id, 200, ok_body, bob_key[..]);
  print("nip05 url=% check=%\n", cast(string) n5_url_buf[0..<n5_un], n5_ok);

  // 4. Fail-closed refusals: bad OAuth state zeroes verifier; NIP-05 refuses redirect, mismatch, uppercase.
  _ := wos_flow_pkce_start(f[..], "", "client_01", "openagents://auth/callback", "GoogleOAuth", "", "", "", "", ver_ent[..], st_ent[..], url_buf[..]);
  bad_cb := wos_flow_pkce_callback(f[..], "", "client_01", "", "code_1", "wrong_state_value_0000000000000000000000000", "", scratch[..]);
  ref_state := bad_cb.error == WOS_GEN_MISSING && f[0].state == WOS_FLOW_FAILED && f[0].verifier_n == 0;
  ref_redir := nip05_check(id, 301, ok_body, bob_key[..]) == N5_REDIRECT;
  other_key := n5_key("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9");
  ref_mismatch := nip05_check(id, 200, ok_body, other_key[..]) == N5_MISMATCH;
  ref_upper := nip05_parse("Bob@example.com").error == N5_IDENTIFIER;
  print("refuse state=% redirect=% mismatch=% upper=%\n", ref_state, ref_redir, ref_mismatch, ref_upper);

  return p_create == AI_CREATE && p_linked == AI_LINKED && p_managed == AI_MANAGED && p_frozen == AI_SKIP &&
         alias_human && !alias_limited && un > 0 && req.error == WOS_GEN_OK && ver_zeroed &&
         org_st == WOS_FLOW_NEED_ORG_SELECT && org_req.error == WOS_GEN_OK && auth_st == WOS_FLOW_AUTHENTICATED &&
         n5_ok == N5_OK && ref_state && ref_redir && ref_mismatch && ref_upper;
}
```

Output (`coax/bin/coax run openagents/examples/one-identity-policy.coax`, identical in `--release`):

```text
identity plan=(create=3,linked=1,managed=2,frozen=0) alias=(human=true,limited=false)
workos url_len=292 cb_ok=true ver_zeroed=true org_step=true auth=true user=user_oa org=org_oa
nip05 url=https://example.com/.well-known/nostr.json?name=bob check=0
refuse state=true redirect=true mismatch=true upper=true
ok=true
```
