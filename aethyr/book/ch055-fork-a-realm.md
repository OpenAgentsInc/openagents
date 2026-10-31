# 55. Fork a realm

> **Status: unimplemented — design draft.** Everglade can be forked today
> (FORK in the realm, `verse.fork_bases`, migration `0075_realm_forks.sql`):
> a fork is a made realm played by Everglade's module with its own rules and
> its own city. This chapter plans forks of any realm as a base version plus
> declared rule switches plus an edit log. Nothing here runs yet.

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

Only Everglade is a fork base, its rules are honoured by checks scattered
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
idea as [chapter 48](ch048-one-world-at-three-fidelities.md)'s.
