# Self-hosted OpenAgents Lightning addresses

Status: source implementation complete; production activation and funded checks
are tracked in `NEEDS_OWNER.md`. Owner decision: 2026-10-20.
Issue: [#12307](https://github.com/OpenAgentsInc/openagents/issues/12307).

## Decision and scope

OpenAgents operates the LNURL service for `name@openagents.com`. The
website keeps its existing DNS and HTTPS endpoint. It routes only the
Lightning address paths to the address service. Spark wallets and the
Spark Service Provider (SSP) still move the money. OpenAgents does not
operate Lightning channels or become a shared wallet for users.

Implement the address service and its account, wallet, SDK, CLI, and
profile integration first. Then finish #12307's receipt-backed profile
earnings and optional browser wallet restoration. Phone-approved browser
spending remains the default throughout.

The address service uses Breez's public Rust LNURL implementation as a
private backend, pinned to an explicit source revision. OpenAgents adds
account authorization, permanent payment-name ownership, public routing,
configuration, deployment, and acceptance tests in this repository. Do
not copy private code or expose the backend's unrestricted registration
API. Do not write a second Spark settlement implementation.

## Why host it

Self-hosting puts name allocation, recovery, routing, logs, and service
availability under OpenAgents control. It supports consistent account
names across profiles, tips, and agent payments. Keeping the domain also
allows a future provider migration without changing published addresses.

The costs are an always-available Rust service, PostgreSQL, HTTPS routing,
backups, monitoring, security updates, and payment incident response.
Hosting the address service does not remove SSP or Spark dependencies,
reduce Lightning fees by itself, or change Spark's custody assumptions.
Breez-hosted service with path forwarding remains a possible future
alternative, but is not the selected implementation.

## Existing components and boundaries

| Component | Responsibility |
| --- | --- |
| `tenancy::accounts::names` | Existing account-owned NIP-05 names and reserved words. |
| `tenancy::accounts` | Atomic, durable account state and payment-name holds. |
| `oa-auth` and the account service | Authenticated address enrollment and wallet proof verification. |
| `openagents-sdk` | Typed account and address API calls for apps and the CLI. |
| `openagents-spark` (`crates/spark-wallet`) | Wallet-local signing and Breez address operations. |
| `openagents-web` | Same-origin public LNURL routes and protected management routes. |
| Private Breez LNURL service | Offline invoice creation, payment metadata, verification, and zap delivery. |
| Apps and CLI | Explicit enable, disable, recovery, and address display. |

The account service remains the authority for which account owns a name.
Neither a profile's `lud16` field, an incoming request's `Host`, nor an
arbitrary Spark public key establishes that authority.

## Ownership and lifecycle

Enrollment requires a signed-in account, its currently held name, and a
proof made by the wallet's Spark identity key. Nostr and Spark keys are
different keys; do not assume that matching recovery words makes their
public keys equal. Never upload recovery words, seed entropy, wallet
spending keys, or an external signer's private material.

The durable record contains the normalized name, account ID, profile
Nostr key, compressed Spark identity public key, previous and historical
wallet keys, domain, revision, creation/update time, and state. State is `pending`, `active`, `disabled`,
or `retired`. Bound pending enrollment challenges by expiry and keep the
last applied challenge ID in the address record for safe retries. No record
contains a wallet secret.

Once a name has been enrolled for payments, its payment-name hold belongs
to that account permanently. Existing social names that have never been
used for payments retain their existing behavior. Reject a social-name
claim that would give another account a payment-held name. Keep holds
after disabling, renaming, removing a key, or deleting an account. Unknown,
disabled, deleted-account, and retired addresses issue no new invoices.
They never fall through to another person's wallet.

A rename does not silently move the old payment address to the new name.
The user enables the new address explicitly. The old name remains held;
its existing address either stays with the same verified owner or is
explicitly disabled. Account/key removal disables public resolution until
ownership is re-established. Do not erase an in-flight invoice's original
recipient when an address is disabled or changed.

Wallet rotation requires a new wallet proof and fresh account
authorization. Changing a destination increments the revision and cannot
reuse an old authorization. Existing invoices keep their original
recipient. Recovery never makes a held name available to another account.
Keep the old wallet as transfer authority until the backend accepts both
wallets' signatures. The original wallet can cancel an unfinished rotation
with a fresh account proof. Retain signed access to old wallets' own payment
metadata after disabling or rotating; those keys cannot register or transfer
the current address. Do not expose general cross-account address transfer
in the first release.

## Enrollment protocol

1. The client reads the signed-in account's name and current address.
2. It asks the account service for a short-lived challenge for that name,
   domain, requested operation, and expected record revision.
3. The wallet signs the exact returned message locally with the SDK's
   `sign_message` operation. The message starts with
   `openagents.lightning-address.v1` and contains the account ID, name,
   domain, operation, revision, random nonce, and expiration. Use a fixed,
   tested encoding. Do not use the SDK-reserved `breez-lnurl:` namespace.
4. The account service checks the session, current name ownership,
   revision, challenge lifetime, and ECDSA signature over SHA-256 of the
   message, using the returned Spark identity public key. Consume a
   challenge once, in the same transaction that saves the authorization
   and payment-name hold. A retry reads the resulting operation state.
5. The wallet registers through the existing Breez SDK against
   `openagents.com`. The public registration facade admits only the
   authorized name/key/domain/operation. The private LNURL backend also
   verifies the SDK's native registration signature.
6. Mark the address active only after backend registration succeeds.
   A failed or timed-out registration stays pending and can be retried by
   its owner. Do not publish a working address before that confirmation.

Use an explicit challenge lifetime of five minutes, a 32-byte random
nonce, and a bounded number of pending challenges per account. A
challenge's operation and revision cannot be changed after issuance.
Proof verification accepts the SDK's documented DER/compact signature
encoding and a valid compressed secp256k1 public key; persist one
normalized key representation.

SDK registration signatures and account-enrollment proofs serve different
purposes. An account proof must never authorize arbitrary SDK operations;
an SDK registration signature must never substitute for account ownership.
Authenticated management is unavailable when the account service is down.

## HTTP contract

| Endpoint | Access and behavior |
| --- | --- |
| `GET /.well-known/lnurlp/{name}` | Public LUD-16 lookup for an active address. |
| `GET /lnurlp/{name}` | Equivalent LNURL lookup. |
| `GET /lnurlp/{name}/invoice` | Bounded, validated LNURL invoice callback. |
| `GET /lnurlp/verify/{payment_hash}` | LUD-21 verification for an invoice this service issued. |
| `/lnurlpay/*` | Only documented SDK operations for a previously authorized name/key. Unrecognized operations are refused. |
| `GET /v1/account/lightning-address` | Authenticated address state and revision. |
| `POST /v1/account/lightning-address/challenge` | Authenticated, bounded challenge issuance. |
| `PUT /v1/account/lightning-address` | Authenticated enrollment/rotation proof and expected revision. |
| `DELETE /v1/account/lightning-address` | Authenticated disable; preserve the permanent name hold. |

Provide device-session equivalents through the existing website/account
API bridge, and typed `openagents-sdk` methods. Keep website session and
CSRF handling consistent with the existing account-name routes. Public
LNURL reads require no sign-in cookie. They allow cross-origin wallet
reads and answer protocol errors as LNURL errors, with plain user copy.
Disable caching for lookups, callbacks, management, verification, and
payment metadata. Reject unsupported methods, request fields, and paths.

Normalize names with the existing name rules. Use a configured public
origin and exact allowed domain. Ignore client-supplied forwarding headers
when choosing the domain or backend. The backend URL is operator
configuration, never an input URL. Disable redirects from that client.
Never relay a user's cookies, authorization token, or browser headers to
the private service.

For invoice requests, require a decimal integer amount in millisatoshis,
within the advertised minimum and maximum; reject overflow, zero,
fractional values, repeated amount fields, and unsupported precision.
Bound comment and zap-request size. Validate the returned BOLT11 network,
amount, expiry, and description hash against the request and metadata.
The SSP's shared invoice payee does not cryptographically identify an
individual Spark wallet: correct recipient routing still depends on the
trusted LNURL service and its registration state.

Begin with a 1-sat minimum and a configurable maximum no higher than the
backend's limit. Keep limits in millisatoshis on the wire and use checked
conversion at satoshi-only SDK boundaries. Do not silently round a payment.

## Security and abuse controls

A compromised address service can redirect new incoming payments by
returning another invoice. Users must trust both this service and the
SSP, including the absence of preimage-sharing collusion. Hosting the
service does not require OpenAgents to keep users' wallet seeds. Service
authentication, webhook secrets, and any zap signing key are separate
operator credentials.

Keep the backend on a private listener/network. Only the OpenAgents
facade reaches its management API. Verify both account ownership and the
wallet signature before forwarding registrations or destination changes.
Block unsigned name squatting, cross-account enrollment, stale revisions,
replayed proofs, and direct backend access. Apply bounded request bodies,
upstream responses, concurrent invoice requests, and upstream timeouts.
Use account limits for enrollment and IP/recipient limits for public
callbacks; trust client IP only from the configured ingress.

Redact signatures, tokens, preimages, webhook bodies, and payment comments
from access logs. Public-name enumeration cannot be eliminated from an
address directory, but responses must not disclose account IDs, email
addresses, private balances, or wallet keys beyond protocol requirements.
Keep payment history private unless its owner explicitly publishes it.

## Settlement, zaps, and earnings

An invoice request is not a payment. Count tips only after the payment
provider confirms settlement for an issued payment hash and recipient.
Authenticate webhooks, deduplicate by stable payment identity, and tolerate
duplicates, reordering, retries, and process restarts. A settled payment's
amount, recipient, and source cannot change on replay. A duplicate invoice
request must not create duplicate earnings.

Support the private backend's NIP-57 zap handling when its dedicated
signing key is configured. Advertise `allowsNostr` and `nostrPubkey` only
when receipt delivery is operational. Validate the signed zap request,
amount, recipient, and description hash. A signed zap receipt is an
attributable statement, not independent proof of settlement; do not use
arbitrary relay receipts to credit earnings.

Profile earnings combine Pylon earnings, plugin revenue share, and tips
through their existing receipt identities. Show amounts and links to the
supporting receipt; distinguish earned, pending, and paid amounts. Do not
sum the same payment from both a webhook and a published zap. Public
profiles do not expose private wallet history by default.

## Wallet and profile experience

The wallet offers an explicit **Enable Lightning address** action after a
name is claimed. Show the full address only when active, with Copy and
Share. Pending and unavailable states explain what the person can do.
Disable hides the receive address without releasing its payment-name hold.
Changing a wallet shows the old and new destination identities and asks
for a fresh local signature. No control claims success before the service
confirms it. Implement the shared Rust wallet logic once for the apps and
CLI; native shells only present it.

Add typed SDK operations and CLI status/enable/disable commands. A command
must not require a seed in an argument, environment variable, or request.
Use the existing wallet and session stores. The app signs a kind-0 update
with its own Nostr key to publish `lud16`; preserve the profile's other
fields and existing signing flow. The server never signs as the user.
On disable, offer removal only if `lud16` still matches the address we set.

For #12307's later browser wallet work, restoration is an explicit opt-in
flow, separate from the default phone-approved wallet. Recovery words and
keys remain local to the browser's Rust/Wasm wallet. Audit outgoing
requests, errors, analytics, persistence, and worker messages for leakage.
Explain browser storage exposure before restoration and provide a local
forget action. Do not add a server endpoint that accepts recovery words.

## Deployment and operations

Build the private Rust backend from an exact public upstream commit and
retain its license and attribution. Pin a revision compatible with the
wallet's current SDK and its native registration signature format; do not
assume an arbitrary upstream `main` server is compatible with our SDK pin.
Use one shared PostgreSQL database across backend replicas. Preserve
registration records, payment records, permanent holds, stable service
authentication identity, webhook secret, and zap key across restarts.

Provide a local build/deploy script and configuration example; do not add
GitHub workflows. Configure allowed domain, fixed public origin, private
backend URL, Breez API key, payment bounds, and secrets explicitly. An
unconfigured service issues no invoices and advertises no working address.
Keep payment routing out of ordinary page deployments where possible.

Monitor lookup/callback latency and errors, backend and database health,
registration failures, invalid/replayed proofs, webhook backlog, and
settlement mismatches. Alert on unexpected destination changes. Avoid
high-cardinality labels containing payment hashes or user names. Back up
and restore the account holds and LNURL database together, testing that a
restore cannot assign a previously used address to someone else.

Roll out first on a test deployment with disposable wallets. Then enable
production routes and new enrollment. On failure, stop new invoices and
enrollment while preserving settlement handling for issued invoices.
Rollback never deletes holds, changes recipients, restores old routing
over a newer destination revision, or silently switches to Breez-hosted
addresses. A future hosted migration must carry registration state and
explicitly account for pending invoices and zap signing identity.

## Acceptance and verification

Required automated coverage:

- Two accounts racing for one name; reserved words; social-name reuse
  before payments; permanent holds after payments, renames, and deletion.
- Valid SDK wallet proof; wrong key/account/domain/name/operation;
  expired, replayed, or altered proof; stale revision; safe retry after a
  registration timeout; disable and wallet rotation.
- Public lookup only after activation; disabled and missing users;
  unauthorized management; private-backend bypass refusal; no secret
  headers forwarded; no redirect following; bounded failures.
- LNURL metadata and CORS; callback amount bounds, units, duplicate fields,
  description hashes, expiry, network, and malformed invoices.
- Webhook authentication, repeated delivery, crash recovery, duplicate
  settlement, and receipt links without private-history disclosure.
- SDK mock-server contracts, fake-wallet CLI/app flows, profile signing
  preserving other metadata, and missing configuration states.
- Browser wallet opt-in and request-capture tests proving that restoration
  words and private keys never leave the local wallet.

Run focused Rust tests and edited-file formatting for affected crates.
Use isolated browser profiles and scratch hosts. Never use the owner's
wallet, seed, production account, or computer. Live Lightning acceptance
uses a dedicated disposable wallet with a small test balance and verifies
receipt, correct recipient, offline receive, restart, and disable.
Record deployment/live checks under the release audit's After-close
checks; owner-only keys or funding go in `NEEDS_OWNER.md`. Close the issue
when its requested code is on main and its own checks pass.

## References

- [Wallet audit and owner decisions](../audits/2026-10-17-spark-wallet.md).
- [Breez custom domains](https://sdk-doc-spark.breez.technology/guide/custom_domain.html).
- [Breez LNURL server](https://github.com/breez/spark-sdk/tree/main/crates/breez-sdk/lnurl).
- [Breez Lightning address API](https://sdk-doc-spark.breez.technology/guide/receive_lnurl_pay.html).
- [Breez payment webhooks](https://sdk-doc-spark.breez.technology/guide/lnurl_webhooks.html).
- [LUD-16](https://github.com/lnurl/luds/blob/luds/16.md) and [LUD-06](https://github.com/lnurl/luds/blob/luds/06.md).
- [NIP-57](../../nips/official/57.md).

## Deployment reference

The [operations runbook](lightning-address-operations.md) covers the pinned
backend build, private network, service configuration, backups, activation
checks, and rollback. Deployment templates live in `deploy/lightning-address/`.

## Implemented profile earnings contract

`GET /v1/device/earnings/{party}` returns private previews for the current
account's verified Nostr key. A Pylon provider credit must reference the
ledger's Pylon job. A plugin credit uses that party's exact revenue share,
not the whole sale. Pending, earned, paid, and reversed remain distinct; a
sent payout needs a wallet reference before it is shown as paid.

A normal Lightning-address deposit is not automatically a tip. The public
`/tip/{profile}` page requests `/lnurlp/{name}/tip` with the intended Nostr
recipient, which the gateway checks against the frozen address record. NIP-57
payments also qualify as explicit tips. Only invoices issued by this gateway
are eligible, and a settled LUD-21 response must supply a preimage matching
the known hash. The preimage is checked in memory and discarded. The original
recipient and exact amount survive name changes and wallet rotation. Polling
rotates through unconfirmed invoices so an abandoned request cannot starve
older payments. Ordinary income deposits cannot become duplicate tip income.

The native public-profile form uses `publish_earning` and `hide_earning` in
the Rust SDK. Publication selects a server-resolved source; the client cannot
choose its amount. Each selected credit becomes a signed kind-30078 NIP-78
event with `d=openagents.earnings.v1:<source>`, the schema `t` tag, and the
beneficiary `p` tag. Its content is `openagents.earnings.v1`: recipient, opaque
source hash, category, state, exact millisatoshis, Bitcoin network, timestamp,
and optional public Pylon receipt or plugin/release reference. Private hashes,
preimages, account IDs, payer details, and payout references are excluded.

Public profiles read `/v1/earnings/public/{party}`, validate signatures and
issuer selection, and show the latest 20 selected credits with receipt links.
This bounded list makes no lifetime-total claim. Selected ledger receipts
follow state changes; previous signed versions remain readable at
`/event/{id}` until the owner hides that credit. Publication is an attributable
service ledger statement, not independent proof of all of a person's income.
No wallet balance or history is published automatically.

`OPENAGENTS_EARNINGS_SERVICE` optionally directs the website's receipt bridge
to the native money gateway that opens the qualified Pylon/plugin ledger. It
defaults to the account service. Use the same account Postgres database and
Lightning backend configuration on that gateway, so account sessions, issued
tip invoices, publication choices, and the service signing key agree across
replicas. The bridge passes only the current bearer authorization to private
routes and none to public reads; it never forwards browser cookies or wallet
secrets. Public reads use no-store responses so a hidden credit disappears.

## Implemented opt-in browser wallet

`/wallet/restore` serves a separate page with no analytics, third-party scripts,
or HTML form submission. The app never gives its account bearer token to the SDK. Settings links to
it only when `OPENAGENTS_WALLET_WEB` points at a complete Wasm build. The Rust
UI in `spark-wallet-web` calls `spark-wallet::browser::BrowserWallet`; generated
JavaScript loads the module. The default phone-approved browser API remains
unchanged.

A checkbox explicitly opts this tab into wallet control. Rust validates the
12-word BIP39 checksum locally, clears the text field before opening Breez's
pinned Rust/Wasm SDK, and uses the same mainnet configuration and API validation
certificate as native wallets. The SDK's records, wallet keys, and temporary
words stay in memory. There is no localStorage, sessionStorage, IndexedDB,
seed upload, server restore endpoint, or automatic profile publication. The
page can refresh balance and recent payments, receive an amount-specific
Lightning invoice, and review a Bitcoin Lightning send before confirming its
amount and fee. Native and browser quotes choose the same lower-cost Spark or
Lightning route. An uncertain send retains its idempotency key; it cannot be
replaced with another quote until its outcome is known. Editing an unsent
invoice invalidates its quote.

Forget replaces the document and ends its Wasm instance, including a pending
network call. Returning through the back/forward cache reloads the page. A
restored tab is independent of the site's account session; close or forget
that tab to remove its wallet. A trusted browser, device, extensions, and
first-party deployment remain part of browser custody's security boundary.
Memory-only storage does not promise protection against a compromised device.

Build with `scripts/build-spark-wallet-web.sh OUTDIR`; the website Dockerfile
includes this artifact. Run the committed outgoing-request test with:

```sh
openagents browser run -- node scripts/smoke/browser-wallet-privacy.mjs OUTDIR
```

The test uses a public BIP39 vector and the actual compiled SDK. Chrome pauses
provider requests before they leave the machine, permits preflights locally,
and records actual request payloads. It checks URL, headers, body, binary and
Base64 forms against the words, entropy, BIP39 seed, BIP32 private roots, Spark
mainnet identity/deposit roots, and NIP-06 identity derivation. It also checks
local refusal of invalid words, no provider contact before consent, empty
browser storage, and forget/back-navigation reset. It does not fund a wallet
or prove a real-money receive/send; those checks use the dedicated test account
after deployment and remain in the owner checklist.
