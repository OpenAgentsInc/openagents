# Probe Blueprint Consumer Contracts

Date: 2026-06-07

Status: implemented as the first local contract layer for Probe issue #172.

Probe now carries a narrowed Effect Schema mirror for the Blueprint contracts it
needs before OpenAgents product surface exposes live HTTP routes. This is intentionally a consumer
surface, not a fork of the OpenAgents product surface Blueprint runtime. Probe does not import
`openagents`; it mirrors only the public/operator-safe fields needed for
signature lookup, registry projection decoding, backend-independent tool menu
planning, Program Run evidence flags, release gate references, and contract
export discovery.

The current fixture in `packages/runtime/src/blueprint/fixtures.ts` seeds two
Probe-facing Blueprint signatures:

- `program_signature.probe.signature_lookup.v1`
- `program_signature.probe.tool_menu.project.v1`

Both fixtures preserve OpenAgents product surface's intended safety posture: registry entries are
`safeProjection: true`, Program Types have `directMutationAllowed: false`, run
details are evidence-only, release gates cannot self-promote, and fixture data
uses refs instead of raw prompts, callback URLs, provider payloads, secrets,
wallet material, private repo content, or customer data.

This local mirror is temporary infrastructure for the next steps. Issue #173
adds registry sources for fixture, assignment-carried projection, and OpenAgents product surface
HTTP. Once OpenAgents product surface ships `GET /api/blueprint/program-registry` and
`GET /api/blueprint/contracts`, Probe should use those routes as the source of
truth and keep the static fixture only for tests, offline development, and
emergency bootstrap.

Issue #173 adds that source client under
`packages/runtime/src/blueprint/registry-client.ts`. The client normalizes three
source modes into the same safe registry view:

- `staticFixture` for the checked-in seed mirror.
- `assignmentInline` for a safe registry slice attached to a Probe assignment.
- `openagentsHttp` for the future authenticated OpenAgents product surface Blueprint routes.

Each source is decoded through Effect Schema and then checked for
`safeProjection`, evidence-only Program Run flags, no direct mutation, no
self-promoting release gates, and private-data-shaped refs. The normalized view
records `sourceKind`, `registryVersionRef`, `safeProjectionPolicyRef`, and the
contract export version when present. The static fixture remains a bridge until
OpenAgents product surface becomes the live source of truth.
