# Probe Coding-Runtime Adapter Contract

Date: 2026-06-06

Status: implemented contract note for issue #337 / `OPENAGENTS-RUST-004`.

## Purpose

OpenAgents product surface now has a schema-first Probe coding-runtime adapter contract.

The implementation lives in
`workers/api/src/probe-coding-runtime-contract.ts`.

This is a contract and projection layer only. It does not launch Probe, run
tools, mutate source, create a pull request, execute tests, upload artifacts,
or call a model provider.

## Contract Shape

The contract defines:

- `OpenAgentsProbeRunRequest`;
- `OpenAgentsProbeTurnEvent`;
- `OpenAgentsProbeToolCallSummary`;
- `OpenAgentsProbeRunRecord`; and
- `OpenAgentsProbeRunProjection`.

Run requests carry:

- workroom refs;
- assignment refs;
- Program Run refs;
- objective refs;
- runtime refs;
- route refs;
- source-authority refs;
- policy refs;
- idempotency refs;
- correlation refs; and
- workload trust.

Run records carry:

- normalized turn events;
- tool-call summaries;
- diff refs;
- artifact refs;
- test result refs;
- preview refs;
- cost refs;
- failure refs;
- retained-failure refs; and
- closeout receipt refs.

## Status Model

Probe run statuses are:

- queued;
- running;
- succeeded;
- failed;
- cancelled;
- timed out;
- needs context;
- needs review; and
- retained failure.

The helpers distinguish terminal states and terminal evidence requirements.
Succeeded runs require closeout receipt refs plus artifact or diff refs.
Failed and timed-out runs require failure refs plus retained-failure refs.
Cancelled runs require closeout receipt refs.

## Conformance Fixtures

`OPENAGENTS_PROBE_CONFORMANCE_FIXTURES` contains two seed fixtures:

- a successful coding run with a tool-call summary, test refs, diff/artifact
  refs, preview refs, cost refs, and closeout receipt refs; and
- a retained-failure run with failure refs and retained-failure refs.

Future Rust Probe adapters can mirror these fixtures without making OpenAgents product surface
depend on a Rust fixture package yet.

## Projection And Redaction

Public/customer/agent projections expose safe summary, artifact, test, preview,
receipt, and route refs. Operator/private projections can also show safe
diagnostic refs.

The contract rejects:

- raw tool logs;
- raw runner logs;
- raw provider payloads;
- raw auth payloads;
- credentials, tokens, cookies, OAuth material, and API keys;
- local paths;
- private repo refs;
- raw source archives;
- wallet/payment material;
- payout targets; and
- raw timestamps.

Projection times use friendly labels instead of raw timestamps.

## Tests

`workers/api/src/probe-coding-runtime-contract.test.ts` covers:

- conformance fixture decoding;
- successful run projection;
- retained-failure projection;
- terminal evidence requirements; and
- unsafe log, provider payload, credential, private repo, wallet/payment, and
  timestamp rejection.
