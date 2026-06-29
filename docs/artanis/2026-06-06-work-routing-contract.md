# Artanis Work Routing Contract

Date: 2026-06-06

Issue: #395 / `ARTANIS-010`

Status: implemented as a schema/projection contract in
`workers/api/src/artanis-work-routing.ts`.

## Purpose

Artanis needs to propose useful work for Pylon, Nexus, Model Lab, Benchmark
Cloud, Psionic, Probe, and runner paths without pretending that a proposal is a
dispatch command. The work-routing contract models proposals, risk, cost
caveats, resource mode, acceptance criteria, traceable work refs, and receipts
while keeping provider, wallet, settlement, and runtime mutation in separate
server-authoritative paths.

## Covered Work

The v1 work classes are:

- inference
- benchmark evaluation
- GEPA/DSPy-style optimization
- LoRA/fine-tuning
- training
- embedding/data prep
- validation

The v1 target capabilities include:

- Pylon inference and training
- Nexus assignment
- Model Lab evidence review
- Benchmark Cloud evaluation
- Psionic adapter validation
- Probe coding-runtime checks
- runner artifact validation
- embedding/data prep
- GEPA/DSPy optimization
- LoRA/fine-tuning

Resource modes can be `background`, `overnight`, `dedicated`,
`operator_selected`, or `not_applicable`.

## Proposal Requirements

Every proposal requires:

- source evidence refs
- target capability refs
- acceptance criteria refs
- risk label
- resource mode
- work class
- target system

Approval-required proposals also require:

- approval requirement refs
- spend-limit refs
- cost-caveat refs

Accepted, dispatched, and completed proposals require:

- traceable work refs
- receipt refs

Blocked or rejected proposals require:

- public-safe blocker refs
- public-safe caveat refs

## Authority Boundary

The ledger exports hard false authority flags:

- no direct dispatch authority
- no provider mutation authority
- no wallet spend authority
- no settlement mutation authority
- no runtime mutation authority

An accepted work-routing proposal means Artanis has recorded a bounded,
traceable proposal and receipt. It does not mean the provider was mutated, a
wallet spent bitcoin, settlement occurred, a model was promoted, or a runtime
was deployed.

## Public Boundary

Public `/artanis` and Forum projections can show:

- public proposal refs
- work class
- target
- capability
- resource mode
- risk label
- source evidence refs
- cost caveat refs
- spend-limit refs
- acceptance criteria refs
- traceable work refs
- receipt refs
- public blocker/caveat refs
- friendly display times

They do not show:

- operator detail refs
- raw runner/provider payloads
- wallet/payment material
- private customer data
- raw datasets, prompts, logs, traces, model artifacts, or private repos
- raw timestamps

## Tests

Coverage lives in `workers/api/src/artanis-work-routing.test.ts`. It proves:

- proposals carry source evidence, target capability, risk, cost caveat,
  resource mode, and approval requirements;
- accepted proposals carry traceable work refs and receipts;
- blocked/rejected proposals expose public-safe caveats;
- public projections redact operator details and raw timestamps;
- direct dispatch, provider mutation, wallet spend, settlement mutation, and
  runtime mutation authority is rejected;
- unsafe refs are rejected before projection.
