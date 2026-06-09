# Blueprint Continuation Mission Briefing v1

Date: 2026-06-06

Status: implemented for issue #275.

## Purpose

Continuation Mission Briefing turns the Decision Queue into a concise,
customer-safe or operator-safe explanation of current workroom state.

It is meant for returning customers, operators, and agents that need to know
what changed, what was verified, what was emailed, what is blocked, what route
is active, what costs are represented, and what the next action should be
without reading private runner logs.

## Inputs

`buildBlueprintMissionBriefing` consumes:

- a `BlueprintContinuationDecisionQueueProjection`;
- work kind: `site` or `coding`;
- a workroom ref;
- public-safe changed artifact refs;
- evidence, build, test, email, cost, route, public link, and acceptance
  request refs;
- `updatedAtIso` and `nowIso`, which are rendered as friendly labels rather
  than raw timestamps.

## Output

The projection includes deterministic sections:

- `changed`;
- `evidence`;
- `verification`;
- `email`;
- `blocked`;
- `costs`;
- `route`;
- `acceptanceRequest`;
- `links`;
- `nextAction`.

Each item contains a section kind, customer-safe ref, summary ref, status,
friendly display time, and optional drill-down link refs.

## Projection Boundaries

Supported audiences are `public`, `customer`, `team`, and `operator`.

Public, customer, and team briefings hide provider-account details, source
authority internals, raw logs, raw email material, credentials, private keys,
tokens, email addresses, and raw ISO timestamps. Operator briefings may retain
redacted operator-safe provider-account refs such as account-failover evidence,
but still reject raw secrets and raw logs.

## What It Does Not Do

Continuation Mission Briefing does not:

- mutate order, queue, Site, Forum, or workroom state;
- send email;
- deploy Sites;
- approve or reject work;
- execute retries;
- create payment or payout claims;
- fetch private logs.

Those actions remain behind Action Submissions, route-specific APIs, and later
release gates.
