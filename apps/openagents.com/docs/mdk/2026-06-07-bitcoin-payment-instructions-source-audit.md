# MoneyDevKit Bitcoin Payment Instructions Source Audit

Date: 2026-06-07
Issue: #451 / OPENAGENTS-H-014

## Source Reviewed

Local source:
`/Users/christopherdavid/work/projects/moneydevkit/repos/bitcoin-payment-instructions`

Revision: `d53d244`

Package: `bitcoin-payment-instructions` `0.7.0`

## Decision

OpenAgents product surface should use MoneyDevKit's `bitcoin-payment-instructions` crate as a
conformance source and future parser/resolver candidate, but it should not be
imported directly into the Cloudflare Worker runtime today.

The Worker now owns a narrow typed classification boundary in
`workers/api/src/payment-destination-input.ts`. That boundary classifies
pasted payment destinations and produces redacted refs. It does not resolve
LNURL, Lightning Address, or BIP353 names; it does not create payout authority;
and it does not dispatch payments.

The next live resolver step should be one of:

- compile the crate's pure parser path into a small WASM adapter and keep
  resolver behavior explicit; or
- run a sidecar/CLI resolver for LNURL, BIP353, or other network-dependent
  formats and return only typed, redacted refs to OpenAgents product surface.

Do not wire DNS, HTTP, wallet mutation, or payment dispatch into this parser
boundary.

## Supported Source Formats

The MDK crate supports these source formats:

- BIP321/BIP21-style `bitcoin:` URIs;
- BOLT11 Lightning invoices, with or without a `lightning:` URI prefix;
- BOLT12 offers;
- on-chain Bitcoin addresses;
- LNURL values, including URI/QR payloads where an LNURL is embedded in a
  larger URL;
- Lightning Address and BIP353-style human-readable names through resolver
  paths; and
- Cashu payment requests.

The source also models unknown required `req-*` parameters, wrong-network
errors, expired instructions, and inconsistent multi-method instructions.

## Runtime Compatibility

The parser crate is Rust, not TypeScript. Its pure parse path is `no_std`
capable, but the network resolver paths pull in runtime dependencies:

- `std`;
- `dnssec-prover` and Tokio-backed behavior;
- `bitreq` for HTTP when the `http` feature is enabled;
- optional proxied HTTP behavior; and
- resolver modules for BIP353, LNURL, DNSSEC, and onion-message paths.

Those resolver paths are not a normal Cloudflare Worker import. OpenAgents product surface can still
use the crate as a source of truth for conformance fixtures and can later use a
WASM or sidecar adapter, but the Worker should not import native MDK wallet,
DNS, or HTTP resolver runtime directly.

## OpenAgents product surface Parser Boundary

Implemented file:

```text
workers/api/src/payment-destination-input.ts
```

Focused test:

```text
workers/api/src/payment-destination-input.test.ts
```

The boundary classifies:

- `bolt11`;
- `bolt12`;
- `lnurl`;
- `lightning_address`;
- `human_readable_name`;
- `bitcoin_uri`;
- `onchain_address`;
- `cashu`;
- `unsupported`;
- `malformed`; and
- `ambiguous`.

The projection always includes:

- `approvalRequired: true`;
- `dispatchAllowed: false`;
- `payoutAuthorityCreated: false`;
- `rawDestinationProjected: false`;
- redacted destination refs;
- method refs;
- source refs; and
- runtime decision refs.

For LNURL, Lightning Address, and BIP353-style human-readable names, the
classification marks `requiresResolution: true` and points to a future
WASM/sidecar resolver. A successful classification is not a promise that a
payment can be made.

## Redaction And Authority

The parser accepts raw payment destination input only at the private parsing
boundary. It rejects wallet material, mnemonics, payment preimages, provider
credentials, MDK access tokens, webhook secrets, and local wallet config paths.

Public and agent projections must not include:

- raw BOLT11 invoices;
- raw BOLT12 offers;
- raw LNURL strings;
- raw Lightning Addresses;
- raw on-chain addresses;
- raw Cashu requests;
- raw payout targets;
- payment hashes;
- preimages;
- wallet mnemonics;
- provider credentials; or
- MDK secrets.

This remains a parser/classifier. Payout target approval, Forum reward
authority, Site checkout authority, Pylon payout target registration, Nexus
dispatch, Treasury settlement, and live wallet movement stay behind their own
policy gates.

## Product Impact

Sites can use the classifier to validate generated paid-action destination
inputs without exposing raw payment strings in Site proofs.

Forum can use the classifier for future rewards, topic boosts, and paid actions
so agents do not paste arbitrary raw payment strings into public posts.

Pylon can use the classifier before payout target admission, but only
`pylon-payout-target-admission` can decide whether a wallet-owned target is
registered and settlement-ready.

Nexus/Treasury can use the classifier as an input normalization step, but it
does not change accepted-work payout authority, dispatch approval, or
settlement evidence.
