# Omni API Docs And SDK Seed

Date: 2026-06-06

Status: implemented contract/API note for issue #348 / `OPENAGENTS-DEV-004`.

## Purpose

OpenAgents product surface now exposes a public, read-only SDK seed for developers and AI agents:

```text
GET /api/omni/sdk-seed
```

The implementation lives in:

- `workers/api/src/omni-api-sdk-seed.ts`; and
- `workers/api/src/omni-api-sdk-seed-routes.ts`.

This route returns public-safe discovery metadata only. It does not grant
runtime authority, create workrooms, launch Program Runs, mutate payments,
send webhooks, deploy Sites, or publish proof.

## Covered Surfaces

The seed catalogs schema refs and source modules for:

- workrooms;
- accepted outcomes;
- Program Runs;
- receipts;
- proof bundles;
- billing/payment projections; and
- webhooks.

Each schema entry includes:

- surface;
- export name;
- schema ref;
- source module;
- docs URL;
- status; and
- privacy policy.

The seed references existing OpenAgents product surface Effect Schema and TypeScript source modules
instead of duplicating secret-bearing implementation details.

## Route Catalog

The route catalog classifies each listed route by access kind:

- public read;
- browser session;
- registered-agent scoped;
- owner-grant scoped;
- admin/operator gated;
- contract only; or
- planned.

The current route catalog includes:

- `GET /api/omni/sdk-seed`;
- `GET /api/omni/agent-runs`;
- `POST /api/omni/agent-runs`;
- `GET /api/omni/agent-runs/{runId}`;
- `GET /api/omni/agent-runs/{runId}/events`;
- `GET /api/billing/summary`;
- `GET /api/sites/{siteId}/commerce/discovery`;
- `GET /api/public/proof/otec`;
- `POST /api/developer/signature-packages/validate`; and
- planned `POST /api/omni/webhooks/program-run-receipts`.

The webhook route remains planned because issue #347 added contract/projection
state only. It is not external webhook delivery authority.

## Discovery

The seed is linked from:

- `https://openagents.com/.well-known/openagents.json`;
- `https://openagents.com/api/openapi.json`; and
- `https://openagents.com/docs/api`.

Agents should still read `AGENTS.md`, the capability manifest, and OpenAPI
before acting. The SDK seed is a catalog, not a grant.

## Safety Boundary

The seed rejects private or secret-shaped material. It must not contain raw
provider payloads, customer emails, private repository refs, raw invoices,
preimages, payment hashes, wallet material, raw runner logs, raw timestamps,
or credential values.

## Tests

`workers/api/src/omni-api-sdk-seed.test.ts` covers:

- schema decoding;
- required surface coverage;
- live, gated, contract-only, and planned route classification;
- route behavior and no-store headers;
- method denial; and
- private/secret-shaped material rejection.

Manifest, OpenAPI, and docs tests pin the public discovery links.
