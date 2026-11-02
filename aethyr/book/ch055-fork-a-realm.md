# 55. Fork a realm

> **Status: unimplemented — design draft.** Rules declared by any base and a
> fork as base version, rule switches and edit log are not built. What runs
> today: Everglade forks with its own rules
> (`0075_realm_forks.sql`), and any made realm forks as a copy of its
> document that records its parent and the revision it came from
> (`0093_realm_documents.sql`). Anyone but the owner forks the revision
> published, not the owner's unpublished saves
> (`0094_realm_live_and_undo.sql`).

A fork is your own copy of someone's realm with your own rules: always
night, the watch in waves, nothing breakable, a third of the gravity. The
original never changes. For that to hold for every realm, a fork cannot be a
copy of code; it is a reference to a published version, a set of switches
the base's code declares, and its own edits on top.

## What this chapter teaches

- Rules as typed modules the base declares, each naming the state it changes.
- A fork as `(base version, rule switches, edit log)`, so it replays.
- Provenance: forked from what, made by whom, chained through forks of forks.
- Isolation: a fork's edits and saves never reach its base.

## The method

The base exports its rule table; a fork selects from it:

```jai
Rule :: struct { id: RuleId; label: string; touches: StateMask; }   // declared by the base's code
Fork :: struct {
    base: Published;              // chapter 54's record, by hash
    rules: RuleMask;              // only ids the base declares
    edits: EditLog;               // chapter 52, on the fork's own world
    forked_from: Slug; maker: Key;
}
```

A rule may change only the state its `touches` mask names; a check runs the
base with and without each rule and compares every other field. The fork's
world starts as the base's published world and its edit log applies on top,
so ORIGINAL is the base with no rules and no edits, and the fork replays from
its base plus its log (chapter 5). A rule the base does not implement is
refused when the fork is made, not ignored when it plays.

## What exists today

- [docs/verse/realm-fork.md](../../docs/verse/realm-fork.md): Everglade's
  rule sets (`night`, `waves`, `nodestroy`, `lowgrav`), the routes
  (`GET` and `POST /api/playtest/realms/<slug>/fork`, `/publish`), names,
  the fork's page and its `noindex` until published.
- [apps/openagents-web/everglade-fork.coax](../../apps/openagents-web/everglade-fork.coax):
  the FORK pane, drawn in Coax, with its own `#test`s.
- The page facts carry the rules first (`rules=night,waves`, `fork=<slug>`
  from `coax/web/host.js`); once the edit layer has the fork's spec, its
  board decides.
- `waves` reuses the watch's own escalation
  ([verse/everglade/watch.coax](../../verse/everglade/watch.coax)).

- Any made realm forks: FORK in the realm, for anyone who may make realms,
  calls `POST /api/playtest/realms/<slug>/fork` and opens the fork. The fork
  is a new made realm owned by whoever forked it, named "NAME Fork" unless a
  name is given. Its document is a copy of the original's at that revision,
  with `"forked_from": {"slug": ..., "rev": N}`; it starts at its own
  revision 1. Its page says "Forked from NAME at revision N." and its spec
  carries `from SLUG REV|NAME`.
- Saving the fork never touches the original: the API test and the journey
  check that the original's document hash and revision are the same after
  the fork is edited and saved.

Only Everglade has rules a fork switches, its rules are honoured by checks scattered
through its module, and nothing checks that a rule touches only what it says.

## What it builds

- `aethyr/forge/fork.coax`: `Rule`, `Fork`, making and replaying a fork,
  ORIGINAL.
- A rule-isolation check any base runs over its own rules.
- Fork bases beyond Everglade: each realm declaring its rule table.

## Acceptance checks and oracles

- Making a fork with a rule the base does not declare is refused (the
  route's 400 today, the type here).
- A fork's edits and saves leave the base's world and slot byte-identical.
- Rule isolation: running the base with one rule on changes only the fields
  in its `touches` mask over a recorded session.
- Replay: a fork's session replays from base version, rules and log to the
  same final state natively and in the browser.
- A fork of a fork records both links, and ORIGINAL returns to the version
  it forked.

## Dependencies

[Chapter 5](ch005-replay-a-session-exactly.md),
[chapter 52](ch052-edit-a-realm-in-place.md) and
[chapter 54](ch054-publish-a-realm.md). Typed rule modules follow the same
idea as [chapter 48](ch048-one-world-at-three-fidelities.md)'s; that comparison
does not make the spacecraft fidelity ladder a prerequisite for an ordinary
realm fork. A fork of a spacecraft profile must additionally preserve and
check that profile's declared rules.
