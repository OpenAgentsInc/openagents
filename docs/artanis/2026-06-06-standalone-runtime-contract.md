# Artanis Standalone Runtime Contract

Date: 2026-06-06

Issue: #386 / `ARTANIS-001`

Status: implemented as a read-only schema/projection contract in
`workers/api/src/artanis-runtime.ts`.

## Purpose

Artanis is now modeled as a first-class standalone autonomous agent runtime,
not just a generic public-agent template projection and not Adjutant with a
different name.

The contract centers the `agent_artanis` identity and requires stable refs for:

- goals,
- autonomous work loops,
- private evidence packets,
- public projection packets,
- Forum coordination,
- Model Lab context,
- Pylon context,
- Nexus context,
- Pylon/Model Lab campaign context.

## Identity

A valid Artanis runtime record must use:

- `agentId`: `agent_artanis`
- `agentRef`: `artanis`
- `displayName`: `Artanis`

Records with generic-agent or Adjutant identity are rejected. This keeps the
generic public-agent template useful for display, but not authoritative for
Artanis runtime state.

## Boundaries

Artanis differs from Adjutant because Artanis is the Nexus/Pylon/Model Lab
continual-learning steward. Adjutant remains the Sites and fulfillment
supervisor.

Artanis differs from a generic public agent because Artanis has a named
standalone runtime, autonomous work-loop refs, operator steering refs, private
evidence refs, Model Lab refs, Pylon refs, Nexus refs, and campaign refs.

## Authority

The runtime contract is read-only evidence. It explicitly denies:

- wallet spend,
- payment spend,
- provider mutation,
- training launch,
- adapter install,
- runtime promotion,
- deployment,
- settlement mutation,
- public claim upgrade.

Any future action that spends money, mutates providers, launches training,
installs adapters, promotes runtime behavior, deploys, settles, or upgrades a
public claim needs a separate server-authoritative action and receipt.

## Projection

`projectArtanisRuntime(record, audience, nowIso)` returns an
`ArtanisRuntimeProjection` for public, agent, customer, team, and operator
audiences.

Public, agent, and customer projections omit private evidence refs. Team and
operator projections may see private evidence refs after validation. All
audiences get friendly time labels rather than raw timestamps.

Public projections reject or redact provider, runner, wallet, payment,
customer, private repo, secret, raw prompt, raw log, and raw timestamp
material.

## Tests

Coverage lives in `workers/api/src/artanis-runtime.test.ts`. The tests cover:

- public projection of `agent_artanis`,
- no implicit wallet/provider/training/adapter/runtime/deploy/settlement/public
  claim authority,
- private evidence visibility only for team/operator audiences,
- rejection of generic or Adjutant identity,
- required goal/work-loop/evidence/projection/Forum/Model Lab/Pylon/Nexus/
  campaign refs,
- clean first-party public URLs,
- blocked and approval-waiting state requirements,
- public redaction of private refs,
- unsafe material and false-authority rejection.
