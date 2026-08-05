# @openagentsinc/mkt-swp-destination

Destination entry and validation for the MKT-SWP swap widget
(openagents#9317, SWAP-2): the headless `DestinationField` state machine
plus the one shared parser behind the address field, the invoice field, and
the QR scanner.

## What lives here

- `parse.ts` — the shared parser. One typed discriminated result for
  on-chain addresses (network-aware), BOLT11 invoices, LNURL, lightning
  addresses, BOLT12 offers, BIP-21 URIs, and unified QR payloads. Every
  failure mode keeps its discriminant to the presentation layer; the
  message table test asserts no two modes collapse.
- `bolt11.ts` / `address.ts` — the family parsers. Amountless invoices are
  refused with the typed identifier `swp_invoice_invalid` (MKT-SWP §7.2).
- `entry.ts` — the pure reducer the SWAP-0 shell mounts: paste-driven
  route switching gated on SWAP-1 reachability, amount reconciliation
  (invoice precedence, amount edits clearing concrete invoices, deferred
  destinations surviving), and epoch-based staleness guards on every async
  step.
- `verify.ts` — the engine verdict port. Script/tree parsing and output-key
  re-derivation (§7.1 steps 3–5) are engine work behind the SWAP-0
  boundary; this package only consumes `swp_script_invalid` /
  `swp_script_commitment_mismatch` / `swp_invoice_invalid` verdicts and
  keeps funding blocked without a current-epoch pass.
- `resolve.ts` — deferred-destination resolution at order time with a
  bounded timeout; destination min/max violations are typed as the
  destination's limits, distinct from the protocol's.
- `qr.ts` — camera capability port and scan intake (same parser, no
  clipboard reads).
- `view.ts` — the render-ready `DestinationFieldView`.
- `messages.ts` — local message table with stable `swap.destination.*`
  keys shaped for the SWAP-8 shared table; migrates there when it lands.

Everything here is UX pre-checking. The MKT-SWP engine owns
verify-before-fund truth; nothing in this package authorises funding.

## Checks

```
pnpm --dir packages/mkt-swp-destination run check
```
