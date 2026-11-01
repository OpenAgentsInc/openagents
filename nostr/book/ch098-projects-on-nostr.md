# 98. Projects on Nostr

The OpenAgents work queue lives on Nostr. A project is a kind 30621 event,
its tasks are NIP-34 issues, its workers report on them with NIP-22
comments, and a status event closes each one. Nothing else holds the queue:
the website's project pages read these events from
`wss://relay.openagents.com` and fold them into tables. This chapter shows
that fold, the gate that keeps a forged or out-of-scope event out of the
verified view, and a check of the fold against tables recorded from a real
relay capture.

Part XII, Nostr in OpenAgents. Result: OpenAgents' project pages fold NIP-34
repositories, issues and statuses and NIP-22 comments from the relay. The
homepage's project rows verify each event's id and BIP-340 signature in Coax
before the fold reads it, and a page replaces its tables only with a
complete bounded capture.

## What this chapter teaches

- The events a project is made of: the project (30621), its order (30623),
  its repository (30617), issues (1621), statuses (1630 to 1633) and
  comments (1111) that claim, plan, note, release or finish a task.
- The store: which events it keeps (the project key's, of those kinds, new)
  and which it drops.
- The fold: how each status and comment finds its issue, which status wins,
  and how claims, plans and commits become the rows the page draws.
- The verified view: every event is verified before the store reads it, and
  a capture that fails anywhere does not replace the tables already shown.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [project-fold.coax](../../apps/openagents-web/project-fold.coax) | `store_event(s, j, pubkey)` | Keeps one event of the project key, of a kind the fold reads, once |
| [project-fold.coax](../../apps/openagents-web/project-fold.coax) | `fold(s, f, slug)` | Finds the project, order, repository and issues, and hangs statuses and comments off their issues |
| [project-fold.coax](../../apps/openagents-web/project-fold.coax) | `fold_one(s, f, r)` | One issue's fields, status, worker, commit, notes, plan and log lines |
| [project-fold.coax](../../apps/openagents-web/project-fold.coax) | `write_tables(s, f, t)` | The page's `WORK_ITEMS`, `WORK_LOG`, `WORK_PLANS`, `WORK_PLAN_INFO` and `WORK_DETAILS` tables |
| [project-fold.coax](../../apps/openagents-web/project-fold.coax) | `frame_head`, `read_frame` | A relay frame's envelope, and an `EVENT` frame read into the store |
| [landing-fold.coax](../../coax/web/project/landing-fold.coax) | `lf_feed` | The homepage's gate: an event enters the store only with its verification bit, and any refusal fails the capture |
| [project-live.coax](../../apps/openagents-web/project-live.coax) | `live_prepare` | A project page's catch-up: a complete candidate replaces the shown tables, an incomplete one doesn't |
| [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_verify_vartime` | Chapter 21's event check: the id recomputed and the BIP-340 signature verified |

All of this existed before the chapter. The fold reads every tag through
the typed readers of chapter 63 (`n34_project`, `n34_order`, `n34_repo`,
`n34_issue`, `n34_fields`, `n34_status`) and chapter 52's `n22_comment`. The
chapter adds listing markers to `project-fold.coax`, `project-live.coax` and
`landing-fold.coax`.

It changes two library files by renaming one local name in each, with no
change in behavior. `coax0` has one flat namespace for procedures and
variables. `project-fold.coax` defines procedures named `fold` and
`event_id`, and `secp_field.coax` used `fold` and `schnorr.coax` used
`event_id` as local names, so no program could load the fold and the
verifier together. The website avoids this by compiling them into separate
units joined by C. The example needs both in one program, so `sf_multiply`'s
local `fold` is now `reduced` and the two Nostr verify entry points' parameter
`event_id` is now `nostr_id`.

## The events of a project

A project names itself with a `d` tag, and everything under it points at
its address, `30621:<owner key>:<d>`:

| Kind | What | How it points at its project or issue |
| --- | --- | --- |
| 30621 | The project | Its first `d` tag is its slug |
| 30623 | The order: `item` tags, each an issue id and a rank | Its first `d` tag is the project's slug |
| 30617 | The repository | The project's first `a` tag names it (`30617:<key>:<d>`) |
| 1621 | An issue: one task, milestone or initiative | An `a` tag equal to the project's address |
| 1630–1633 | A status: open, resolved, closed or draft | Its `e` tag marked `root`, else its first `e` |
| 1111 | A comment | Its NIP-22 root scope: an `E` tag with `K` 1621 |

A comment's other tags say what it does. `claim` names the worker who takes
the task, `release` gives it back, `plan` with the value
`openagents.task-plan.v1` carries a checklist in the content, `commit` says
the task is done in that commit, `by` names who wrote it, and `revision`
with `openagents.issue-revision.v1` replaces the issue's fields.

### The store keeps the owner's events

The page sends five subscriptions, all with `authors` set to the project's
key. Whatever the relay answers, `store_event` keeps an event only when its
`pubkey` is that key, its kind is one of those above, all six fields are
present and well formed, and its id is not already kept. Anything else is
dropped whole and the arena gives its bytes back. So a status by another
key never reaches the fold, even when its signature is valid.

### The fold

`fold` makes one pass for the project and order, one for the repository,
one for the issues and one for what hangs off them:

<!-- source: apps/openagents-web/project-fold.coax fold -->
```jai
fold :: (s: [] Store, f: [] Fold, slug: string) {
  f[0].n = 0; f[0].log_n = 0; f[0].notes_n = 0; f[0].project = -1; f[0].repo = -1; f[0].order = -1; f[0].items_n = 0;
  // the project and its order: the latest of each whose address is the slug
  for e: 0..<s[0].n {
    f[0].under[e] = -1;
    k := s[0].kind[e];
    if k == KIND_PROJECT {
      ev := store_nt(s, e); p := n34_project(ev);
      if p.status == N34_OK && p.d.n == 1 && json_is(nt_raw(ev, p.d.v[0]), slug) && newer(s, e, f[0].project) { f[0].project = e; }
    }
    if k == KIND_ORDER {
      ev := store_nt(s, e); o := n34_order(ev, f[0].items[..]);
      if o.status == N34_OK && json_is(nt_raw(ev, o.d.v[0]), slug) && newer(s, e, f[0].order) { f[0].order = e; }
    }
  }
  if f[0].order >= 0 { f[0].items_n = n34_order(store_nt(s, f[0].order), f[0].items[..]).items; }
  // the repository the project names in its first a tag (30617:KEY:D), else any
  repo_d := "";
  if f[0].project >= 0 {
    ev := store_nt(s, f[0].project); p := n34_project(ev);
    if p.a.n > 0 {
      // the coordinate's kind and key are plain bytes, so its d starts at the same place unescaped
      c := nt_coord(nt_raw(ev, p.a.v[0])); a := span_str(s, f[0].project, p.a.v[0]);
      if c.kind == KIND_REPO && c.d <= a.count { repo_d = a[c.d..<a.count]; }
    }
  }
  for e: 0..<s[0].n {
    if s[0].kind[e] != KIND_REPO { continue; }
    ev := store_nt(s, e); r := n34_repo(ev);
    if r.status == N34_OK && (repo_d.count == 0 || equal(span_str(s, e, r.d.v[0]), repo_d)) && newer(s, e, f[0].repo) { f[0].repo = e; }
  }
  // the issues: kind 1621 with an a tag 30621:KEY:SLUG, oldest first, one per slug
  roots: [FOLD_ISSUES] int; rn := 0;
  for e: 0..<s[0].n {
    if s[0].kind[e] != KIND_ISSUE || rn >= FOLD_ISSUES { continue; }
    ev := store_nt(s, e); i := n34_issue(ev);
    if i.status != N34_OK { continue; }
    under := false;
    for k: 0..<i.a { if is_coord(nt_raw(ev, nt_tag(ev, "a", k).v[0]), slug) { under = true; } }
    if !under { continue; }
    k := rn; rn += 1;
    while k > 0 && older(s, e, roots[k - 1]) { roots[k] = roots[k - 1]; k -= 1; }
    roots[k] = e;
  }
  for k: 0..<rn {
    e := roots[k];
    ev := store_nt(s, e); i := n34_issue(ev);
    sl := ifx i.slug.n > 0 then span_ref(s, e, i.slug.v[0]) else Ref.{from = s[0].id[e], len = 8};
    if sl.len < 0 { sl = Ref.{}; }
    taken := false;
    for r: 0..<f[0].n { if equal(str(s, f[0].slug[r]), str(s, sl)) { taken = true; } }
    if taken { continue; }
    r := f[0].n; f[0].n += 1;
    f[0].root[r] = e; f[0].slug[r] = sl;
  }
  // what hangs off each issue: a comment by its NIP-22 root scope, a status by its NIP-34 root
  for e: 0..<s[0].n {
    k := s[0].kind[e];
    if k == KIND_COMMENT {
      f[0].c_marks[e] = 0; f[0].c_by[e] = Ref.{}; f[0].c_claim[e] = Ref.{}; f[0].c_commit[e] = Ref.{};
      ev := store_nt(s, e); c := n22_comment(ev);
      if c.status != N34_OK || c.root.at != 'E' || c.root.kind_value != KIND_ISSUE { continue; }
      f[0].under[e] = fold_issue_of(s, f, nt_raw(ev, c.root.ref.v[0]));
      m := C_OK;
      if c.revised { m |= C_REVISED; }
      if c.revision.n > 0 { m |= C_REVISION; }
      if c.planned { m |= C_PLANNED; }
      if c.claim.n > 0 { m |= C_CLAIM; f[0].c_claim[e] = span_val(s, e, c.claim.v[0]); }
      if c.release > 0 { m |= C_RELEASE; }
      if c.commit.n > 0 { m |= C_COMMIT; f[0].c_commit[e] = span_val(s, e, c.commit.v[0]); }
      // by: the by tag, else the claim (model.ts); len -1 when neither
      f[0].c_by[e] = ifx c.by.n > 0 then span_ref(s, e, c.by.v[0]) else ifx c.claim.n > 0 then f[0].c_claim[e] else Ref.{from = 0, len = -1};
      f[0].c_marks[e] = m;
    } else if k >= 1630 && k <= 1633 {
      ev := store_nt(s, e); st := n34_status(ev);
      if st.status == N34_OK { f[0].under[e] = fold_issue_of(s, f, nt_raw(ev, st.root.v[0])); }
    }
  }
  for r: 0..<f[0].n { fold_one(s, f, r); }
  if f[0].order >= 0 { log_push(f, s[0].at[f[0].order], LOG_ORDER, -1, f[0].order, Ref.{}); }
  // aliases: four hex digits, more where two ids would share them
  for r: 0..<f[0].n {
    n := 4;
    for o: 0..<f[0].n {
      if o == r { continue; }
      a := event_id(s, f[0].root[r]); b := event_id(s, f[0].root[o]);
      while n < 64 && equal(a[0..<n], b[0..<n]) { n += 1; }
    }
    f[0].alias[r] = n;
  }
}
```

The rules come from the NIPs, as the
[2026-10-31 verification record](../../docs/verification/2026-10-31-project-fold-typed.md)
lists them:

- The latest project and order whose first `d` is the slug win, newest
  `created_at` first and the smaller id on a tie. A project with two `d`
  tags is refused.
- An issue is under the project when any of its `a` tags is the project's
  address. Issues are taken oldest first, one per slug.
- A status hangs off the issue its root `e` names. A comment hangs off the
  issue its NIP-22 root scope names, and only when that root is an `E` whose
  `K` is 1621. A comment rooted anywhere else hangs off nothing.
- An event its reader refuses is not part of the project.

`fold_one` then works through one issue. The newest status decides the
state; with none, the issue is open since its own `created_at`. The
comments are read oldest first: a claim sets the worker, a release clears
it, a commit sets the commit, a plan replaces the plan, and any other
comment with text is a note. A resolved or closed task has no worker.

### Worked example: project tea

The owner (BIP-340 test key 3, public key `f9308a01…36f9`) publishes nine
events. Times are seconds after `T0` = 1761686400, which is 2025-10-28
21:20 UTC and 16:20 in Chicago, the time zone the page shows:

| # | At | Kind | What |
| --- | --- | --- | --- |
| 0 | 0 | 30621 | project `tea` |
| 1 | 60 | 1621 | issue `timer`, "Fix the 59-second timer", size S, id `8427…` |
| 2 | 120 | 1621 | issue `scale`, "Add a grams scale", size M, id `d060…` |
| 3 | 130 | 30623 | order: `scale` rank `0a`, `timer` rank `0b` |
| 4 | 180 | 1111 | on `timer`: `claim w1` |
| 5 | 200 | 1111 | on `timer`: a plan by `w1` with steps `[x]`, `[>]`, `[ ]` |
| 6 | 240 | 1111 | on `scale`: a note by `w2`, "Needs a load cell first." |
| 7 | 300 | 1111 | on `timer`: `commit 4b825dc642` by `w1`, "Done in 4b825dc642." |
| 8 | 310 | 1631 | `timer` resolved |

Fold them by hand for slug `tea`:

1. **Project and order.** Event 0 is the only 30621 with `d` `tea`, and
   event 3 the only 30623. No 30617 exists, and the project has no `a` tag,
   so the repository is none.
2. **Issues.** Events 1 and 2 both carry `a`
   `30621:f9308a01…36f9:tea`. Oldest first, `timer` is issue 0 and `scale`
   is issue 1. Their ids differ in the first hex digit, so each alias is 4
   digits: `8427` and `d060`.
3. **What hangs where.** Events 4, 5 and 7 have `E` = `8427…` and `K` 1621,
   so they hang off `timer`; event 6 hangs off `scale`. Status 8's root `e`
   is `8427…`, so it hangs off `timer`.
4. **timer.** The newest status is event 8, kind 1631, so the state is
   1631 − 1630 = 1, resolved, at T0+310. The order gives it rank `0b`.
   Comment 4 makes `w1` the worker. Comment 5 is the plan: three lines
   match `- [m] text`, one with `x`, so 1 of 3 steps is done. Comment 7 sets
   commit `4b825dc642`; its text is a note, so the note count is 1. The task
   is resolved, so the worker is cleared: none.
5. **scale.** No status, so it is open since its own time, T0+120, with
   rank `0a`. Comment 6 has text and no claim, plan or commit, so it is a
   note by `w2`.
6. **WORK_ITEMS.** Rows go by rank, so `d060` (`0a`) comes before `8427`
   (`0b`). The resolved row ends with its done time, T0+310 = 1761686710.
7. **WORK_LOG.** The workers' lines are the claim (16:23), the plan
   (16:23), the note (16:24), the done (16:25) and the status (16:25),
   newest first. The status line is left out: the task's own done line is
   10 seconds before it, under the page's two-minute window.

Then five more events arrive, each meant to change the view:

- the owner's status 1632 closing `scale` (T0+400), with one signature
  digit changed;
- the same status with its kind changed to 1633 after signing;
- the same close, validly signed by another key (BIP-340 test key 1);
- an issue `grind` under project `coffee`;
- a claim of `scale` by `w3` whose `K` is 1111, not 1621.

The first two fail verification: the signature, and the id, which no longer
matches the changed bytes. They never reach the store. The third verifies,
but the store drops it as not the owner's. The fourth is kept, but its `a`
names `coffee`, so it is not in `tea`'s fold. The fifth is kept, but its
root is not an issue, so it hangs off nothing. `scale` stays open with no
worker.

## The verified view

The fold trusts what is in its store, so the gate is in front of the store.
On the homepage the C bridge calls the Coax verifier on the event's original
bytes and passes its bit to `lf_feed`, which refuses the event, and marks
the whole capture failed, when the bit is false or the store drops it:

<!-- source: coax/web/project/landing-fold.coax feed -->
```jai
lf_feed :: (s:[]LfState,text:string,verified:bool)->bool #export_c {
  if !s[0].active || s[0].failed {return false;}
  if !json_valid(text) {s[0].failed=true;return false;}
  if !verified {s[0].failed=true;return false;}
  j:[1]Json;json_start(j[..],text);
  if json_next(j[..])!=JSON_OBJECT {s[0].failed=true;return false;}
  previous:=s[0].store[0].dropped;
  e:=store_event(s[0].store[..],j[..],cast(string)s[0].key[..]);
  if s[0].store[0].overflow || s[0].store[0].dropped!=previous {s[0].failed=true;return false;}
  return true;
}
```

A failed capture returns no rows (`lf_result` gives −1), and the page keeps
the rows it showed before. The same rule holds for a stalled read: a project
page's catch-up builds candidate tables, and only a complete candidate
replaces the shown ones. Any refusal, timeout, page budget or missing known
issue calls `live_incomplete` instead, which goes offline and leaves the
last display alone:

<!-- source: apps/openagents-web/project-live.coax prepare -->
```jai
live_prepare :: (l: [] Live) {
  l[0].dirty = false;
  if !live_collect_known(l) { live_incomplete(l, 5); return; }
  l[0].batch_n = 0;
  for k: 0..<l[0].known_n {
    e := store_find(l[0].store[..], live_known_id(l, k));
    if !live_scoped_issue(l, e) && l[0].batch_n < CATCHUP_IDS { l[0].batch[l[0].batch_n] = k; l[0].batch_n += 1; }
  }
  if l[0].batch_n > 0 { l[0].state = LIVE_SYNCING; l[0].stage = 3; live_query_send(l, true); return; }
  fold(l[0].store[..], l[0].fold[..], live_slug(l));
  write_tables(l[0].store[..], l[0].fold[..], l[0].candidate[..]);
  if !l[0].candidate[0].complete { live_incomplete(l, 5); return; }
  l[0].state = LIVE_LIVE; l[0].dirty = true; l[0].backoff = BACKOFF_FIRST;
  if l[0].link_stage == LINK_IDLE { live_links_ask(l); }
}
```

The project page (`project-live.coax`) also refuses, before storage, a frame
for a subscription it didn't open, and treats an event of the wrong kind for
its subscription, or an issue not under the project, as an incomplete
capture. It does not verify signatures itself yet; see
[What is not built](#what-is-not-built).

## What is not built

- **Signature checks on the project page.** `project-live.coax` trusts the
  relay's signature check, as `project-fold.coax`'s header says. The
  homepage's project rows (`landing-fold.coax`) verify every event in Coax;
  the `/project/<slug>` pages do not yet.
- **The fold and the verifier in one Wasm unit.** The website still builds
  them apart and joins them in C. With this chapter's renames they compile
  together natively; the web build was not changed.
- **A stack-sized store.** The store, fold and tables are fixed arrays of
  about 15 MB, more than the 8 MiB default stack, so the example runs with
  a 32 MiB stack (`stack_kib` in [examples.json](examples.json)).

## The example

[nostr/examples/projects.coax](../examples/projects.coax) holds the
thirteen signed events of the worked example as a relay sends them. It
verifies and admits each one, prints what happened to it, folds `tea` and
prints the state and three of the page's tables, and folds `coffee`. Then it
reads the captured relay transcript `COAX_RELAY` (243 frames that
`wss://relay.openagents.com` sent for project `coax` on 2026-10-28 at
20:33 UTC, in
[coax_relay.coax](../../coax/lib/nostr/fixtures/coax_relay.coax)). It
verifies every event's signature in Coax, folds `coax`, and compares its
five tables with the ones `capture.ts` recorded from the TypeScript
model's `fold()` and `tables.ts` over the same events
([coax_relay_tables.coax](../../coax/lib/nostr/fixtures/coax_relay_tables.coax)).

The events and their signatures are constants. The signer still uses
`event_id` as a local name (in `schnorr_self_verify.coax`), so it can't
load beside the fold; the events were signed once by a separate program
with `nostr_event_sign_step` and copied in.

<!-- source: nostr/examples/projects.coax example -->
```jai
LABEL :: [13] string.["project tea", "issue timer", "issue scale", "order", "claim timer (w1)", "plan timer",
  "note scale (w2)", "done timer", "status timer resolved", "status scale closed", "status scale closed, another key",
  "issue grind under coffee", "claim scale, root kind 1111"];
NE_NAME :: [9] string.["ok", "json", "shape", "limit", "hex", "output", "alias", "id", "signature"];

// The verified view's gate, as coax/web/project/landing-fold.coax keeps it:
// the original bytes are verified first (id and BIP-340 signature), and only
// then does the store read them, keeping the owner's events of the kinds the
// fold reads. Returns the event's index, or -1.
admit :: (s: [] Store, text: string, label: string) -> int {
  scratch: [NE_MAX_OUTPUT] u8;
  v := nostr_event_verify_vartime(text, scratch[..], false);
  if v.error != NE_OK || !v.verified {
    print("  %: refused, %\n", label, NE_NAME[v.error]);
    return -1;
  }
  j: [1] Json; json_start(j[..], text);
  if json_next(j[..]) != JSON_OBJECT { print("  %: refused, not an object\n", label); return -1; }
  dropped := s[0].dropped;
  e := store_event(s, j[..], OWNER);
  if e >= 0 { print("  %: kept as event %\n", label, e); }
  else if s[0].last >= 0 { print("  %: already kept as event %\n", label, s[0].last); }
  else if s[0].dropped > dropped { print("  %: verified, dropped by the store (not the owner's)\n", label); }
  return e;
}
// Where needle first starts in text (it must be there).
find :: (text: string, needle: string) -> int {
  for i: 0..<text.count - needle.count + 1 { if equal(text[i..<i + needle.count], needle) { return i; } }
  #assert false;
  return -1;
}
// A copy of text with the character at i replaced by c.
altered :: (text: string, i: int, c: u8, out: [] u8) -> string {
  for k: 0..<text.count { out[k] = text[k]; }
  out[i] = c;
  return cast(string) out[0..<text.count];
}

show :: (s: [] Store, f: [] Fold) {
  print("project event %, order event %, repository %\n", f[0].project, f[0].order, ifx f[0].repo >= 0 then "kept" else "none");
  for r: 0..<f[0].n {
    root := f[0].root[r];
    print("  %  %: %, size %, rank %, status % at T0+%", event_id(s, root)[0..<f[0].alias[r]], str(s, f[0].slug[r]),
      KIND_NAMES[f[0].kind[r]], ifx f[0].size[r].len > 0 then str(s, f[0].size[r]) else "none", ifx f[0].rank[r].len > 0 then str(s, f[0].rank[r]) else "none",
      STATUS_NAME[f[0].status[r]], f[0].status_at[r] - s[0].at[0]);
    print(", worker %, commit %, notes %, plan %/%\n", ifx f[0].claimed[r].len > 0 then str(s, f[0].claimed[r]) else "none",
      ifx f[0].commit[r].len > 0 then str(s, f[0].commit[r]) else "none", f[0].note_cnt[r], f[0].steps_done[r], f[0].steps[r]);
  }
}

// The captured relay: every EVENT frame's object verified in Coax, then read
// as the page reads frames. The verified count, or -1 when one fails.
read_verified :: (s: [] Store, text: string) -> int {
  verified := 0; i := 0;
  while i < text.count {
    e := i;
    while e < text.count && text[e] != 10 { e += 1; }
    line := text[i..<e];
    if frame_head(line).type == FRAME_EVENT {
      j: [1] Json; json_start(j[..], line);
      a := json_next(j[..]); b := json_next(j[..]); c := json_next(j[..]); o := json_next(j[..]);
      if a != JSON_ARRAY || b != JSON_STRING || c != JSON_STRING || o != JSON_OBJECT { return -1; }
      scratch: [NE_MAX_OUTPUT] u8;
      v := nostr_event_verify_vartime(line[j[0].from..<line.count - 1], scratch[..], false);
      if v.error != NE_OK || !v.verified { return -1; }
      verified += 1;
      fr := read_frame(s, line, FOLD_TEST_KEY);
      if fr.event < 0 { return -1; }
    }
    i = e + 1;
  }
  return verified;
}
show_table :: (label: string, made: string, recorded: string) {
  print("  %: % bytes, % lines, %\n", label, made.count, count_lines(made), ifx equal(made, recorded) then "equal to the recorded table" else "DIFFERENT");
}
count_lines :: (text: string) -> int { n := 0; for i: 0..<text.count { if text[i] == 10 { n += 1; } } return n; }
first_lines :: (text: string, n: int) {
  k := 0; i := 0;
  while i < text.count && k < n {
    e := i; while e < text.count && text[e] != 10 { e += 1; }
    print("    %\n", text[i..<e]); k += 1; i = e + 1;
  }
}

main :: () {
  s: [1] Store; f: [1] Fold; t: [1] Tables; buf: [4096] u8;
  store_clear(s[..]);
  print("a small project, event by event\n");
  for k: 0..<9 { e := admit(s[..], EVENTS[k], LABEL[k]); }
  print("refusals\n");
  // The owner's close of scale, with one signature digit changed: forged.
  forged := EVENTS[9];
  sig := forged.count - 3;
  e9 := admit(s[..], altered(forged, sig, ifx forged[sig] == '0' then '1' else '0', buf[..]), "status scale closed, signature changed");
  // The same close with its kind changed after signing: the id no longer matches.
  e9 = admit(s[..], altered(forged, find(forged, "\"kind\":1632") + 10, '3', buf[..]), "status scale closed, kind changed to 1633");
  for k: 10..<13 { e := admit(s[..], EVENTS[k], LABEL[k]); }
  e1 := admit(s[..], EVENTS[1], "issue timer again");
  print("kept %, dropped %\n", s[0].n, s[0].dropped);

  print("the fold of project tea\n");
  fold(s[..], f[..], "tea");
  show(s[..], f[..]);
  write_tables(s[..], f[..], t[..]);
  print("WORK_ITEMS\n"); first_lines(cast(string) t[0].items[0..<t[0].items_n], 10);
  print("WORK_LOG\n"); first_lines(cast(string) t[0].log[0..<t[0].log_n], 10);
  print("WORK_PLANS\n"); first_lines(cast(string) t[0].plans[0..<t[0].plans_n], 10);
  print("the fold of project coffee\n");
  fold(s[..], f[..], "coffee");
  show(s[..], f[..]);

  print("a captured relay: relay.openagents.com, project coax, 2026-10-28T20:33Z\n");
  store_clear(s[..]);
  v := read_verified(s[..], COAX_RELAY);
  print("frames %, events verified %, kept %, dropped %\n", COAX_RELAY_FRAMES, v, s[0].n, s[0].dropped);
  fold(s[..], f[..], "coax");
  write_tables(s[..], f[..], t[..]);
  print("issues %, complete %\n", f[0].n, t[0].complete);
  show_table("WORK_ITEMS", cast(string) t[0].items[0..<t[0].items_n], COAX_RELAY_ITEMS);
  show_table("WORK_LOG", cast(string) t[0].log[0..<t[0].log_n], COAX_RELAY_LOG);
  show_table("WORK_PLANS", cast(string) t[0].plans[0..<t[0].plans_n], COAX_RELAY_PLANS);
  show_table("WORK_PLAN_INFO", cast(string) t[0].info[0..<t[0].info_n], COAX_RELAY_PLAN_INFO);
  show_table("WORK_DETAILS", cast(string) t[0].details[0..<t[0].details_n], COAX_RELAY_DETAILS);
  print("  the first three items\n"); first_lines(cast(string) t[0].items[0..<t[0].items_n], 3);
}
```

Run it with a larger stack:

```shell
(ulimit -s 32768 && coax/bin/coax run nostr/examples/projects.coax)
```

Its output, in debug and release builds alike
([nostr/fixtures/projects.out](../fixtures/projects.out)):

```text
a small project, event by event
  project tea: kept as event 0
  issue timer: kept as event 1
  issue scale: kept as event 2
  order: kept as event 3
  claim timer (w1): kept as event 4
  plan timer: kept as event 5
  note scale (w2): kept as event 6
  done timer: kept as event 7
  status timer resolved: kept as event 8
refusals
  status scale closed, signature changed: refused, signature
  status scale closed, kind changed to 1633: refused, id
  status scale closed, another key: verified, dropped by the store (not the owner's)
  issue grind under coffee: kept as event 9
  claim scale, root kind 1111: kept as event 10
  issue timer again: already kept as event 1
kept 11, dropped 1
the fold of project tea
project event 0, order event 3, repository none
  8427  timer: task, size S, rank 0b, status resolved at T0+310, worker none, commit 4b825dc642, notes 1, plan 1/3
  d060  scale: task, size M, rank 0a, status open at T0+120, worker none, commit none, notes 1, plan 0/0
WORK_ITEMS
    d060|task|open|Add a grams scale||M||||0a|scale||
    8427|task|resolved|Fix the 59-second timer||S||||0b|timer|4b825dc642|1761686710
WORK_LOG
    10-28 16:25|#8427 DONE IN 4b825dc642 BY w1 - Fix the 59-second timer
    10-28 16:24|#d060 w2: Needs a load cell first. - Add a grams scale
    10-28 16:23|#8427 W1 PLANNED 1 OF 3 DONE - Fix the 59-second timer
    10-28 16:23|#8427 CLAIMED BY W1 - Fix the 59-second timer
WORK_PLANS
    8427|x|read the timer loop
    8427|>|carry seconds into minutes
    8427|o|test 1:30
the fold of project coffee
project event -1, order event -1, repository none
  302e  grind: task, size none, rank none, status open at T0+420, worker none, commit none, notes 0, plan 0/0
a captured relay: relay.openagents.com, project coax, 2026-10-28T20:33Z
frames 243, events verified 237, kept 237, dropped 0
issues 105, complete true
  WORK_ITEMS: 11049 bytes, 105 lines, equal to the recorded table
  WORK_LOG: 4635 bytes, 29 lines, equal to the recorded table
  WORK_PLANS: 2325 bytes, 32 lines, equal to the recorded table
  WORK_PLAN_INFO: 177 bytes, 2 lines, equal to the recorded table
  WORK_DETAILS: 42185 bytes, 105 lines, equal to the recorded table
  the first three items
    65b7|initiative|open|Coax: the language and its toolchain, R0 to R12|coax compiler roadmap|||||0a|coax-roadmap||
    6f40|milestone|open|R0: the adoption gate||||65b7||1A|r0||
    d98b|milestone|open|R1: the compiler substrate||||65b7||1k|r1||
```

The state matches the worked example: `timer` is resolved at T0+310 with
commit `4b825dc642`, no worker and 1 of 3 plan steps done; `scale` is open
with its note and no worker. The changed signature is refused with
`signature` and the changed kind with `id`, before the store reads them; the
other key's close is dropped by the store; the `coffee` issue appears only
in `coffee`'s fold. For the capture, all 237 events verify, and each of the
five tables is byte for byte the one the TypeScript model recorded.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/projects.coax](../examples/checks/projects.coax),
which loads the example. They compare the code with values that share no
code with it:

- The ids of all thirteen events, from a Python script written from NIP-01
  (`hashlib.sha256` over NIP-01's serialization).
- The two forged events refuse with `NE_SIGNATURE` and `NE_ID`.
- The state worked by hand above: statuses, ranks, worker, commit, plan
  steps, notes, the `WORK_ITEMS` text, the four log lines, and the `coffee`
  issue only in `coffee`'s fold.

The fold of the captured relay is checked in the example itself against
the tables recorded from the TypeScript model, which is an independent
implementation of the same fold.

<!-- source: nostr/examples/checks/projects.coax check -->
```jai
// The ids from a Python script written from NIP-01 (it shares no code with
// Coax): hashlib.sha256 of json.dumps([0, pubkey, created_at, kind, tags,
// content], separators=(",", ":"), ensure_ascii=False) for each of EVENTS.
PY_ID :: [13] string.[
  "7d1c4fcbaed86a0d0ef849977fb5a7933e0202b9e677ff276dec2090a5dbb57d",
  "8427688078fc8b70441763e524ee10da7fd38f937fa48b9001ecb31fcaaa701e",
  "d060b7aa83a751d98d6785df43ba8ae6074347e2c68bb9cc6e6a3054f3253199",
  "43fc4473166737363330a5e9c63605080416d0ea884f36dede4131aefc0c53a9",
  "a50773addfdd1b99a1ccd70d518c45fc7ab3742f4df99a73b018dd2e38eb0363",
  "12b3ab9ed5c597c740777ed6f47f6b3bf362b5fd00b96634cf9c6c9abce8336b",
  "24e5f8b7a34dcfd18c4b034a22b8e75ec6a801921131fe72eb13140e29644d27",
  "e951a9517dd4c592c1e6bb28e197bef37760d397f1019d8d15f658bbb15b61b5",
  "e3eae8f9732838cb705cee0709306557c8ae2ded7573864f21e86071a60028d9",
  "edbb18aef2bf04a1854eb63f9d7096db3299e349f9635ce262569864799f6e18",
  "4aeed7eb892dfc030f97d9ad67de00b7d0aa3c5ca680099a647b28c566ff94c8",
  "302e3b6ddcf6c4b3504e0e7ab780572a434bf7be79820254f73dba20c7fd2397",
  "641cca7bc53779930b57a8a673ce0c39a3112718701daf03e7df7792d8f3d7da"];
T0 :: 1761686400;

// Event k verified, its id the script's; then into the store as the example does.
check_admit :: (s: [] Store, text: string) -> int {
  scratch: [NE_MAX_OUTPUT] u8;
  v := nostr_event_verify_vartime(text, scratch[..], false);
  if v.error != NE_OK || !v.verified { return -2; }
  j: [1] Json; json_start(j[..], text);
  #assert json_next(j[..]) == JSON_OBJECT;
  return store_event(s, j[..], OWNER);
}

#test "every event verifies and has the id the independent script computed" {
  s: [1] Store; store_clear(s[..]);
  for k: 0..<13 {
    // the example only ever sends event 9 altered; here its id is checked, but it isn't stored
    if k == 9 { #assert equal(EVENTS[k][7..<71], PY_ID[k]); continue; }
    e := check_admit(s[..], EVENTS[k]);
    #assert e != -2;
    #assert equal(EVENTS[k][7..<71], PY_ID[k]);
    if e >= 0 { #assert equal(event_id(s[..], e), PY_ID[k]); }
  }
  // the owner's eleven are kept; the other key's close is dropped
  #assert s[0].n == 11 && s[0].dropped == 1;
}

#test "a changed signature or a changed kind is refused before the store sees it" {
  buf: [4096] u8; scratch: [NE_MAX_OUTPUT] u8;
  forged := EVENTS[9]; sig := forged.count - 3;
  v := nostr_event_verify_vartime(altered(forged, sig, ifx forged[sig] == '0' then '1' else '0', buf[..]), scratch[..], false);
  #assert v.error == NE_SIGNATURE && !v.verified;
  v = nostr_event_verify_vartime(altered(forged, find(forged, "\"kind\":1632") + 10, '3', buf[..]), scratch[..], false);
  #assert v.error == NE_ID && !v.verified;
}

#test "the fold of tea matches the values worked by hand" {
  s: [1] Store; f: [1] Fold; t: [1] Tables; store_clear(s[..]);
  for k: 0..<13 { if k != 9 { e := check_admit(s[..], EVENTS[k]); } }
  fold(s[..], f[..], "tea");
  #assert f[0].project == 0 && f[0].order == 3 && f[0].repo == -1 && f[0].n == 2;
  timer := fold_issue_of(s[..], f[..], PY_ID[1]); scale := fold_issue_of(s[..], f[..], PY_ID[2]);
  #assert timer == 0 && scale == 1 && f[0].alias[timer] == 4 && f[0].alias[scale] == 4;
  // timer: resolved by the 1631 at T0+310; its claim is cleared; done in 4b825dc642; one of three steps done
  #assert f[0].status[timer] == ST_RESOLVED && f[0].status_at[timer] == T0 + 310;
  #assert f[0].claimed[timer].len == 0 && equal(str(s[..], f[0].commit[timer]), "4b825dc642");
  #assert f[0].steps[timer] == 3 && f[0].steps_done[timer] == 1 && f[0].note_cnt[timer] == 1;
  #assert equal(str(s[..], f[0].rank[timer]), "0b") && equal(str(s[..], f[0].rank[scale]), "0a");
  // scale: open since T0+120; the forged and the other key's closes never reached it; the 1111-rooted claim hangs off nothing
  #assert f[0].status[scale] == ST_OPEN && f[0].status_at[scale] == T0 + 120 && f[0].status_ev[scale] == -1;
  #assert f[0].claimed[scale].len == 0 && f[0].note_cnt[scale] == 1 && f[0].under[10] == -1;
  write_tables(s[..], f[..], t[..]);
  #assert t[0].complete;
  #assert equal(cast(string) t[0].items[0..<t[0].items_n],
    "d060|task|open|Add a grams scale||M||||0a|scale||\n8427|task|resolved|Fix the 59-second timer||S||||0b|timer|4b825dc642|1761686710\n");
  // the status line is hidden: the done comment is 10 s before it (under two minutes)
  #assert count_lines(cast(string) t[0].log[0..<t[0].log_n]) == 4;
  // coffee's issue is in the store but not in tea's fold
  #assert fold_issue_of(s[..], f[..], PY_ID[11]) == -1;
  fold(s[..], f[..], "coffee");
  #assert f[0].n == 1 && f[0].project == -1 && fold_issue_of(s[..], f[..], PY_ID[11]) == 0;
}
```

```shell
coax/bin/coax test nostr/examples/checks/projects.coax --steps 4000000000
```

The checks verify BIP-340 signatures in the interpreter, so they need a
larger step budget and take about a minute. On 2026-11-01 `coax test` ran
213 checks, including the loaded files' own, and all passed.

### The library's own checks

`project-checks.coax` holds the fold's checks: the captured transcript
against the TypeScript tables, first-`d` addresses, a project with two `d`
tags refused, statuses by their root `e`, comments by their NIP-22 scope,
refused issues, orders and revisions, and escaped tag values.
`project-live-checks.coax` runs the catch-up checks natively, including "a
capped initial EOSE cannot replace the last display before bounded
backfill" and "a refused subscription never counts as successful catchup".
The
[2026-10-31 verification record](../../docs/verification/2026-10-31-project-fold-typed.md)
reports the fold's 82 checks passing and its tables equal to the model's
over every live project. The
[2026-10-30 new-land record](../../docs/verification/2026-10-30-newland-coax-nostr.md)
reports the homepage reader's checks (292 of 292) and its forged and
changed-content cases.

Those files load `apps/openagents-web/projects/baked.coax`, which the web
build generates and git doesn't track, so they were not rerun for this
chapter. The example reproduces the transcript comparison without it.

For the two renames, on 2026-11-01 `coax test` passed all 39 checks of
`secp_field.coax` and all 125 of `schnorr.coax`. Their mutation runs were
not repeated: renaming a local changes no operation. The project files
gained only comment lines. The full mutation run (`COAX_MUTATION_FULL=1`)
was not run.

## Summary

- A project is a 30621 addressed `30621:<key>:<d>`; its issues point at
  that address, its statuses at an issue by root `e`, and its comments at an
  issue by a NIP-22 `E` scope with `K` 1621.
- The store keeps only the project key's events of those kinds, once; the
  fold takes the newest status per issue and reads comments oldest first
  for the worker, plan, commit and notes.
- An event enters the verified view only after its id and signature check
  out, and only a complete capture replaces the tables already shown.
- The fold of a real relay capture equals the TypeScript model's tables
  byte for byte.

## Dependencies

- [63. Git on Nostr](ch063-git-on-nostr.md)
- [30. Subscriptions and EOSE](ch030-subscriptions-and-eose.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Sources

- [NIP-34](../nips/official/34.md)
- [NIP-22](../nips/official/22.md)
- [NIP-01](../nips/official/01.md)
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
- Record:
  [2026-10-30-newland-coax-nostr.md](../../docs/verification/2026-10-30-newland-coax-nostr.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
