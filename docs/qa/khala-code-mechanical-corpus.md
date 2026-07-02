# Khala Code Mechanical Corpus

Status: ROADMAP_QA Q4.1 / issue #8027 implemented.

The fixture seed corpus in `packages/khala-qa-harness/src/seed-corpus.ts`
now enumerates the Q4.1 RPC groups as first-class manifest coverage:

- threads, turns, approvals, settings/config, models/personality
- ecosystem, fs/mentions/attachments, background terminals, slash commands
- token summaries, fleet, FleetRun, session catalog, forum panel
- inbox routing, gym pane sources, plans/billing, headless event sources,
  and QA metrics

Each scenario phase has at least one oracle. The scenario loader rejects an
oracle-less phase, and `seed-corpus.test.ts` pins that failure mode directly.

The coverage ledger now carries the manifest counts forward as run artifacts:
`rpcGroups`, `fleetRunControlVerbs`, and `inboxRoutingFlagKinds` sit beside
the existing RPC method, slash-command, settings-key, approval-decision, and
ThreadItem counters. The frontier report can therefore distinguish "this
method has never run" from "this roadmap group has never run."

Boundary note: Q4.1 is the fixture RPC corpus. Inbox routing is covered at
the RPC boundary through the status source RPCs and `fleetWorkerControl`
verbs (`interrupt`, `retry`, `flag`). Gym and headless event coverage is
covered through the source RPC projections those surfaces consume. DOM-level
flag-kind rendering and cross-mode state equivalence remain owned by the
headed/cross-mode roadmap items.
