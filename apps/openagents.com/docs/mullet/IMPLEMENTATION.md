# Mullet Runner Implementation Status

This file tracks the sequential implementation of the private `/mullet`
unified simulation runner. The source audit is
`docs/mullet/2026-06-08-openagents-unified-mullet-simulation-runner-audit.md`.

## Completed Issues

| Issue                                                               | Status   | Notes                                                                                                                                                                                                                                                                                                                            |
| ------------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [#536](https://github.com/OpenAgentsInc/openagents/issues/536) | Complete | Confirmed `chris@openagents.com` as the only mullet operator account, added the simulation-only invariant, and denied the misspelled account in tests.                                                                                                                                                                           |
| [#537](https://github.com/OpenAgentsInc/openagents/issues/537) | Complete | Added `@openagentsinc/mullet-schema` with Effect Schema domain records, branded units/IDs, provenance fields, Tinybox and 100 MW fixture decode tests, and JSON boundary helpers.                                                                                                                                                   |
| [#538](https://github.com/OpenAgentsInc/openagents/issues/538) | Complete | Added `@openagentsinc/mullet-sim`, a pure side-effect-free simulation package covering MODEL1 accepted-outcome economics, consumer/SHC/miner provider floors, mining and raw-AI Margot-style baselines, hourly dispatch reason codes, and party capital-return summaries.                                                           |
| [#539](https://github.com/OpenAgentsInc/openagents/issues/539) | Complete | Added private D1 tables and `MulletRepository` Effect service/layer for scenarios, simulation runs, hourly dispatch results, candidate modes, and export metadata; repository tests cover create/list/get/not-found, Schema decoding, unsafe payload rejection, and candidate-mode persistence.                                  |
| [#540](https://github.com/OpenAgentsInc/openagents/issues/540) | Complete | Added private `/api/mullet/*` Worker routes for bootstrap, scenarios, run creation/read, and latest export read; every route requires a browser session plus `chris@openagents.com` admin authority, decodes request bodies through Effect Schema, maps tagged errors only at the HTTP boundary, and returns no-store responses. |
| [#541](https://github.com/OpenAgentsInc/openagents/issues/541) | Complete | Added the private Foldkit `/mullet` route, a dedicated `loggedInMulletAccessAllowed` browser policy requiring completed onboarding, admin authority, and exact `chris@openagents.com` email, plus a logged-in mullet submodel, bootstrap command, update transitions, and compact operator-console shell.                        |
| [#542](https://github.com/OpenAgentsInc/openagents/issues/542) | Complete | Built the private scenario workbench on `/mullet`: six Chris-requested templates, dense provenance-bearing assumption controls, hourly candidate-mode dispatch, party-specific return rows that count buyer revenue only once, accepted-outcome per-energy metrics, decision-flip sensitivity rows, and explicit evidence empty states. |
| [#543](https://github.com/OpenAgentsInc/openagents/issues/543) | Complete | Added private Markdown/JSON run export generation, export redaction scanning, optional typed proof/telemetry/settlement/market-memory run attachments, route tests for exports and refs, a Mullet API deploy smoke in `bun run check:deploy`, and the private operator runbook.                              |

## Current Authority Boundary

The runner remains simulation-only. Schema records and future run rows do not
authorize live Pylon assignment, provider mutation, wallet spend, Bitcoin
settlement, accepted-work closeout, public claim promotion, Forum posting, or
other production side effects.

The pure simulation engine is deterministic and has no Worker, D1, browser,
payment, provider, wallet, settlement, or public-claim authority. Its outputs
are modeled values until later API/UI layers attach measured telemetry,
accepted-work proof packets, payment receipts, or settlement refs through typed
Schema records.

The D1 persistence layer stores private simulation records only. It decodes
scenario, run, candidate, and export payloads through named Effect Schema
boundaries and rejects secret-shaped/private-data-shaped payloads before D1
inserts. Repository services return tagged domain/storage errors; HTTP response
mapping now lives in `workers/api/src/mullet/routes.ts`.

The API layer can create and read private scenarios, run a deterministic
single-hour modeled simulation from an existing scenario, attach supplied typed
proof/telemetry/market-memory refs to runs, generate private export packets,
read stored runs, and read latest export metadata. It does not expose any live
dispatch, provider mutation, wallet spend, settlement, public-claim, Forum, or
accepted-work closeout authority.

Private export generation now lives behind `POST
/api/mullet/runs/:runId/export`. It returns generated Markdown or JSON content
with private visibility, marks the packet as not a public claim projection,
labels modeled, measured, verified, accepted, paid, and settled values
separately, and persists only export metadata after redaction passes. Run
creation may attach existing proof packets, energy telemetry, settlement refs,
and market-memory records, but the runner only copies supplied refs; it does
not fabricate proof refs from modeled dispatch. The operator workflow and
simulation-only boundary are documented in `docs/mullet/OPERATOR_RUNBOOK.md`.

The browser route is also private. `/mullet` parses as a logged-in product
route, requires auth bootstrap, redirects logged-out users to the public
homepage path, redirects non-admin and wrong-admin-email sessions to `/order`,
and schedules `LoadMulletBootstrap` for the allowed `chris@openagents.com`
operator. The route now hosts a local private workbench over typed Mullet
schemas and pure simulation functions. It includes the Tinybox SHC, Tinybox
residential, Tinybox West Texas miner-site, 100 MW 80/20 facility, SHC
CPU/VPS/colo, and miner-site GPU-island templates; every editable assumption
has a value, source label, provenance state, confidence, and required-evidence
hint. The UI keeps modeled, measured, accepted, paid, and settled values
visually distinct and renders explicit missing states for measured energy,
accepted-work demand, settlement evidence, Margot baseline import, readiness
proof, and payout proof. The browser workbench still has no authority to
assign live work, mutate providers, spend wallet funds, settle payouts, or
promote public claims.
