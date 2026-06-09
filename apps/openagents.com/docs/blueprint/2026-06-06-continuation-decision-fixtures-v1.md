# Continuation Decision Fixtures v1

Date: 2026-06-06

Status: implemented for #273 / `OPENAGENTS-CONT-002`.

## Purpose

The continuation fixture catalog retains public-safe first-batch examples for
the continuation decision service. These fixtures let OpenAgents product surface improve
continuation behavior through visible regression cases instead of hidden prompt
edits.

## Fixture Coverage

The first catalog covers:

- successful continuation;
- generated Site changes that need tests;
- failed Site builder or repair work that needs a fix;
- returning operator summary needs;
- missing customer or source context;
- account-fleet retry needs;
- stopped or superseded runs;
- quality-gate escalation;
- review-ready Site revision work.

Each fixture links:

- a public-safe task/source doc ref;
- a `BlueprintContinuationTurnResult`;
- the expected continuation decision;
- a Program Signature id;
- an eval fixture ref;
- scorecard refs;
- evidence refs;
- receipt refs;
- source authority refs.

## Redaction Boundary

Fixtures must not contain raw secrets, provider tokens, agent bearer tokens,
OAuth material, raw runner logs, raw email payloads, wallet/payment secrets, or
private customer data. The fixture test suite includes a private-material
regression check.

## Consumers

#272's `decideBlueprintContinuation` service consumes these fixtures directly
in tests. #274 can use them to seed Decision Queue examples, and #276 can use
their scorecard and eval fixture refs when checking continuation release gates.
