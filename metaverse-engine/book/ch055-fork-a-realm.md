# 55. Fork a realm

> **Status: partly implemented.** Any made realm forks: the server copies
> the document at the revision forked (the published revision for anyone
> but the owner) into a new realm at its own revision 1 that records
> `forked_from {slug, rev}`, and pins it to its parent's engine
> ([chapter 54](ch054-publish-a-realm.md)). The OpenAgents Metaverse Engine reads that
> provenance ([metaverse-engine/editor/doc.coax](../editor/doc.coax)); a fork's edits
> are its own log in chapter 52's model, they leave the parent's document
> byte-identical, and the parent's revision plus the fork's log replays to
> the fork. The example below runs natively in debug and release, and the
> server's side is checked against PostgreSQL. Everglade's forks also switch
> rules its code declares (`verse.fork_bases`, 0075). Not built yet: rule
> modules any base declares with the state each touches, the check that a
> rule changes only that state, and a fork's session replayed natively and
> in the browser. See [Still to do](#still-to-do).

A fork is your own copy of someone's realm with your own changes: always
night, the watch in waves, a tree turned, a rock taken away. The original
never changes. For that to hold for every realm, a fork cannot be a copy of
code; it is a reference to a published version, the switches the base
declares, and its own edits on top.

## What this chapter teaches

- A fork as `(parent revision, edit log)`: it starts as its parent's realm
  and replays from it.
- Provenance: forked from what, at which revision, chained through forks of
  forks.
- Isolation: a fork's edits and saves never reach its parent.
- Rules as switches the base declares, refused when it doesn't.

## What the server makes

`POST /api/playtest/realms/<slug>/fork` (`verse.made_fork`,
[0094_realm_live_and_undo.sql](../../coax/web/api/migrations/0094_realm_live_and_undo.sql))
makes a new made realm owned by whoever forked it, named "NAME Fork" unless a
name is given. Its document is a copy of the parent's at the revision forked:
moves, and through triggers the shapes
([0109_realm_shapes.sql](../../coax/web/api/migrations/0109_realm_shapes.sql))
and the lore and quests
([0111_realm_edit_log.sql](../../coax/web/api/migrations/0111_realm_edit_log.sql))
of that revision. Anyone but the owner forks the revision published, not the
owner's unpublished saves. The fork starts at its own revision 1, names its
parent in `forked_from`, and is pinned to the parent's engine
([0107_realm_engine_pins.sql](../../coax/web/api/migrations/0107_realm_engine_pins.sql)).
Its page says "Forked from NAME at revision N." and its spec carries
`from SLUG REV|NAME`. A fork of a fork names its own parent, so the chain is
followed one link at a time.

## Reading the parent

The reader takes `forked_from` as part of the document. It is `null` (or
absent) for a realm that isn't a fork; otherwise both fields must hold, by
the server's own slug rule, or the whole document is refused:

<!-- source: metaverse-engine/editor/doc.coax fork -->
```jai
  // A fork names the realm it was copied from and that realm's revision: {"slug": S, "rev": N}.
  parent := jv_get(j[0], at, "forked_from");   // null, or absent, when it isn't a fork
  if jv_kind(j[0], parent) > JV_NULL {
    buf: [EDITOR_SLUG + 1] u8; slug := editor_str(j[0], jv_get(j[0], parent, "slug"), buf[..]);
    got.from_rev = editor_whole(editor_num(j[0], jv_get(j[0], parent, "rev")), 1, EDITOR_REV_MOST);
    if jv_kind(j[0], parent) != JV_OBJECT || !editor_slug_ok(slug) || got.from_rev < 1 {
      return editor_why("a fork's parent out of bounds");
    }
    for i: 0..<slug.count { got.from[i] = slug[i]; }
    got.from_n = slug.count;
  }
```

## A fork is its parent plus its log

A fork's edits are chapter 52's typed operations in the fork's own model.
Replaying that log onto a fresh copy of the parent's revision gives the fork
again, which is what makes `(parent revision, edit log)` a complete
description of a fork:

<!-- source: metaverse-engine/editor/edit.coax replay -->
```jai
// Replay: the applied steps of `from`, in order, onto `d` as steps of its own. False at the first
// step whose before no longer matches (the document changed under it); the steps before stay.
editor_replay :: (d: [] EditorDoc, from: [] EditorDoc) -> bool {
  i := 0;
  while i < from[0].done {
    end := i + 1; while end < from[0].done && from[0].ops[end].joined { end += 1; }
    o: [EDITOR_OPS] EditorOp;
    if !editor_room(d, from, i, end) { return false; }
    for k: i..<end {
      o[k - i] = from[0].ops[k];
      if o[k - i].kind == EDITOR_SLOT {   // its values move into this document's text
        if o[k - i].has { o[k - i].av = editor_keep(d, editor_span(from, o[k - i].av, o[k - i].av_n)); }
        if o[k - i].had { o[k - i].bv = editor_keep(d, editor_span(from, o[k - i].bv, o[k - i].bv_n)); }
      }
    }
    if !editor_push(d, o[0..<end - i]) { return false; }
    i = end;
  }
  return true;
}
// Room in d's text for the values of from's ops i..<end (packed first when it needs to be), so
// keeping them packs nothing away: a value kept for an op not yet in the log is named by nothing.
editor_room :: (d: [] EditorDoc, from: [] EditorDoc, i: int, end: int) -> bool {
  need := 0;
  for k: i..<end { o := from[0].ops[k]; if o.kind == EDITOR_SLOT { if o.has { need += o.av_n; } if o.had { need += o.bv_n; } } }
  if d[0].text_n + need > EDITOR_TEXT { editor_pack(d); }
  return d[0].text_n + need <= EDITOR_TEXT;
}
```

### Worked by hand

Lamp Cove is published at revision 4 with `trees:3` at (125, −250) cm and
`rocks:0` at (−4000, 7) cm, neither turned. A visitor forks it. The fork's
document is the same JSON with `"rev": 1` and
`"forked_from": {"rev": 4, "slug": "lamp-cove"}`. The fork then makes three
edits, each one step with its before and after:

| Step | Edit | Before | After |
| --- | --- | --- | --- |
| 1 | turn `trees:3` 90° | yaw 0, scale 100 | yaw 90, scale 100 |
| 2 | size `trees:3` 150% | yaw 90, scale 100 | yaw 90, scale 150 |
| 3 | remove `rocks:0` | not gone | gone |

Every step's before is what the parent's revision 4 has (step 2's is what
step 1 left), so replaying the three steps onto a fresh copy of revision 4
applies all three and ends where the fork is. The fork's save names its own
revision, 1, and carries both overrides with their places unchanged:
`trees:3` at 1.25, −2.50 m turned 90° at 150%, and `rocks:0` at −40.00,
0.07 m, gone. Nothing in the parent's model is written.

## Fork Lamp Cove

The program is
[metaverse-engine/examples/fork-a-realm.coax](../examples/fork-a-realm.coax):

<!-- source: metaverse-engine/examples/fork-a-realm.coax example -->
```jai
#load "../editor/doc.coax";
#import "Basic";

// The text with its first `from` replaced by `to`.
swap :: (t: string, from: string, to: string, buf: [] u8) -> string {
  for i: 0..<t.count {
    if i + from.count <= t.count && t[i..<i + from.count] == from {
      n := editor_put(buf, 0, t[0..<i]); n = editor_put(buf, n, to); n = editor_put(buf, n, t[i + from.count..<t.count]);
      return cast(string) buf[0..<n];
    }
  }
  return "";
}
// A model's text without its first line (the revision): the realm itself.
realm :: (d: [] EditorDoc, buf: [] u8) -> string {
  n := editor_doc_text(d, buf);
  at := 0; while buf[at] != '\n' { at += 1; }
  return cast(string) buf[at + 1..<n];
}

// Lamp Cove at its published revision 4, and the fork the server made of it: the same document
// at the fork's own revision 1, naming its parent. The fork's edits are its own log; the parent's
// model doesn't change, and the parent plus the fork's log is the fork.
main :: () {
  parent: [1] EditorDoc; editor_reset(parent[..], 1);
  read := editor_read_doc(parent[..], EDITOR_DOC_V1);
  print("parent read: %, revision %\n", read.ok, parent[0].rev);
  a: [4096] u8; b: [4096] u8;
  doc := swap(swap(EDITOR_DOC_V1, "\"rev\": 4", "\"rev\": 1", a[..]), "\"forked_from\": null",
              "\"forked_from\": {\"rev\": 4, \"slug\": \"lamp-cove\"}", b[..]);
  fork: [1] EditorDoc; editor_reset(fork[..], 1);
  r := editor_read_doc(fork[..], doc);
  print("fork read: %, revision %, forked from % at revision %\n", r.ok, fork[0].rev, cast(string) r.from[0..<r.from_n], r.from_rev);
  p0: [8192] u8; f0: [8192] u8;
  print("the fork starts as its parent's realm: %\n", realm(parent[..], p0[..]) == realm(fork[..], f0[..]));
  before: [8192] u8; bn := editor_doc_text(parent[..], before[..]);
  edited := editor_rotate(fork[..], "trees:3", 90) && editor_resize(fork[..], "trees:3", 150) && editor_remove(fork[..], "rocks:0");
  print("the fork turns trees:3, makes it 150%%, takes rocks:0 away: %, % steps in its log\n", edited, fork[0].done);
  after: [8192] u8; an := editor_doc_text(parent[..], after[..]);
  print("the parent is byte-identical: %\n", cast(string) before[0..<bn] == cast(string) after[0..<an]);
  base: [1] EditorDoc; editor_reset(base[..], 1);
  replayed := editor_read_doc(base[..], EDITOR_DOC_V1).ok && editor_replay(base[..], fork[..]);
  p1: [8192] u8; f1: [8192] u8;
  print("parent + the fork's log replays: %, and is the fork: %\n", replayed, realm(base[..], p1[..]) == realm(fork[..], f1[..]));
  body: [2048] u8; n := editor_save_body(fork[..], body[..]);
  print("the fork's save: %\n", cast(string) body[0..<n]);
  c: [4096] u8;
  no := editor_read_doc(fork[..], swap(doc, "\"rev\": 4, \"slug\"", "\"rev\": 0, \"slug\"", c[..]));
  print("a parent at revision 0 -> refused: %; the fork keeps its % edits\n", no.why, fork[0].done);
}
```

```shell
coax/bin/coax run metaverse-engine/examples/fork-a-realm.coax
```

```text
parent read: true, revision 4
fork read: true, revision 1, forked from lamp-cove at revision 4
the fork starts as its parent's realm: true
the fork turns trees:3, makes it 150%, takes rocks:0 away: true, 3 steps in its log
the parent is byte-identical: true
parent + the fork's log replays: true, and is the fork: true
the fork's save: {"rev":1,"moved":[{"entity":"rocks:0","x":-40.00,"z":0.07},{"entity":"trees:3","x":1.25,"z":-2.50}],"shaped":[{"entity":"rocks:0","yaw":0,"scale":100,"gone":true},{"entity":"trees:3","yaw":90,"scale":150,"gone":false}]}
a parent at revision 0 -> refused: a fork's parent out of bounds; the fork keeps its 3 edits
```

The save is the hand-worked one. The last line is the broken case: a fork
document whose parent revision is 0 is refused whole, and the fork's model,
with its three edits, stays as it was.

## Checks

- `coax/bin/coax test metaverse-engine/editor/doc.coax --library`: a document that
  isn't a fork reads with no parent; a fork reads with its parent's slug and
  revision; a slug at the server's 64-character limit reads and one past it
  doesn't; seven broken parents (revision 0, a quoted revision, a capital
  in the slug, an empty slug, no slug, a bare string and a list) are each
  refused with "a fork's parent out of bounds" and change nothing.
- The independent check is the server. `coax/web/tool.sh
  coax/web/api/tests/realm_walkthrough.coax` runs the route against
  PostgreSQL ([chapter 54](ch054-publish-a-realm.md#the-route-checked-end-to-end)):
  a visitor's fork starts on its parent's pinned engine; its document is the
  published revision's moves and shapes, not the owner's later unpublished
  save; the fork's own edits, saved, leave the parent's published and current
  documents untouched.
- `coax/web/api/test_realm_fork.py` (in `coax/web/api/test.sh`): only the
  owner saves a fork; a visitor's fork copies the published revision.
  `coax/web/api/tests/realm_cli_api.coax` does the same with the built CLI
  against a local API: a fork's rotate leaves the parent's revision and turn
  as they were.

## Rules a base declares

Everglade is the one base with rules today
([docs/metaverse/realm-fork.md](../../docs/metaverse/realm-fork.md)).
`verse.fork_bases.rules`
([0075_realm_forks.sql](../../coax/web/api/migrations/0075_realm_forks.sql))
lists what Everglade's code really switches: `night`, `waves`, `nodestroy`
and `lowgrav`. `POST .../everglade/fork {rules}` refuses a rule the base
doesn't list with `400`. The FORK pane is drawn in Coax
([everglade-fork.coax](../../apps/openagents-web/everglade-fork.coax), with
its own `#test`s); the page facts carry the rules first (`rules=night,waves`,
`fork=<slug>` from `coax/web/host.js`), then the fork's board decides.
`waves` reuses the watch's own escalation
([everglade/legacy/v1/watch.coax](../../everglade/legacy/v1/watch.coax)). A made
realm forks without rules.

## Still to do

- **Rules as typed modules.** `metaverse-engine/editor/fork.coax`: a `Rule` declared by
  the base's code with the state it touches, and a `Fork` as
  `(base record, rule mask, edit log)` in one type. Today Everglade's rules
  are a SQL list and are honoured by checks scattered through its module.
- **Rule isolation.** A check any base runs over its own rules: the base
  with one rule on, over a recorded session, changes only the fields that
  rule names.
- **Bases beyond Everglade.** Each realm declaring its rule table.
- **Session replay of a fork.** A fork's play session replayed from base
  version, rules and log to the same final state natively and in the
  browser. Today the document replays (above); a play session doesn't yet.
- **ORIGINAL for made realms.** Everglade's fork pane has ORIGINAL; a made
  realm's fork names and links its parent but has no such control.

### Acceptance checks still open

- Making a fork with a rule the base does not declare is refused by the type
  as well as by the route's 400.
- Rule isolation over a recorded session for each declared rule.
- A fork's session replays natively and in the browser to the same state.
- A fork of a fork records both links in one check, and ORIGINAL returns to
  the version it forked.

## Dependencies

[Chapter 5](ch005-replay-a-session-exactly.md),
[chapter 52](ch052-edit-a-realm-in-place.md) and
[chapter 54](ch054-publish-a-realm.md). Typed rule modules follow the same
idea as [chapter 48](ch048-one-world-at-three-fidelities.md)'s; that comparison
does not make the spacecraft fidelity ladder a prerequisite for an ordinary
realm fork. A fork of a spacecraft profile must additionally preserve and
check that profile's declared rules.

## In the other books

A fork records where it came from. That record is what a residual to the
parent's maker would be paid from; the rule for splitting a payment between
a fork and its parent is not written yet. Keys and payments are built in
[Nostr chapter 71](../../nostr/book/ch071-lightning-zaps-and-goals.md) and
[Bitcoin chapter 114](../../bitcoin/book/ch114-invoices.md).
