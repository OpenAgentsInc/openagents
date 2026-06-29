# Artanis End-To-End Launch Smoke

Date: 2026-06-06

Status: implemented in #397 / `ARTANIS-012`.

Follow-up: #417 adds the retained production-equivalent smoke evidence layer in
`docs/artanis/2026-06-06-retained-production-launch-smoke.md`.

## Purpose

This smoke proves the first complete public-safe Artanis launch chain:

```text
operator steering
-> autonomous loop claim
-> safe result
-> Forum post
-> /artanis public summary
```

It is a verification contract, not a production scheduler. It does not grant
Artanis authority to spend money, mutate providers, promote runtime behavior,
launch training, settle payouts, or post without the separate Forum delivery
path.

By itself this projection does not prove that production D1 retained the rows
or that an operator ran a controlled production-equivalent window. That
retained evidence is owned by the #417 retained-smoke contract.

## Implementation

Code lives in:

- `workers/api/src/artanis-launch-smoke.ts`
- `workers/api/src/artanis-launch-smoke.test.ts`

The smoke composes existing projections:

- operator steering:
  `workers/api/src/artanis-operator-steering.ts`
- autonomous loop:
  `workers/api/src/artanis-loop.ts`
- Forum publication queue:
  `workers/api/src/artanis-forum-publication.ts`
- public Artanis summary:
  `workers/api/src/artanis-public-report.ts`

## Required Chain

The smoke requires:

- an accepted or completed `create_goal` command for `agent_artanis`;
- an active Artanis loop for the same public goal ref;
- a completed loop tick for that goal;
- a safe `status_projection` action result;
- a delivered Artanis Forum publication intent with a public post ref;
- `/artanis` included in public report URLs;
- `/artanis` linking the public Artanis Forum section;
- public-safe receipt and artifact refs.

## Public Projection

The smoke projection includes:

- `goalRef`
- `loopRef`
- `tickRef`
- `safeActionRef`
- `forumIntentRef`
- `forumPostRef`
- `forumTopicRef`
- public summary refs for `/artanis`, the report route, and Forum links;
- receipt refs;
- artifact refs;
- stage rows for operator goal, loop claim, safe result, Forum post, and
  public summary.

## Blocked Before Authority

The smoke intentionally records blockers that remain true before production
launch claims:

- live spend requires an operator-approved spend gate;
- provider mutation requires separate authority;
- runtime promotion requires a release gate;
- settlement requires public receipt chains.

Those blockers are part of the smoke output so launch notes cannot accidentally
claim that Artanis has live spend, provider mutation, runtime promotion, or
settlement authority.

## Safety Boundary

The smoke rejects:

- missing delivered Forum post;
- public report projections that do not link `/artanis`;
- provider, runner, wallet, payment, customer, private repo, secret, raw
  prompt, raw log, raw payload, raw source archive, and raw timestamp material.

The smoke also inherits the existing safety checks from operator steering,
loop, Forum publication, and public report projections.

## Verification

Coverage lives in `workers/api/src/artanis-launch-smoke.test.ts`.

The tests cover:

- the full operator steering -> loop claim -> safe result -> Forum post ->
  `/artanis` summary chain;
- public-safe receipt, artifact, Forum, and summary refs;
- blockers before live spend, provider mutation, runtime promotion, and
  settlement;
- missing delivered Forum post rejection;
- missing `/artanis` summary link rejection;
- unsafe public material rejection.
