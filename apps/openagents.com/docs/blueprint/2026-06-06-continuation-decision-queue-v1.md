# Continuation Decision Queue v1

Date: 2026-06-06

Status: implemented for #274 / `OPENAGENTS-CONT-003`.

## Purpose

The Decision Queue projection turns continuation decisions into a visible list
of pending next actions for operators and customers.

It is a typed projection boundary, not a mutating route. It does not continue a
run, deploy a Site, send email, create a pull request, rotate an account, spend
money, or approve a public claim. It tells the next authorized actor what kind
of work is waiting and which evidence/receipt refs justify that state.

## Queue Item Fields

Each queue item includes:

- decision ref;
- action;
- status;
- recommended next order ref;
- blocker refs;
- approval refs;
- retry refs;
- account-failover flag and refs;
- stop condition refs;
- Program Signature id;
- workroom/order/Site refs;
- evidence refs;
- receipt refs;
- safe summary ref.

## Customer Versus Operator Projection

Operator projection keeps operator-safe refs such as source authority refs and
account-failover refs.

Customer projection keeps the queue item visible but strips source authority
refs and account/provider details. For example, a `retry_account` item remains
visible as `accountFailoverNeeded: true`, but customer output carries only the
safe retry marker `retry.account_failover_needed`.

## Fixture Coverage

The first tests build queue projections from the #273 continuation fixtures and
cover:

- next order refs for every decision kind;
- blocker refs for fix, request-context, and escalation cases;
- approval refs for review and escalation cases;
- retry and account-failover refs for operator projection;
- customer redaction for account/source details;
- stop condition refs for terminal decisions.

Future HTTP/API handlers should consume this projection instead of exposing raw
run logs, provider details, or private workroom internals.
