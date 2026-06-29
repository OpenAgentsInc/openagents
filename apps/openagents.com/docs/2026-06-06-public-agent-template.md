# Public Agent Template

This note defines the reusable public-agent projection contract added for
OPENAGENTS-061. Artanis and Adjutant now fit the same public shape instead of each
surface inventing a different proof page model.

## Contract

`workers/api/src/public-agent-template.ts` defines a source record and a
projected record for public agents. The source record can carry:

- agent identity and source type;
- objective and current-state refs;
- health state;
- gate refs;
- timeline event refs;
- artifact refs;
- proof refs;
- caveat refs;
- clean first-party public URLs; and
- customer, team, and operator refs that are only projected to the matching
  audience.

The projection audiences are the same as public claim projections:

- `public`;
- `customer`;
- `team`; and
- `operator`.

Public projections never expose customer, team, or operator refs. Customer
projections expose only customer refs. Team projections expose customer and team
refs. Operator projections expose all three, but still reject raw secrets,
provider grants, runner payloads, wallet state, customer private data, raw
payment material, and private workroom artifacts.

## Relationship To Claim State

Every public-agent template contains a `PublicClaimProjectionRecord`. The
template projection uses `projectPublicClaimRecord`, so public-agent pages
inherit the same planned, modeled, measured, verified, settled, blocked, and
prohibited copy rules.

That matters for campaign pages: an agent can say that a gate is planned,
measured, verified, or blocked only when the evidence refs support that claim.
If evidence is missing, the claim-state layer lowers the state and carries the
caveat text.

## Source Examples

`publicAgentTemplateSourceExample('artanis')` and
`publicAgentTemplateSourceExample('adjutant')` are deliberately example
records, not page-specific route logic. They prove that:

- Artanis can use the template for public Pylon/campaign progress; and
- Adjutant can use the same template for public Sites supervision progress.

Future public agents should add source records or source adapters into this
contract instead of creating one-off public page schemas.

## Safety Rules

The template rejects:

- prompt logs and raw runner payload refs;
- provider account, provider grant, and token refs;
- API tokens, bearer strings, OAuth/cookie material, and private keys;
- wallet state, invoices, preimages, and raw payment refs;
- customer private data, including email-shaped refs; and
- private workroom artifact refs.

Public URLs must be clean `https://openagents.com/...` URLs without query
parameters or fragments.

## Verification

Coverage lives in `workers/api/src/public-agent-template.test.ts` and checks:

- public/customer/team/operator redaction;
- Artanis and Adjutant source examples using the same projection contract;
- claim-state copy/caveat lowering when evidence is missing;
- unsafe prompt/provider/wallet/customer/workroom refs; and
- public URL query-state rejection.
