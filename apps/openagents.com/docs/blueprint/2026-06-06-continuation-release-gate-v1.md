# Blueprint Continuation Release Gate v1

Date: 2026-06-06

Status: implemented for issue #277.

## Purpose

Continuation Release Gate prevents new or edited Autopilot continuation
Program Signatures and Module Versions from reaching runtime authority without
fixtures, regression coverage, rollback posture, policy review, and explicit
operator approval.

The service wraps the generic `BlueprintReleaseGate` predicate with
continuation-specific target checks.

## Service

`evaluateBlueprintContinuationReleaseGate` consumes:

- a `BlueprintReleaseGate`;
- a Program Signature target or Module Version target.

It returns:

- whether the target can promote;
- target kind and ref;
- release gate ref;
- required fixture refs;
- required receipt refs;
- failure refs.

## Required Conditions

Promotion requires:

- target kind and target ref match the gate;
- target is a continuation Program Signature or Autopilot continuation Module
  Version;
- fixture refs are present;
- fixtures passed;
- policy is compliant;
- review is approved;
- explicit operator decision is present;
- scorecard is present;
- receipts are present;
- rollback anchor receipt is present;
- self-promotion was not attempted;
- the generic `blueprintReleaseGateCanPromote` predicate passes.

Module Version promotion also requires:

- valid release state;
- not already production;
- no self-promotion ability;
- operator-promotion-required candidate posture.

## Boundary

The gate is a predicate only. It does not:

- promote a Program Signature;
- promote a Module Version;
- mutate Program Registry state;
- execute rollback;
- grant runtime authority;
- approve draft marketplace contributions.

Action Submissions, operator approval APIs, registry promotion records, and
runtime deployment are later authority boundaries.
