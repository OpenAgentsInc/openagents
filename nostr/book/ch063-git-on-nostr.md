# 63. Git on Nostr

A git repository lives on a git server, but the talk around it can live on
Nostr: who owns the repository, where to clone it, the bug reports, the
patches, and whether each one is still open. NIP-34 defines an event for
each of these. OpenAgents publishes its projects and their work items as
NIP-34 events, so the project pages read them with the code in this
chapter. The chapter reads a repository announcement, an issue, a patch
and status events, and works out which status decides an issue's state.

Part VI, Every NIP. Result: NIP-34 repository announcements (30617), issues
(1621), patches (1617) and statuses (1630 to 1633), read and checked, and
the state of an issue or patch resolved from the newest status by its
author or a maintainer; pull requests (1618) and their updates (1619),
read and checked, with the pull request's tip worked out from its author's
newest update; and NIP-C0 code snippets (1337). Repository state (30618)
and a pull request's status are not built yet; see
[What is not built](#what-is-not-built).

## What this chapter teaches

- Kind 30617: `d`, `name`, `description`, `web`, `clone`, `relays`,
  `maintainers`, and `r` with the `euc` marker for the earliest unique
  commit.
- The repository's address, `30617:<owner pubkey>:<d>`, and how issues,
  patches and statuses point at it with an `a` tag.
- Kind 1621 issues and kind 1617 patches tied to the repository by `a`.
- Statuses 1630 open, 1631 applied, merged or resolved, 1632 closed and
  1633 draft. The newest one from the root's author or a maintainer wins;
  anyone else's is ignored.
- Kind 1618 pull requests: a commit (`c`) and where to fetch it (`clone`)
  in place of a patch's diff. A kind 1619 update moves the tip; only the
  pull request's own author's updates count here.
- Kind 1337 code snippets (NIP-C0): the code is the content, and optional
  tags name its language, file name, licence and repository.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip34.coax](../../coax/lib/nostr/nip34.coax) | `n34_repo(e)` | A repository announcement's `d`, `name`, `description`, `web`, `clone`, `relays` and `euc` |
| [nip34.coax](../../coax/lib/nostr/nip34.coax) | `n34_issue(e)` | An issue's `a` coordinates (the first repository and project), slug and fields |
| [nip34.coax](../../coax/lib/nostr/nip34.coax) | `n34_status(e)` | A status's state and the root it is about |
| [nip34.coax](../../coax/lib/nostr/nip34.coax) | `nt_coord(raw)` | Splits `kind:pubkey:d` into its kind and where `d` starts |
| [nip34-patch.coax](../../coax/lib/nostr/nip34-patch.coax) | `n34_patch(e)` | A patch's repository, `commit`, `parent-commit` and `t root` marks |
| [nip34-patch.coax](../../coax/lib/nostr/nip34-patch.coax) | `n34_maintainer(repo, key)` | Whether a key is in the announcement's `maintainers` tags |
| [nip34-patch.coax](../../coax/lib/nostr/nip34-patch.coax) | `n34_resolve(repo, root, set)` | The state the newest authorized status gives an issue or patch |
| [nip34-event.coax](../../coax/lib/nostr/nip34-event.coax) | `nt_event_of(e)`, `nt_signed_of(e)` | A parsed `NostrEvent` as the readers' `NtEvent`, and as the resolver's `N34Signed` |
| [nip34-pr.coax](../../coax/lib/nostr/nip34-pr.coax) | `n34_pull(e)`, `n34_pull_update(e)` | A pull request's and an update's repository, tip commit, clone URLs and merge base; an update's `E` and `P` |
| [nip34-pr.coax](../../coax/lib/nostr/nip34-pr.coax) | `n34_head(pull, set)` | The pull request's tip after the updates its author signed |
| [nip34-pr.coax](../../coax/lib/nostr/nip34-pr.coax) | `n34_pull_tags`, `n34_pull_update_tags` | The tags of a pull request and of an update |
| [nipc0.coax](../../coax/lib/nostr/nipc0.coax) | `nc0_snippet(e)`, `nc0_tags(t, out)` | A code snippet's tags, read and written |

`nip34.coax` existed before this chapter. OpenAgents' project pages
([project-fold.coax](../../apps/openagents-web/project-fold.coax)) read
every repository, project, issue and status through it. This chapter adds
listing markers to it and no code.

`nip34-patch.coax` is new with this chapter. It reads patches, which
nothing read before, and it decides whose status counts, which the
project pages decide another way (see [The project fold](#the-project-fold)).
Because it decides whose word counts, it is an `#authority` file, so its
checks came first and a mutation run kills every mutant. It loads only
`nip34.coax`, not `event.coax`, so it takes the author, id and time of each
event as plain fields (`N34Signed`) and `nip34-event.coax` fills them from
a parsed event.

`nip34-pr.coax` and `nipc0.coax` were added on 2026-11-12. `nip34-pr.coax`
loads `nip34-patch.coax` and changes nothing in it. It decides whose
update moves a pull request, so it is `#authority` too; `nipc0.coax`
carries the mark as well, and both have mutation runs with no survivors.

Like chapter 52's readers, none of this verifies a signature. The caller
passes events that `nostr_event_verify_vartime` (chapter 21) accepted. A
reader that returns `N34_OK` says only that the tags have the shape below.

## Repository announcements

A repository announcement is an addressable event (kind 30617, so in the
30000 to 39999 range of chapter 22). Its `d` tag names the repository, and
the author's newest announcement with that `d` replaces the older ones.
Only `d` is required. The other tags NIP-34 lists:

| Tag | Values | Meaning |
| --- | --- | --- |
| `d` | one | The repository's identifier, usually a short kebab-case name |
| `name`, `description` | one | For people |
| `web` | one or more | Pages to browse the repository |
| `clone` | one or more | URLs that `git clone` accepts |
| `relays` | one or more | Relays the repository watches for issues and patches |
| `r` with marker `euc` | a commit id | The earliest unique commit: usually the root commit, so forks and mirrors of the same project can be grouped |
| `maintainers` | one or more pubkeys | Other people whose word counts for this repository |

By publishing the announcement, its author says they maintain the
repository. The `maintainers` tag adds others.

`n34_repo` selects the first tag of each name. It refuses with
`N34_MISSING` when there is no `d`, and with `N34_MALFORMED` when a
selected tag has no value or the `euc` commit is not 40 lowercase hex
digits (a SHA-1 commit id).

<!-- source: coax/lib/nostr/nip34.coax repo -->
```jai
// ---- 30617: a NIP-34 repository announcement. d is required; euc is the
// first r tag marked "euc", whose commit is 40 hex digits.
N34Repo :: struct {status:int;d:NtTag;name:NtTag;description:NtTag;web:NtTag;clone:NtTag;relays:NtTag;euc:NtTag;}
n34_repo :: (e:NtEvent)->N34Repo #export_c #effects(all) {
 r:N34Repo;r.status=n34_start(e,N34_KIND_REPO);if r.status!=N34_OK {return r;}
 r.d=nt_tag(e,"d",0);r.name=nt_tag(e,"name",0);r.description=nt_tag(e,"description",0);
 r.web=nt_tag(e,"web",0);r.clone=nt_tag(e,"clone",0);r.relays=nt_tag(e,"relays",0);r.euc=nt_select(e,"r",1,"euc",0);
 s:=nt_shape(r.d,1,true);s=nt_and(s,nt_shape(r.name,1,false));s=nt_and(s,nt_shape(r.description,1,false));
 s=nt_and(s,nt_shape(r.web,1,false));s=nt_and(s,nt_shape(r.clone,1,false));s=nt_and(s,nt_shape(r.relays,1,false));
 if s==N34_OK && r.euc.n>0 && !nt_hex(nt_raw(e,r.euc.v[0]),40) {s=N34_MALFORMED;}
 r.status=s;return r;
}
```

`n34_repo` does not read `maintainers`, because a tag can list any number of
keys and `NtTag` keeps only the first four values. `n34_maintainer` asks
the question the resolver needs instead: is this key one of the values?
`nt_count(e, name, i, marker)` counts the tags whose value at position `i`
is `marker`, so trying every position of every `maintainers` tag finds a
key wherever it is:

<!-- source: coax/lib/nostr/nip34-patch.coax maintainer -->
```jai
// Whether key is a value of a maintainers tag of the repository (any tag,
// any position), as 64 lowercase hex digits. The announcement's author is a
// maintainer too; n34_resolve checks that separately.
n34_maintainer :: (repo:NtEvent,key:[32]u8)->bool #effects(all) {
 hex:=n34_hex32(key);
 for k:0..<nt_tags(repo,"maintainers") {
  for i:0..<nt_tag(repo,"maintainers",k).values {if nt_count(repo,"maintainers",i,cast(string)hex[..])>0 {return true;}}
 }
 return false;
}
```

### Worked example: Alice's tea-tools

Alice announces a repository with these tags (keys shortened):

```text
["d", "tea-tools"]
["name", "Tea tools"]
["description", "Timers and scales for brewing tea"]
["web", "https://git.example.com/tea-tools"]
["clone", "https://git.example.com/tea-tools.git", "ssh://git@git.example.com/tea-tools.git"]
["relays", "wss://relay.example.com"]
["r", "4b825dc642cb6eb9a060e54bf8d69288fbee4904", "euc"]
["maintainers", "dff1d77f...7b502ba659"]
```

1. `d` is `tea-tools`. Every other tag is optional, so the announcement
   reads.
2. `clone` has two values. `n34_repo` keeps both spans and says
   `values` is 2.
3. The `r` tag has the marker `euc` at value position 1, and its commit is
   40 lowercase hex digits, so it is the earliest unique commit.
4. The maintainers are Alice, the author, and Bob, whose key
   `dff1d77f...7b502ba659` is the `maintainers` value.

The repository's address is its kind, its author's key and its `d`,
joined by colons (NIP-01's `a` tag form, chapter 22):

```text
30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools
```

`nt_coord` reads it from the left: the digits before the first colon are
the kind (30617, at most 65535). Then come exactly 64 lowercase hex digits
and a colon, so the key is bytes 6 to 69, and `d` starts at byte 71. An
uppercase key, a short key, or a kind that isn't a number make the kind
-1, which every reader refuses.

## Issues and patches

An issue (kind 1621) is a Markdown report about a repository: a bug, a
request or a question. A patch (kind 1617) holds the output of
`git format-patch`. Both name the repository with an `a` tag holding its
address, and should carry a `p` tag for its owner.

`n34_issue` requires at least one `a` tag and refuses any `a` that isn't a
coordinate. It keeps the first coordinate of kind 30617 as the repository
and the first of kind 30621 as the OpenAgents project (chapter 98). It also
reads the fields OpenAgents writes on issues (`subject`, `t` labels,
`size`, `reward`, `parent`, `blocked-by` and `needs`) through `n34_fields`.

<!-- source: coax/lib/nostr/nip34.coax issue -->
```jai
// ---- 1621: a NIP-34 issue. At least one a tag is required and every a is
// a coordinate; repo and project are the first of kinds 30617 and 30621
// (to>from when present). slug is OpenAgents' item slug.
N34Issue :: struct {status:int;fields:N34Fields;slug:NtTag;a:int;repo:NtSpan;project:NtSpan;}
n34_issue :: (e:NtEvent)->N34Issue #export_c #effects(all) {
 r:N34Issue;r.status=n34_start(e,N34_KIND_ISSUE);if r.status!=N34_OK {return r;}
 r.fields=n34_fields(e);r.slug=nt_tag(e,"slug",0);r.a=nt_tags(e,"a");
 s:=ifx r.a==0 then N34_MISSING else nt_and(nt_shape(r.slug,1,false),r.fields.status);
 for k:0..<r.a {
  // A valueless a tag's first span is empty, which is no coordinate.
  t:=nt_tag(e,"a",k);c:=nt_coord(nt_raw(e,t.v[0]));
  if c.kind<0 {s=nt_and(s,N34_MALFORMED);}
  if c.kind==N34_KIND_REPO && r.repo.to==0 {r.repo=t.v[0];}
  if c.kind==N34_KIND_PROJECT && r.project.to==0 {r.project=t.v[0];}
 }
 r.status=s;return r;
}
```

`n34_patch` reads a patch the same way. NIP-34 says a patch *should*
carry the `a` tag; `n34_patch` requires one that names a repository,
because a patch that names none can't be resolved against any
maintainers. `commit` and `parent-commit`, which let a maintainer apply the
patch with the commit id the proposer had, must be 40 hex digits when
present. `t root` marks the first patch of a series and `t root-revision`
the first patch of a revised series.

<!-- source: coax/lib/nostr/nip34-patch.coax patch -->
```jai
// ---- 1617: a NIP-34 patch. At least one a tag is required, every a is a
// coordinate, and repo is the first of kind 30617 (required). commit and
// parent-commit, when present, are 40 hex digits. root says a t tag is
// "root" (the first patch of a series), revision that one is
// "root-revision".
N34Patch :: struct {status:int;repo:NtSpan;root:bool;revision:bool;commit:NtTag;parent:NtTag;}
n34_patch :: (e:NtEvent)->N34Patch #effects(all) {
 r:N34Patch;r.status=n34_start(e,N34_KIND_PATCH);if r.status!=N34_OK {return r;}
 a:=nt_tags(e,"a");s:=ifx a==0 then N34_MISSING else N34_OK;
 for k:0..<a {
  t:=nt_tag(e,"a",k);c:=nt_coord(nt_raw(e,t.v[0]));
  if c.kind<0 {s=nt_and(s,N34_MALFORMED);}
  if c.kind==N34_KIND_REPO && r.repo.to==0 {r.repo=t.v[0];}
 }
 if s==N34_OK && r.repo.to==0 {s=N34_MISSING;}
 // A valueless commit's first span is empty, which is no commit.
 r.commit=nt_tag(e,"commit",0);r.parent=nt_tag(e,"parent-commit",0);
 if s==N34_OK && r.commit.n>0 && !nt_hex(nt_raw(e,r.commit.v[0]),40) {s=N34_MALFORMED;}
 if s==N34_OK && r.parent.n>0 && !nt_hex(nt_raw(e,r.parent.v[0]),40) {s=N34_MALFORMED;}
 r.root=nt_count(e,"t",0,"root")>0;r.revision=nt_count(e,"t",0,"root-revision")>0;
 r.status=s;return r;
}
```

Carol's issue about tea-tools carries:

```text
["a", "30617:f9308a01...bce036f9:tea-tools"]
["p", "f9308a01...bce036f9"]
["subject", "The timer stops at 59 seconds"]
["t", "bug"]
```

Its one `a` tag is a coordinate of kind 30617, so `repo` is that span.
Its id, which statuses will name, is `f56388ff...2491b47e`.

## Statuses

An issue or a root patch is open until someone says otherwise. Four kinds
say otherwise:

| Kind | Issue | Patch |
| --- | --- | --- |
| 1630 | open | open |
| 1631 | resolved | applied or merged |
| 1632 | closed | closed |
| 1633 | draft | draft |

A status names its root with `["e", <root id>, "", "root"]`. When a
maintainer applies a revision, a second `e` tag with the marker `reply`
names it. `n34_status` takes the `e` tag marked `root`, else the first
`e`, and refuses one whose id isn't 64 lowercase hex digits. The state is
the kind less 1630.

<!-- source: coax/lib/nostr/nip34.coax status -->
```jai
// ---- 1630-1633: a NIP-34 status. root is the first e tag marked "root",
// else the first e tag; it is required and is an event id.
N34Status :: struct {status:int;state:int;root:NtTag;}
n34_status :: (e:NtEvent)->N34Status #export_c #effects(all) {
 r:N34Status;r.status=n34_valid(e);
 if r.status==N34_OK && (e.kind<N34_KIND_OPEN || e.kind>N34_KIND_DRAFT) {r.status=N34_KIND;}if r.status!=N34_OK {return r;}
 r.state=e.kind-N34_KIND_OPEN;r.root=nt_select(e,"e",2,"root",0);
 if r.root.n==0 {r.root=nt_tag(e,"e",0);}
 s:=nt_shape(r.root,1,true);
 if s==N34_OK && !nt_hex(nt_raw(e,r.root.v[0]),64) {s=N34_MALFORMED;}
 r.status=s;return r;
}
```

### Whose status counts

Anyone can publish a status about any issue. If every status counted, a
stranger could close every issue in a repository. NIP-34 says: "The most
recent Status event (by `created_at` date) from either the issue/patch
author or a maintainer is considered valid." So the resolver keeps three
groups apart:

- statuses about this root by its author, the repository's author or a
  key in `maintainers`, which are *counted*;
- statuses about this root by anyone else, which are *ignored*;
- events of a status kind that don't read as a status, which are
  *refused*.

Statuses about another root are left out of all three. Of the counted
ones, the newest by `created_at` wins. NIP-34 doesn't say what happens on a
tie; `n34_resolve` gives it to the lower id, NIP-01's rule for replaceable
events, as chapter 52's reaction tally does. With none counted, the root is
open.

Before counting, the resolver checks that the root belongs to this
repository: the root's first 30617 coordinate must be the repository's
address, byte for byte. Otherwise a maintainer of Bob's `tea-tools` could
decide issues filed against Alice's. A root that names another repository
refuses with `N34_ELSEWHERE`.

<!-- source: coax/lib/nostr/nip34-patch.coax record -->
```jai
// An event as the resolver reads it: its tags, and the author, id and time
// its signature covers.
N34Signed :: struct {e:NtEvent;public_key:[32]u8;id:[32]u8;created_at:int;}
N34State :: struct {
 status:int=N34_EVENT;     // N34_OK, or why the repository or root doesn't read or match
 state:int=N34_STATE_OPEN; // the winning status's, or open when none counts
 winner:int=-1;            // the winning status's index in the set, or -1
 counted:int;              // statuses for the root by its author or a maintainer
 ignored:int;              // statuses for the root by anyone else
 refused:int;              // events of a status kind that don't read
}
```

<!-- source: coax/lib/nostr/nip34-patch.coax resolve -->
```jai
// The repository's address, "30617:<its author>:<its d>", is the root's
// first 30617 coordinate, byte for byte.
n34_names :: (repo:N34Signed,d:string,a:string)->bool #effects(all) {
 c:=nt_coord(a);
 if c.kind!=N34_KIND_REPO || a[c.d..<a.count]!=d {return false;}
 hex:=n34_hex32(repo.public_key);
 return a[c.d-65..<c.d-1]==cast(string)hex[..];
}
n34_same :: (a:[32]u8,b:[32]u8)->bool #effects(all) {for i:0..<32 {if a[i]!=b[i] {return false;}}return true;}
// a is newer than b: a later created_at, or the same and a lower id.
n34_newer :: (a:N34Signed,b:N34Signed)->bool #effects(all) {
 if a.created_at>b.created_at {return true;}
 if a.created_at<b.created_at {return false;}
 for i:0..<32 {
  if a.id[i]<b.id[i] {return true;}
  if a.id[i]>b.id[i] {return false;}
 }
 return false;
}
n34_resolve :: (repo:N34Signed,root:N34Signed,set:[]N34Signed)->N34State #effects(all) {
 out:N34State;
 re:=repo.e;rr:=n34_repo(re);
 if rr.status!=N34_OK {out.status=rr.status;return out;}
 te:=root.e;at:NtSpan;
 if te.kind==N34_KIND_PATCH {
  p:=n34_patch(te);if p.status!=N34_OK {out.status=p.status;return out;}
  at=p.repo;
 } else {
  i:=n34_issue(te);if i.status!=N34_OK {out.status=i.status;return out;}
  at=i.repo;
 }
 if !n34_names(repo,nt_raw(re,rr.d.v[0]),nt_raw(te,at)) {out.status=N34_ELSEWHERE;return out;}
 out.status=N34_OK;
 for k:0..<set.count {
  ev:=set[k];
  if ev.e.kind<N34_KIND_OPEN || ev.e.kind>N34_KIND_DRAFT {continue;}
  se:=ev.e;st:=n34_status(se);
  if st.status!=N34_OK {out.refused+=1;continue;}
  id:=n34_hex32(root.id);
  if nt_raw(se,st.root.v[0])!=cast(string)id[..] {continue;}
  if !n34_same(ev.public_key,root.public_key) && !n34_same(ev.public_key,repo.public_key) && !n34_maintainer(re,ev.public_key) {
   out.ignored+=1;continue;
  }
  out.counted+=1;
  if out.winner<0 || n34_newer(ev,set[out.winner]) {out.winner=k;out.state=st.state;}
 }
 return out;
}
```

### Worked example: is Carol's issue resolved?

Four statuses arrive, each naming its root with an `e` tag marked `root`:

| Status | Kind | By | At | Root | Who they are |
| --- | --- | --- | --- | --- | --- |
| S1 | 1630 open | Carol | T0+100 | the issue | the issue's author |
| S2 | 1631 resolved | Bob | T0+200 | the issue | a maintainer |
| S3 | 1632 closed | Dave | T0+300 | the issue | neither |
| S4 | 1631 applied | Alice | T0+400 | the patch | the repository's author |

For the issue:

1. The issue's first 30617 coordinate is
   `30617:f9308a01...bce036f9:tea-tools`. The repository's kind is 30617,
   its author is `f9308a01...bce036f9` and its `d` is `tea-tools`, so the
   issue belongs to it.
2. S4's root is the patch, not the issue: left out.
3. S1 is Carol's, and Carol wrote the issue: counted.
4. S2 is Bob's, and Bob is in `maintainers`: counted.
5. S3 is Dave's. Dave wrote neither the issue nor the announcement, and
   isn't in `maintainers`: ignored, though it is the newest.
6. Of S1 (T0+100) and S2 (T0+200), S2 is newer. The issue is resolved:
   counted 2, ignored 1, refused 0.

For the patch, only S4 is about it, and Alice owns the repository: the
patch is applied, counted 1.

Without S2, S1 is the only counted status, so the issue is open again,
even though Dave's later status says closed.

## The project fold

OpenAgents' project pages fold a project's events into tables (chapter
98): one row per issue, with its status. That fold
([project-fold.coax](../../apps/openagents-web/project-fold.coax)) reads
every status with `n34_status`, hangs it off the issue its root names, and
takes the newest. It does not call `n34_resolve`. Instead its store keeps
only events signed by the project's key, so every status it sees is the
project owner's, and the newest one wins: the same answer `n34_resolve`
gives when the owner is the only maintainer. A status by anyone else never
reaches the fold.

The fold's own check, "the fold of a captured relay transcript equals
model.ts fold() and tables.ts, byte for byte", in
[project-checks.coax](../../apps/openagents-web/project-checks.coax),
compares its tables for a captured relay transcript with those of the
TypeScript model. The verification record
[2026-10-31-project-fold-typed.md](../../docs/verification/2026-10-31-project-fold-typed.md)
reports it passing (82 of 82 checks), and the fold's tables identical to
the model's over every live project (2697 events for the Coax project) on
2026-10-31. It was not rerun for this chapter: it loads
`apps/openagents-web/projects/baked.coax`, which the web build generates
and git doesn't track, and which this worktree doesn't have.

## Pull requests

A patch carries its change in its content. That stops working when the
change is large: NIP-34 says "Patches SHOULD be used if each event is
under 60kb, otherwise PRs SHOULD be used." A pull request (kind 1618)
carries no diff. It names a commit and says where to fetch it:

```text
["a", "30617:<base-repo-owner-pubkey>:<base-repo-id>"]
["p", "<repository-owner>"]
["subject", "<PR-subject>"]
["c", "<current-commit-id>"]
["clone", "<clone-url>", ...]
["branch-name", "<branch-name>"]
```

`c` is the tip of the branch the author wants merged, and `clone` is "at
least one git clone url where commit can be downloaded". The content is
Markdown that describes the change. Optional tags add labels (`t`), the
newest commit shared with the target branch (`merge-base`) and, when the
pull request replaces a patch, that root patch's id (`e`).

A pull request and an update share two readings: which repository they
are for, and the tip.

<!-- source: coax/lib/nostr/nip34-pr.coax target -->
```jai
// The repository an event is for: the first a tag of kind 30617. Every a
// tag must be a coordinate.
n34_target :: (e:NtEvent,repo:[]NtSpan)->int #effects(all) {
 s:=N34_OK;
 for k:0..<nt_tags(e,"a") {
  // A valueless a tag's first span is empty, which is no coordinate.
  t:=nt_tag(e,"a",k);c:=nt_coord(nt_raw(e,t.v[0]));
  if c.kind<0 {s=N34_MALFORMED;}
  if c.kind==N34_KIND_REPO && repo[0].to==0 {repo[0]=t.v[0];}
 }
 return nt_and(s,ifx repo[0].to==0 then N34_MISSING else N34_OK);
}
// The tip both kinds carry: c and clone are required.
n34_tip :: (e:NtEvent)->N34Tip #effects(all) {
 r:N34Tip;r.commit=nt_tag(e,"c",0);r.clone=nt_tag(e,"clone",0);r.base=nt_tag(e,"merge-base",0);
 s:=nt_and(nt_shape(r.commit,1,true),nt_shape(r.clone,1,true));
 if s==N34_OK && !nt_hex(nt_raw(e,r.commit.v[0]),40) {s=N34_MALFORMED;}
 // A valueless merge-base's first span is empty, which is no commit.
 if s==N34_OK && r.base.n>0 && !nt_hex(nt_raw(e,r.base.v[0]),40) {s=N34_MALFORMED;}
 r.status=s;return r;
}
```

<!-- source: coax/lib/nostr/nip34-pr.coax pull -->
```jai
// ---- 1618: a pull request.
n34_pull :: (e:NtEvent)->N34Pull #effects(all) {
 r:N34Pull;r.status=n34_start(e,N34_KIND_PULL);if r.status!=N34_OK {return r;}
 repo:[1]NtSpan;s:=n34_target(e,repo[..]);r.repo=repo[0];r.tip=n34_tip(e);
 r.subject=nt_tag(e,"subject",0);r.branch=nt_tag(e,"branch-name",0);r.revises=nt_tag(e,"e",0);r.labels=nt_each(e,"t",0);
 s=nt_and(s,r.tip.status);s=nt_and(s,nt_shape(r.subject,1,false));s=nt_and(s,nt_shape(r.branch,1,false));
 if r.labels<0 {s=nt_and(s,N34_MALFORMED);}
 if r.revises.n>0 && !nt_hex(nt_raw(e,r.revises.v[0]),64) {s=nt_and(s,N34_MALFORMED);}
 r.status=s;return r;
}
// ---- 1619: a pull request update. E and P are required.
n34_pull_update :: (e:NtEvent)->N34PullUpdate #effects(all) {
 r:N34PullUpdate;r.status=n34_start(e,N34_KIND_PULL_UPDATE);if r.status!=N34_OK {return r;}
 repo:[1]NtSpan;s:=n34_target(e,repo[..]);r.repo=repo[0];r.tip=n34_tip(e);
 r.pull=nt_tag(e,"E",0);r.author=nt_tag(e,"P",0);
 s=nt_and(s,r.tip.status);s=nt_and(s,nt_shape(r.pull,1,true));s=nt_and(s,nt_shape(r.author,1,true));
 if s==N34_OK && (!nt_hex(nt_raw(e,r.pull.v[0]),64) || !nt_hex(nt_raw(e,r.author.v[0]),64)) {s=N34_MALFORMED;}
 r.status=s;return r;
}
```

NIP-34 says a pull request "SHOULD include an `a` tag". `n34_pull`
requires it, as `n34_patch` does, because the next step works out the tip
for one repository.

### Updates

When the author pushes more commits, the first event's `c` is out of
date. A signed event can't be edited, so the author signs a second one. A
pull request update (kind 1619) "changes the tip of a referenced PR
event". It names the pull request and its author with NIP-22's uppercase
tags, `["E", "<pull-request-event-id>"]` and
`["P", "<pull-request-author>"]`, and carries the new `c` and `clone`.

NIP-34 does not say whose update counts. If anyone's did, Dave could sign
a kind 1619 that moves Carol's pull request to a commit of his own, and a
maintainer who merges "the tip" would merge Dave's code under Carol's
name. So `n34_head` counts an update only when both of these hold:

- the pull request's own author signed it;
- it names the same repository as the pull request.

Of the updates that count, the newest decides. A tie in `created_at` goes
to the lower id, as it does in `n34_resolve`. With none, the tip is the
pull request's own `c`.

<!-- source: coax/lib/nostr/nip34-pr.coax head -->
```jai
// The pull request's tip after the updates in set.
n34_head :: (pull:N34Signed,set:[]N34Signed)->N34Head #effects(all) {
 out:N34Head;pe:=pull.e;p:=n34_pull(pe);
 if p.status!=N34_OK {out.status=p.status;return out;}
 out.status=N34_OK;id:=n34_hex32(pull.id);tip:=nt_raw(pe,p.tip.commit.v[0]);
 for k:0..<set.count {
  ev:=set[k];
  if ev.e.kind!=N34_KIND_PULL_UPDATE {continue;}
  ue:=ev.e;u:=n34_pull_update(ue);
  if u.status!=N34_OK {out.refused+=1;continue;}
  if nt_raw(ue,u.pull.v[0])!=cast(string)id[..] {continue;}
  if !n34_same(ev.public_key,pull.public_key) || nt_raw(ue,u.repo)!=nt_raw(pe,p.repo) {out.ignored+=1;continue;}
  out.counted+=1;
  if out.winner<0 || n34_newer(ev,set[out.winner]) {out.winner=k;tip=nt_raw(ue,u.tip.commit.v[0]);}
 }
 for i:0..<40 {out.commit[i]=tip[i];}
 return out;
}
```

`n34_head` trusts each event's author, id and time, like `n34_resolve`:
the caller passes verified events. It fetches no commit, so it does not
know whether the new tip descends from the old one.

### Worked example: Carol's pull request

Carol opens a pull request against Alice's `tea-tools`, then fixes a
review comment. Dave tries to move it.

| Event | Signer | Kind | At | `c` | Names |
| --- | --- | --- | --- | --- | --- |
| PR1 | Carol | 1618 | T0+100 | `9c4f6e2b...` | `tea-tools` |
| U1 | Carol | 1619 | T0+200 | `2b1d0c9f...` | PR1, `tea-tools` |
| U2 | Dave | 1619 | T0+300 | `7e6d5c4b...` | PR1, `tea-tools` |

1. PR1 alone: no update, so the tip is PR1's own `c`, `9c4f6e2b...`.
2. U1 names PR1 in `E`, Carol signed it, and its `a` is the same address:
   it counts. The tip is `2b1d0c9f...`.
3. U2 names PR1 and is newer than U1, but Dave did not sign PR1: ignored.
   The tip stays `2b1d0c9f...`, with one update counted and one ignored.
4. U2 alone: nothing counts, and the tip is PR1's own `c` again.

## Code snippets (NIP-C0)

NIP-C0 is a kind for a piece of code on its own, with no repository
needed: kind 1337. "The `.content` field contains the actual code snippet
text." Every tag is optional:

| Tag | Holds |
| --- | --- |
| `l` | The language's name, in lowercase: `rust`, `python` |
| `name` | A name for the snippet, usually a file name |
| `extension` | The file extension, without the dot |
| `description` | What the code does |
| `runtime` | What runs it: `node v18.15.0` |
| `license` | An SPDX identifier such as `MIT`. It can be repeated |
| `dep` | A dependency. It can be repeated |
| `repo` | Where the code comes from: a URL, or a NIP-34 repository's address |

The `repo` tag is the link to the rest of this chapter. NIP-C0: "This MUST
be either a standard URL or, alternatively, the address of a NIP-34 Git
repository announcement event", `30617:<pubkey>:<d>`.

<!-- source: coax/lib/nostr/nipc0.coax snippet -->
```jai
// A kind 1337 code snippet.
nc0_snippet :: (e:NtEvent)->NC0Snippet #effects(all) {
 r:NC0Snippet;r.status=n34_start(e,NC0_KIND_SNIPPET);if r.status!=N34_OK {return r;}
 r.language=nt_tag(e,"l",0);r.name=nt_tag(e,"name",0);r.extension=nt_tag(e,"extension",0);
 r.description=nt_tag(e,"description",0);r.runtime=nt_tag(e,"runtime",0);r.repo=nt_tag(e,"repo",0);
 r.licenses=nt_each(e,"license",0);r.deps=nt_each(e,"dep",0);
 s:=nt_and(nt_shape(r.language,1,false),nt_shape(r.name,1,false));s=nt_and(s,nt_shape(r.extension,1,false));
 s=nt_and(s,nt_shape(r.description,1,false));s=nt_and(s,nt_shape(r.runtime,1,false));
 if r.licenses<0 || r.deps<0 {s=nt_and(s,N34_MALFORMED);}
 for c:nt_raw(e,r.language.v[0]) {if c>='A' && c<='Z' {s=nt_and(s,N34_MALFORMED);}}
 extension:=nt_raw(e,r.extension.v[0]);
 if extension.count>0 && extension[0]=='.' {s=nt_and(s,N34_MALFORMED);}
 // A valueless repo's first span is empty, which is neither.
 if s==N34_OK && r.repo.n>0 {
  repo:=nt_raw(e,r.repo.v[0]);
  if nt_coord(repo).kind==N34_KIND_REPO {r.repo_is=NC0_ADDRESS;}
  else if nc0_url(repo) {r.repo_is=NC0_URL;}
  else {s=N34_MALFORMED;}
 }
 r.status=s;return r;
}
```

Since no tag is required, an event with none reads. What refuses is a tag
that is present and breaks NIP-C0's own words: an `l` with an uppercase
letter, an `extension` that starts with a dot, a `repo` that is neither a
URL nor a 30617 address, and any of these tags without a value.
`repo_is` says which form the `repo` tag has, so a client knows whether
to open a link or to look up an announcement with `n34_repo`.

`nc0_url` is a shape check, not a URL parser: a scheme, `://` and
something after it.

<!-- source: coax/lib/nostr/nipc0.coax url -->
```jai
// Whether raw is a URL as this file reads one: a scheme (a letter, then
// letters, digits, '+', '-' or '.'), "://" and at least one more byte.
nc0_url :: (raw:string)->bool #effects(all) {
 i:=0;
 while i<raw.count {
  c:=raw[i];letter:=(c>='a' && c<='z') || (c>='A' && c<='Z');
  if !(letter || (i>0 && ((c>='0' && c<='9') || c=='+' || c=='-' || c=='.'))) {break;}
  i+=1;
 }
 return i>0 && raw.count>i+3 && raw[i..<i+3]=="://";
}
```

The reader does not check that a licence is on the SPDX list, and it never
runs the code.

## What is not built

- **Repository state (30618).** The `refs/heads/*`, `refs/tags/*` and
  `HEAD` tags have no reader.
- **A pull request's status.** Kinds 1630 to 1633 apply to pull requests
  too, and a status about one reads with `n34_status`. But `n34_resolve`
  takes only an issue or a patch as its root, so no code works out
  whether a pull request is open, merged or closed.
- **More about pull requests.** Fetching the commit, checking that an
  update's tip descends from the old one, updates by a maintainer who
  pushed to the author's branch, and closing the root patch that a pull
  request's `e` tag replaces.
- **Patch revisions.** NIP-34 says a revision's status is its root patch's,
  or closed when the root is applied and the revision isn't named in that
  1631. `n34_resolve` resolves only the root.
- **Forks and grouping.** The `u` tag (a subordinate fork) and grouping
  announcements by `euc` are not read.
- **For NIP-C0 snippets:** checking a `license` against the SPDX list, and
  everything NIP-C0 asks of a client's screen (highlighting, copying,
  running).
- The user grasp list (10317) and `nostr://` clone URLs.

## The example

[nostr/examples/repositories.coax](../examples/repositories.coax) signs
Alice's announcement (with Bob as maintainer), Carol's issue and patch,
and the four statuses above, with the BIP-340 test vector keys 0 to 3
(Alice, Bob, Carol, Dave). It reads each one back, resolves the issue and
the patch, resolves the issue again without S2 and against Bob's own
`tea-tools`, and then shows the refusals: an `euc` of 39 digits, a `clone`
with no URL, an announcement without `d`, an issue without `a`, an issue
whose `a` has an uppercase key, a patch whose commit has 8 digits, a patch
that names only a project, and statuses without `e` or with a short id.

<!-- source: nostr/examples/repositories.coax example -->
```jai
#load "../../coax/lib/nostr/nip34-event.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

STATUS :: [7] string.["ok", "event", "kind", "missing", "malformed", "output", "elsewhere"];
STATE :: [4] string.["open (1630)", "applied, merged or resolved (1631)", "closed (1632)", "draft (1633)"];
// BIP-340 test vectors 0, 1, 2 and 3's published secret keys. Never sign
// with a real key in an example.
KEYS :: [4] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9",
  "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710"];
PEOPLE :: [4] string.["Alice", "Bob", "Carol", "Dave"];
ALICE :: 0; BOB :: 1; CAROL :: 2; DAVE :: 3;
EUC :: "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
COMMIT :: "9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6";
PARENT :: "5d7f1e0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e";
T0 :: 1761686400;

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}

// Sign an event into out and read it back with its ID and signature checked.
sign :: (key: [] u8, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  aux: [32] u8; work: [1] NostrSignWork; scratch: [8192] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key, aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [8192] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}

// A tag list is built from pieces: raw JSON text and 64-digit hex keys/ids.
Tags :: struct { b: [4096] u8; n: int; }
put :: (t: [] Tags, s: string) { t[0].n = json_raw(t[0].b[..], t[0].n, s); #assert t[0].n > 0; }
put_key :: (t: [] Tags, id: [32] u8) { h := n34_hex32(id); put(t, cast(string) h[..]); }
tags_of :: (t: [] Tags) -> string { return cast(string) t[0].b[0..<t[0].n]; }
// "30617:<owner>:<d>", the repository's address.
put_address :: (t: [] Tags, owner: [32] u8, d: string) { put(t, "30617:"); put_key(t, owner); put(t, ":"); put(t, d); }

// The tags of each event, as NIP-34 lays them out.
repo_tags :: (t: [] Tags, maintainer: [32] u8) {
  put(t, "[[\"d\",\"tea-tools\"],[\"name\",\"Tea tools\"],[\"description\",\"Timers and scales for brewing tea\"],");
  put(t, "[\"web\",\"https://git.example.com/tea-tools\"],[\"clone\",\"https://git.example.com/tea-tools.git\",\"ssh://git@git.example.com/tea-tools.git\"],");
  put(t, "[\"relays\",\"wss://relay.example.com\"],[\"r\",\""); put(t, EUC); put(t, "\",\"euc\"],[\"maintainers\",\""); put_key(t, maintainer); put(t, "\"]]");
}
issue_tags :: (t: [] Tags, owner: [32] u8) {
  put(t, "[[\"a\",\""); put_address(t, owner, "tea-tools"); put(t, "\"],[\"p\",\""); put_key(t, owner);
  put(t, "\"],[\"subject\",\"The timer stops at 59 seconds\"],[\"t\",\"bug\"]]");
}
patch_tags :: (t: [] Tags, owner: [32] u8) {
  put(t, "[[\"a\",\""); put_address(t, owner, "tea-tools"); put(t, "\"],[\"r\",\""); put(t, EUC); put(t, "\"],[\"p\",\""); put_key(t, owner);
  put(t, "\"],[\"t\",\"root\"],[\"commit\",\""); put(t, COMMIT); put(t, "\"],[\"parent-commit\",\""); put(t, PARENT); put(t, "\"]]");
}
status_tags :: (t: [] Tags, root: NostrEvent, owner: [32] u8) {
  put(t, "[[\"e\",\""); put_key(t, root.id); put(t, "\",\"\",\"root\"],[\"p\",\""); put_key(t, owner);
  put(t, "\"],[\"p\",\""); put_key(t, root.public_key); put(t, "\"],[\"a\",\""); put_address(t, owner, "tea-tools"); put(t, "\"]]");
}

// An unsigned event with these tags, parsed but not verified: its id and
// sig are zeros. Used for the refusals, which the readers refuse on their
// tags alone.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

who :: (keys: [] [32] u8, key: [32] u8) -> string {
  for k: 0..<keys.count { if n34_same(keys[k], key) { return PEOPLE[k]; } }
  return "someone else";
}
show_resolve :: (label: string, names: [] string, s: N34State) {
  print("%: %", label, STATUS[s.status]);
  if s.status != N34_OK { print("\n"); return; }
  print(", state %, winner %, counted %, ignored %, refused %\n", STATE[s.state], ifx s.winner >= 0 then names[s.winner] else "none", s.counted, s.ignored, s.refused);
}

main :: () {
  secret: [4][32] u8; keys: [4][32] u8;
  for k: 0..<4 { decode_hex(KEYS[k], secret[k][..]); }
  buf: [8][8192] u8;

  print("the repository announcement, by Alice\n");
  t: [1] Tags;
  // Alice's public key comes from signing: sign an empty note first.
  for k: 0..<4 { keys[k] = sign(secret[k][..], T0, 1, "[]", "", buf[0][..]).public_key; }
  for k: 0..<4 { h := n34_hex32(keys[k]); print("%: %\n", PEOPLE[k], cast(string) h[..]); }
  repo_tags(t[..], keys[BOB]);
  repo := sign(secret[ALICE][..], T0, 30617, tags_of(t[..]), "", buf[0][..]);
  re := nt_event_of(repo); r := n34_repo(re);
  print("read: %, d %, name \"%\", clone % values, first %, euc %\n", STATUS[r.status], nt_raw(re, r.d.v[0]), nt_raw(re, r.name.v[0]), r.clone.values, nt_raw(re, r.clone.v[0]), nt_raw(re, r.euc.v[0]));
  print("maintainers: Alice (the author) and");
  for k: 1..<4 { if n34_maintainer(re, keys[k]) { print(" %", PEOPLE[k]); } }
  print("\n");
  a: [1] Tags; put_address(a[..], keys[ALICE], "tea-tools");
  print("its address: %\n", tags_of(a[..]));

  print("an issue and a patch, by Carol\n");
  t[0].n = 0; issue_tags(t[..], keys[ALICE]);
  print("issue tags: %\n", tags_of(t[..]));
  issue := sign(secret[CAROL][..], T0 + 60, 1621, tags_of(t[..]), "Set the timer to 1:30 and it stops at 0:59.", buf[1][..]);
  ie := nt_event_of(issue); i := n34_issue(ie);
  print("issue: %, repository %, subject \"%\"\n", STATUS[i.status], ifx nt_raw(ie, i.repo) == tags_of(a[..]) then "tea-tools (the address above)" else "another", nt_raw(ie, i.fields.subject.v[0]));
  t[0].n = 0; patch_tags(t[..], keys[ALICE]);
  patch := sign(secret[CAROL][..], T0 + 90, 1617, tags_of(t[..]), "From 9c4f6e2b Mon Sep 17 00:00:00 2001\nSubject: [PATCH] timer: count minutes past 59 seconds\n", buf[2][..]);
  pe := nt_event_of(patch); p := n34_patch(pe);
  print("patch: %, root %, revision %, commit %, parent %\n", STATUS[p.status], p.root, p.revision, nt_raw(pe, p.commit.v[0]), nt_raw(pe, p.parent.v[0]));

  print("statuses\n");
  set: [4] NostrEvent; names := [4] string.["S1", "S2", "S3", "S4"];
  t[0].n = 0; status_tags(t[..], issue, keys[ALICE]);
  print("S1..S3 tags: %\n", tags_of(t[..]));
  set[0] = sign(secret[CAROL][..], T0 + 100, 1630, tags_of(t[..]), "Still happens on 2.1.", buf[3][..]);
  set[1] = sign(secret[BOB][..], T0 + 200, 1631, tags_of(t[..]), "Fixed by the patch.", buf[4][..]);
  set[2] = sign(secret[DAVE][..], T0 + 300, 1632, tags_of(t[..]), "Not a bug.", buf[5][..]);
  t[0].n = 0; status_tags(t[..], patch, keys[ALICE]);
  set[3] = sign(secret[ALICE][..], T0 + 400, 1631, tags_of(t[..]), "Applied.", buf[6][..]);
  for k: 0..<4 {
    st := n34_status(nt_event_of(set[k]));
    print("%: kind % by % at T0+%, state %\n", names[k], set[k].kind, who(keys[..], set[k].public_key), set[k].created_at - T0, STATE[st.state]);
  }
  print("each event's id\n");
  all := [7] NostrEvent.[repo, issue, patch, set[0], set[1], set[2], set[3]];
  all_names := [7] string.["repository", "issue", "patch", "S1", "S2", "S3", "S4"];
  for k: 0..<7 { h := n34_hex32(all[k].id); print("%: %, verified %\n", all_names[k], cast(string) h[..], all[k].verified); }
  // The resolver reads each event's tags, author, id and time.
  signed: [4] N34Signed; for k: 0..<4 { signed[k] = nt_signed_of(set[k]); }
  show_resolve("the issue", names[..], n34_resolve(nt_signed_of(repo), nt_signed_of(issue), signed[..]));
  show_resolve("the patch", names[..], n34_resolve(nt_signed_of(repo), nt_signed_of(patch), signed[..]));
  // Without Bob's status, Carol's own "open" is the newest that counts.
  less := [3] N34Signed.[signed[0], signed[2], signed[3]]; less_names := [3] string.["S1", "S3", "S4"];
  show_resolve("the issue without S2", less_names[..], n34_resolve(nt_signed_of(repo), nt_signed_of(issue), less[..]));
  // Bob signs the same repository name: it is another repository.
  t[0].n = 0; repo_tags(t[..], keys[BOB]);
  bobs := sign(secret[BOB][..], T0, 30617, tags_of(t[..]), "", buf[7][..]);
  show_resolve("the issue against Bob's tea-tools", names[..], n34_resolve(nt_signed_of(bobs), nt_signed_of(issue), signed[..]));

  print("refusals\n");
  bad: [8192] u8;
  e := unsigned(30617, "[[\"d\",\"tea-tools\"],[\"r\",\"4b825dc642cb6eb9a060e54bf8d69288fbee490\",\"euc\"]]", bad[..]);
  print("repository, euc of 39 digits: %\n", STATUS[n34_repo(nt_event_of(e)).status]);
  e = unsigned(30617, "[[\"d\",\"tea-tools\"],[\"clone\"]]", bad[..]);
  print("repository, clone without a URL: %\n", STATUS[n34_repo(nt_event_of(e)).status]);
  e = unsigned(30617, "[[\"name\",\"Tea tools\"]]", bad[..]);
  print("repository without d: %\n", STATUS[n34_repo(nt_event_of(e)).status]);
  e = unsigned(1621, "[[\"subject\",\"No repository\"]]", bad[..]);
  print("issue without a: %\n", STATUS[n34_issue(nt_event_of(e)).status]);
  e = unsigned(1621, "[[\"a\",\"30617:F9308A019258C31049344F85F89D5229B531C845836F99B08601F113BCE036F9:tea-tools\"]]", bad[..]);
  print("issue, a with an uppercase key: %\n", STATUS[n34_issue(nt_event_of(e)).status]);
  t[0].n = 0; put(t[..], "[[\"a\",\""); put(t[..], tags_of(a[..])); put(t[..], "\"],[\"commit\",\"9c4f6e2b\"]]");
  e = unsigned(1617, tags_of(t[..]), bad[..]);
  print("patch, commit of 8 digits: %\n", STATUS[n34_patch(nt_event_of(e)).status]);
  e = unsigned(1617, "[[\"a\",\"30621:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"]]", bad[..]);
  print("patch naming only a project: %\n", STATUS[n34_patch(nt_event_of(e)).status]);
  e = unsigned(1631, "[[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"]]", bad[..]);
  print("status without e: %\n", STATUS[n34_status(nt_event_of(e)).status]);
  e = unsigned(1631, "[[\"e\",\"9c4f6e2b\",\"\",\"root\"]]", bad[..]);
  print("status, e of 8 digits: %\n", STATUS[n34_status(nt_event_of(e)).status]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/repositories.coax --run-steps 250000000
```

Its output, in debug and release builds alike
([nostr/fixtures/repositories.out](../fixtures/repositories.out)):

```text
the repository announcement, by Alice
Alice: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
Bob: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
Carol: dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8
Dave: 25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517
read: ok, d tea-tools, name "Tea tools", clone 2 values, first https://git.example.com/tea-tools.git, euc 4b825dc642cb6eb9a060e54bf8d69288fbee4904
maintainers: Alice (the author) and Bob
its address: 30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools
an issue and a patch, by Carol
issue tags: [["a","30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["subject","The timer stops at 59 seconds"],["t","bug"]]
issue: ok, repository tea-tools (the address above), subject "The timer stops at 59 seconds"
patch: ok, root true, revision false, commit 9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6, parent 5d7f1e0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e
statuses
S1..S3 tags: [["e","f56388ff36057e4382436dcb6c7e466328019080c3a4617ba03555d92491b47e","","root"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["p","dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"],["a","30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools"]]
S1: kind 1630 by Carol at T0+100, state open (1630)
S2: kind 1631 by Bob at T0+200, state applied, merged or resolved (1631)
S3: kind 1632 by Dave at T0+300, state closed (1632)
S4: kind 1631 by Alice at T0+400, state applied, merged or resolved (1631)
each event's id
repository: 8c99e43141fd7751a14b2419acea50a71a7db6fc89afd3551f2d6418b2b2b513, verified true
issue: f56388ff36057e4382436dcb6c7e466328019080c3a4617ba03555d92491b47e, verified true
patch: 606040c51ab0b8131b7e6c3ca860d45c310d300ec4aefa0bb3ee78649cb420c8, verified true
S1: 33e5c7180963549bbd238029724b512732c60b3d4497857726b0b582e3501014, verified true
S2: 4e0c22de598048943e380a75221b3b20351958fcd50564121e4ec76c8ff0ccd4, verified true
S3: 671a13e44d859aedc490b7b48e6ebb552afaae12c535dfc6cc59d500dc6291f3, verified true
S4: 187a30002610c4704b812372c6a0a5ca3d6db3afba75061ba50744da7d552810, verified true
the issue: ok, state applied, merged or resolved (1631), winner S2, counted 2, ignored 1, refused 0
the patch: ok, state applied, merged or resolved (1631), winner S4, counted 1, ignored 0, refused 0
the issue without S2: ok, state open (1630), winner S1, counted 1, ignored 1, refused 0
the issue against Bob's tea-tools: elsewhere
refusals
repository, euc of 39 digits: malformed
repository, clone without a URL: malformed
repository without d: missing
issue without a: missing
issue, a with an uppercase key: malformed
patch, commit of 8 digits: malformed
patch naming only a project: missing
status without e: missing
status, e of 8 digits: malformed
```

The resolutions match the worked example: the issue is resolved by S2 with
Dave's S3 ignored, the patch is applied by S4, and without S2 the issue is
open by S1. Bob's `tea-tools` has the same `d` but another author, so
Carol's issue is `elsewhere` for it. Each refusal is the reader's status
code: `malformed` for a value of the wrong shape, `missing` for a required
tag that isn't there.

### The snippets and pull requests example

A second example,
[nostr/examples/snippets-pull-requests.coax](../examples/snippets-pull-requests.coax),
uses the same four keys and Alice's `tea-tools` address. It signs Carol's
snippet, her pull request and the two updates of the worked example, reads
each one back, works out the tip three ways, and then shows the refusals.

<!-- source: nostr/examples/snippets-pull-requests.coax example -->
```jai
#load "../../coax/lib/nostr/nip34-event.coax";
#load "../../coax/lib/nostr/nip34-pr.coax";
#load "../../coax/lib/nostr/nipc0.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

STATUS :: [6] string.["ok", "event", "kind", "missing", "malformed", "output"];
REPO_IS :: [3] string.["none", "a URL", "a repository's address"];
// BIP-340 test vectors 0, 1, 2 and 3's published secret keys. Never sign
// with a real key in an example.
KEYS :: [4] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9",
  "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710"];
PEOPLE :: [4] string.["Alice", "Bob", "Carol", "Dave"];
ALICE :: 0; BOB :: 1; CAROL :: 2; DAVE :: 3;
// The branch's tip when the pull request opens, after Carol's fix, and the
// commit Dave tries to move it to.
TIP_1 :: "9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6";
TIP_2 :: "2b1d0c9f8e7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c";
TIP_3 :: "7e6d5c4b3a291807f6e5d4c3b2a190807f6e5d4c";
CLONE :: "https://git.example.com/carol/tea-tools.git";
CODE :: "fn seconds(minutes: u32, seconds: u32) -> u32 {\n    minutes * 60 + seconds\n}\n";
T0 :: 1761686400;

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}

// Sign an event into out and read it back with its ID and signature checked.
sign :: (key: [] u8, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  aux: [32] u8; work: [1] NostrSignWork; scratch: [8192] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key, aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [8192] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}

// An unsigned event with these tags, parsed but not verified: its id and
// sig are zeros. Used for the refusals, which the readers refuse on their
// kind and tags alone.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

who :: (keys: [] [32] u8, key: [32] u8) -> string {
  for k: 0..<keys.count { if n34_same(keys[k], key) { return PEOPLE[k]; } }
  return "someone else";
}
show_head :: (label: string, names: [] string, h: N34Head) {
  print("%: %", label, STATUS[h.status]);
  if h.status != N34_OK { print("\n"); return; }
  tip := h.commit;
  print(", tip %, from %, counted %, ignored %, refused %\n", cast(string) tip[..], ifx h.winner >= 0 then names[h.winner] else "the pull request", h.counted, h.ignored, h.refused);
}

main :: () {
  secret: [4][32] u8; keys: [4][32] u8; pk: [4][64] u8;
  for k: 0..<4 { decode_hex(KEYS[k], secret[k][..]); }
  buf: [6][8192] u8;
  // Each public key comes from signing: sign an empty note first.
  for k: 0..<4 { keys[k] = sign(secret[k][..], T0, 1, "[]", "", buf[0][..]).public_key; pk[k] = n34_hex32(keys[k]); }
  for k: 0..<4 { print("%: %\n", PEOPLE[k], cast(string) pk[k][..]); }
  // Alice's repository from repositories.coax: "30617:<Alice>:tea-tools".
  address: [128] u8; a := json_raw(address[..], 0, "30617:"); a = json_raw(address[..], a, cast(string) pk[ALICE][..]); a = json_raw(address[..], a, ":tea-tools");
  repo := cast(string) address[0..<a];
  print("the repository's address: %\n", repo);

  print("a code snippet (kind 1337), by Carol\n");
  tags: [2048] u8;
  st: NC0Tags; st.language = "rust"; st.extension = "rs"; st.name = "seconds.rs"; st.description = "Minutes and seconds as seconds";
  st.runtime = "rustc 1.82"; st.license = "MIT"; st.repo = repo;
  n := nc0_tags(st, tags[..]);
  #assert n > 0;
  print("S1 tags: %\n", cast(string) tags[0..<n]);
  s1 := sign(secret[CAROL][..], T0, 1337, cast(string) tags[0..<n], CODE, buf[0][..]);
  se := nt_event_of(s1); s := nc0_snippet(se);
  print("S1: %, language %, extension %, name %, runtime \"%\", licenses %, deps %, repo is %\n", STATUS[s.status], nt_raw(se, s.language.v[0]),
    nt_raw(se, s.extension.v[0]), nt_raw(se, s.name.v[0]), nt_raw(se, s.runtime.v[0]), s.licenses, s.deps, REPO_IS[s.repo_is]);
  print("S1's content, as its JSON text: %\n", se.original[se.content_from..<se.content_to]);

  print("a pull request (kind 1618), by Carol\n");
  n = n34_pull_tags(repo, cast(string) pk[ALICE][..], "Count minutes past 59 seconds", "timer-minutes", TIP_1, CLONE, tags[..]);
  #assert n > 0;
  print("PR1 tags: %\n", cast(string) tags[0..<n]);
  pr := sign(secret[CAROL][..], T0 + 100, 1618, cast(string) tags[0..<n], "The timer stopped at 0:59. This counts whole minutes.", buf[1][..]);
  pe := nt_event_of(pr); p := n34_pull(pe);
  print("PR1: %, repository %, subject \"%\", branch %, tip %, clone %\n", STATUS[p.status], ifx nt_raw(pe, p.repo) == repo then "tea-tools (the address above)" else "another",
    nt_raw(pe, p.subject.v[0]), nt_raw(pe, p.branch.v[0]), nt_raw(pe, p.tip.commit.v[0]), nt_raw(pe, p.tip.clone.v[0]));

  print("updates (kind 1619)\n");
  id := n34_hex32(pr.id);
  set: [2] NostrEvent; names := [2] string.["U1", "U2"];
  n = n34_pull_update_tags(repo, cast(string) pk[ALICE][..], cast(string) id[..], cast(string) pk[CAROL][..], TIP_2, CLONE, tags[..]);
  #assert n > 0;
  print("U1 tags: %\n", cast(string) tags[0..<n]);
  set[0] = sign(secret[CAROL][..], T0 + 200, 1619, cast(string) tags[0..<n], "", buf[2][..]);
  n = n34_pull_update_tags(repo, cast(string) pk[ALICE][..], cast(string) id[..], cast(string) pk[CAROL][..], TIP_3, "https://git.example.com/dave/tea-tools.git", tags[..]);
  #assert n > 0;
  set[1] = sign(secret[DAVE][..], T0 + 300, 1619, cast(string) tags[0..<n], "", buf[3][..]);
  for k: 0..<2 {
    ue := nt_event_of(set[k]); u := n34_pull_update(ue);
    print("%: % by % at T0+%, moves %to %\n", names[k], STATUS[u.status], who(keys[..], set[k].public_key), set[k].created_at - T0,
      ifx nt_raw(ue, u.pull.v[0]) == cast(string) id[..] then "PR1 " else "another pull request ", nt_raw(ue, u.tip.commit.v[0]));
  }
  signed: [2] N34Signed; for k: 0..<2 { signed[k] = nt_signed_of(set[k]); }
  show_head("PR1 alone", names[..], n34_head(nt_signed_of(pr), signed[0..<0]));
  show_head("PR1 with U1 and U2", names[..], n34_head(nt_signed_of(pr), signed[..]));
  // Dave's update alone moves nothing.
  dave_names := [1] string.["U2"];
  show_head("PR1 with U2 only", dave_names[..], n34_head(nt_signed_of(pr), signed[1..<2]));

  print("each event's id\n");
  all := [4] NostrEvent.[s1, pr, set[0], set[1]];
  all_names := [4] string.["S1", "PR1", "U1", "U2"];
  for k: 0..<4 { h := n34_hex32(all[k].id); print("%: %, verified %\n", all_names[k], cast(string) h[..], all[k].verified); }

  print("refusals\n");
  bad: [8192] u8;
  e := unsigned(1337, "[[\"l\",\"Rust\"]]", bad[..]);
  print("snippet, language in capitals: %\n", STATUS[nc0_snippet(nt_event_of(e)).status]);
  e = unsigned(1337, "[[\"extension\",\".rs\"]]", bad[..]);
  print("snippet, extension with its dot: %\n", STATUS[nc0_snippet(nt_event_of(e)).status]);
  e = unsigned(1337, "[[\"repo\",\"tea-tools\"]]", bad[..]);
  print("snippet, repo that is neither a URL nor an address: %\n", STATUS[nc0_snippet(nt_event_of(e)).status]);
  e = unsigned(1337, "[[\"license\"]]", bad[..]);
  print("snippet, license without a value: %\n", STATUS[nc0_snippet(nt_event_of(e)).status]);
  t: [1024] u8; m := json_raw(t[..], 0, "[[\"a\",\""); m = json_raw(t[..], m, repo); m = json_raw(t[..], m, "\"],[\"c\",\"9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6\"]]");
  e = unsigned(1618, cast(string) t[0..<m], bad[..]);
  print("pull request without a clone URL: %\n", STATUS[n34_pull(nt_event_of(e)).status]);
  m = json_raw(t[..], 0, "[[\"a\",\""); m = json_raw(t[..], m, repo); m = json_raw(t[..], m, "\"],[\"c\",\"9c4f6e2b\"],[\"clone\",\"https://git.example.com/carol/tea-tools.git\"]]");
  e = unsigned(1618, cast(string) t[0..<m], bad[..]);
  print("pull request, c of 8 digits: %\n", STATUS[n34_pull(nt_event_of(e)).status]);
  e = unsigned(1618, "[[\"c\",\"9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6\"],[\"clone\",\"https://git.example.com/carol/tea-tools.git\"]]", bad[..]);
  print("pull request that names no repository: %\n", STATUS[n34_pull(nt_event_of(e)).status]);
  m = json_raw(t[..], 0, "[[\"a\",\""); m = json_raw(t[..], m, repo); m = json_raw(t[..], m, "\"],[\"c\",\"2b1d0c9f8e7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c\"],[\"clone\",\"https://git.example.com/carol/tea-tools.git\"]]");
  e = unsigned(1619, cast(string) t[0..<m], bad[..]);
  print("update that names no pull request: %\n", STATUS[n34_pull_update(nt_event_of(e)).status]);
  print("pull request tags with a tip of 8 digits: %\n", n34_pull_tags(repo, cast(string) pk[ALICE][..], "s", "b", "9c4f6e2b", CLONE, tags[..]));
}
```

```shell
coax/bin/coax run nostr/examples/snippets-pull-requests.coax --run-steps 250000000
```

It prints
([nostr/fixtures/snippets-pull-requests.out](../fixtures/snippets-pull-requests.out)):

```text
Alice: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
Bob: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
Carol: dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8
Dave: 25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517
the repository's address: 30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools
a code snippet (kind 1337), by Carol
S1 tags: [["l","rust"],["extension","rs"],["name","seconds.rs"],["description","Minutes and seconds as seconds"],["runtime","rustc 1.82"],["license","MIT"],["repo","30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools"]]
S1: ok, language rust, extension rs, name seconds.rs, runtime "rustc 1.82", licenses 1, deps 0, repo is a repository's address
S1's content, as its JSON text: fn seconds(minutes: u32, seconds: u32) -> u32 {\n    minutes * 60 + seconds\n}\n
a pull request (kind 1618), by Carol
PR1 tags: [["a","30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["c","9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6"],["clone","https://git.example.com/carol/tea-tools.git"],["subject","Count minutes past 59 seconds"],["branch-name","timer-minutes"]]
PR1: ok, repository tea-tools (the address above), subject "Count minutes past 59 seconds", branch timer-minutes, tip 9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6, clone https://git.example.com/carol/tea-tools.git
updates (kind 1619)
U1 tags: [["a","30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools"],["p","f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9"],["c","2b1d0c9f8e7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c"],["clone","https://git.example.com/carol/tea-tools.git"],["E","54c4d3eec90a68ba7625305610f782ced95cc56de287f19d368b5125ff1bd3a8"],["P","dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8"]]
U1: ok by Carol at T0+200, moves PR1 to 2b1d0c9f8e7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c
U2: ok by Dave at T0+300, moves PR1 to 7e6d5c4b3a291807f6e5d4c3b2a190807f6e5d4c
PR1 alone: ok, tip 9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6, from the pull request, counted 0, ignored 0, refused 0
PR1 with U1 and U2: ok, tip 2b1d0c9f8e7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c, from U1, counted 1, ignored 1, refused 0
PR1 with U2 only: ok, tip 9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6, from the pull request, counted 0, ignored 1, refused 0
each event's id
S1: cdee62118e1266c7871f0a4484d828062e47ce8fba14db75b5714f8922c8f5e9, verified true
PR1: 54c4d3eec90a68ba7625305610f782ced95cc56de287f19d368b5125ff1bd3a8, verified true
U1: 158583eda69df86ffe5cf803fa7c0117b222620ceda12f56b8ab016ee37f6e53, verified true
U2: 8d9c61e0e53de8d1db8759e94fb7a322eb23439a38d836f1b9e52641432fe073, verified true
refusals
snippet, language in capitals: malformed
snippet, extension with its dot: malformed
snippet, repo that is neither a URL nor an address: malformed
snippet, license without a value: malformed
pull request without a clone URL: missing
pull request, c of 8 digits: malformed
pull request that names no repository: missing
update that names no pull request: missing
pull request tags with a tip of 8 digits: -1
```

The tips match the worked example: PR1's own commit when it is alone,
Carol's U1 when both updates are there, and PR1's own commit again when
Dave's U2 is the only update. The snippet's `repo` tag reads as a
repository's address. The refusals: a language in capitals, an extension
with its dot, a `repo` that is a bare name and a `license` with no value
are `malformed`; a pull request with no `clone`, with no `a` tag, and an
update with no `E` are `missing`; a `c` of 8 digits is `malformed`; and
`n34_pull_tags` returns -1 for a tip of 8 digits and writes nothing.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/repositories.coax](../examples/checks/repositories.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The ids of all seven events and the tags the example writes, from a
  Python script written from NIP-01 and NIP-34 (`hashlib.sha256` over
  NIP-01's serialization).
- The resolution worked by hand above, in four orders of S1 to S4.
- PLAN.md's "Done means": Dave's close alone leaves the issue open with
  one ignored status; the same close by Bob counts.
- NIP-34's status layout, with a `reply` `e` before the `root` one and the
  `q` and `merge-commit` tags, reads its root.

<!-- source: nostr/examples/checks/repositories.coax check -->
```jai
// The ids and tags from a Python script written from NIP-01 and NIP-34 (it
// shares no code with Coax): the ids are hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False).
PUB :: [4] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8",
  "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517"];
PY_ID :: [7] string.[
  "8c99e43141fd7751a14b2419acea50a71a7db6fc89afd3551f2d6418b2b2b513",
  "f56388ff36057e4382436dcb6c7e466328019080c3a4617ba03555d92491b47e",
  "606040c51ab0b8131b7e6c3ca860d45c310d300ec4aefa0bb3ee78649cb420c8",
  "33e5c7180963549bbd238029724b512732c60b3d4497857726b0b582e3501014",
  "4e0c22de598048943e380a75221b3b20351958fcd50564121e4ec76c8ff0ccd4",
  "671a13e44d859aedc490b7b48e6ebb552afaae12c535dfc6cc59d500dc6291f3",
  "187a30002610c4704b812372c6a0a5ca3d6db3afba75061ba50744da7d552810"];
PY_REPO :: "[[\"d\",\"tea-tools\"],[\"name\",\"Tea tools\"],[\"description\",\"Timers and scales for brewing tea\"],[\"web\",\"https://git.example.com/tea-tools\"],[\"clone\",\"https://git.example.com/tea-tools.git\",\"ssh://git@git.example.com/tea-tools.git\"],[\"relays\",\"wss://relay.example.com\"],[\"r\",\"4b825dc642cb6eb9a060e54bf8d69288fbee4904\",\"euc\"],[\"maintainers\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"]]";
PY_ISSUE :: "[[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"subject\",\"The timer stops at 59 seconds\"],[\"t\",\"bug\"]]";
PY_PATCH :: "[[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"],[\"r\",\"4b825dc642cb6eb9a060e54bf8d69288fbee4904\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"t\",\"root\"],[\"commit\",\"9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6\"],[\"parent-commit\",\"5d7f1e0c9b8a7f6e5d4c3b2a1f0e9d8c7b6a5f4e\"]]";
PY_ON_ISSUE :: "[[\"e\",\"f56388ff36057e4382436dcb6c7e466328019080c3a4617ba03555d92491b47e\",\"\",\"root\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"p\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"]]";
PY_ON_PATCH :: "[[\"e\",\"606040c51ab0b8131b7e6c3ca860d45c310d300ec4aefa0bb3ee78649cb420c8\",\"\",\"root\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"p\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"]]";
PY_AUTHOR :: [7] int.[0, 2, 2, 2, 1, 3, 0];
PY_AT :: [7] int.[0, 60, 90, 100, 200, 300, 400];
PY_KIND :: [7] int.[30617, 1621, 1617, 1630, 1631, 1632, 1631];
PY_TAGS :: [7] string.[PY_REPO, PY_ISSUE, PY_PATCH, PY_ON_ISSUE, PY_ON_ISSUE, PY_ON_ISSUE, PY_ON_PATCH];
PY_CONTENT :: [7] string.["", "Set the timer to 1:30 and it stops at 0:59.",
  "From 9c4f6e2b Mon Sep 17 00:00:00 2001\nSubject: [PATCH] timer: count minutes past 59 seconds\n",
  "Still happens on 2.1.", "Fixed by the patch.", "Not a bug.", "Applied."];

// Event i of the script with a zero signature, read with its id recomputed;
// the signature is not checked.
py :: (i: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[i]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[PY_AUTHOR[i]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + PY_AT[i]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, PY_KIND[i]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, PY_TAGS[i]);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, PY_CONTENT[i]);
  n = json_raw(out, n, ",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}
py_key :: (k: int) -> [32] u8 { out: [32] u8; decode_hex(PUB[k], out[..]); return out; }

#test "the ids are Python's, and so are the tags the example writes" {
  buf: [7][4096] u8; scratch: [4096] u8;
  for i: 0..<7 { e := py(i, buf[i][..], scratch[..]); #assert e.error == NE_OK && n34_same(e.computed_id, e.id); }
  issue := py(1, buf[1][..], scratch[..]); patch := py(2, buf[2][..], scratch[..]);
  t: [1] Tags;
  repo_tags(t[..], py_key(BOB)); #assert tags_of(t[..]) == PY_REPO;
  t[0].n = 0; issue_tags(t[..], py_key(ALICE)); #assert tags_of(t[..]) == PY_ISSUE;
  t[0].n = 0; patch_tags(t[..], py_key(ALICE)); #assert tags_of(t[..]) == PY_PATCH;
  t[0].n = 0; status_tags(t[..], issue, py_key(ALICE)); #assert tags_of(t[..]) == PY_ON_ISSUE;
  t[0].n = 0; status_tags(t[..], patch, py_key(ALICE)); #assert tags_of(t[..]) == PY_ON_PATCH;
}

// By hand: S1 (Carol, the issue's author, open, T0+100), S2 (Bob, a
// maintainer, resolved, T0+200) and S3 (Dave, neither, closed, T0+300) are
// about the issue; S4 (Alice, the owner, applied, T0+400) is about the
// patch. Dave's is the newest but doesn't count, so S2 decides: resolved,
// counted 2, ignored 1. The patch: S4, applied.
#test "the statuses resolve as worked by hand, in any order" {
  buf: [7][4096] u8; scratch: [4096] u8; ev: [7] N34Signed;
  for i: 0..<7 { ev[i] = nt_signed_of(py(i, buf[i][..], scratch[..])); }
  // Four orders of S1..S4 (events 3 to 6), four entries each.
  orders := [16] int.[3, 4, 5, 6, 6, 5, 4, 3, 5, 3, 6, 4, 4, 6, 3, 5];
  for o: 0..<4 {
    set: [4] N34Signed; for k: 0..<4 { set[k] = ev[orders[4 * o + k]]; }
    s := n34_resolve(ev[0], ev[1], set[..]);
    #assert s.status == N34_OK && s.state == N34_STATE_RESOLVED && s.counted == 2 && s.ignored == 1 && s.refused == 0;
    #assert n34_same(set[s.winner].id, ev[4].id);
    s = n34_resolve(ev[0], ev[2], set[..]);
    #assert s.status == N34_OK && s.state == N34_STATE_RESOLVED && s.counted == 1 && s.ignored == 0 && n34_same(set[s.winner].id, ev[6].id);
  }
}

// PLAN.md's "Done means": a status from someone who is neither the author
// nor a maintainer is ignored. Dave's close alone leaves the issue open.
#test "a status by neither the author nor a maintainer is ignored" {
  buf: [7][4096] u8; scratch: [4096] u8; ev: [7] N34Signed;
  for i: 0..<7 { ev[i] = nt_signed_of(py(i, buf[i][..], scratch[..])); }
  dave := [1] N34Signed.[ev[5]];
  s := n34_resolve(ev[0], ev[1], dave[..]);
  #assert s.status == N34_OK && s.state == N34_STATE_OPEN && s.winner == -1 && s.counted == 0 && s.ignored == 1;
  // The same close by Bob, who the announcement names, counts.
  bob := ev[5]; bob.public_key = py_key(BOB);
  one := [1] N34Signed.[bob];
  s = n34_resolve(ev[0], ev[1], one[..]);
  #assert s.state == N34_STATE_CLOSED && s.counted == 1 && s.ignored == 0;
}

// NIP-34's status example, with a reply e for an accepted revision, q,
// merge-commit and r tags: the root is the e marked root, wherever it is.
#test "NIP-34's status layout reads its root" {
  tags := "[[\"e\",\"0202020202020202020202020202020202020202020202020202020202020202\",\"\",\"reply\"],[\"e\",\"f56388ff36057e4382436dcb6c7e466328019080c3a4617ba03555d92491b47e\",\"\",\"root\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\",\"wss://relay.example.com\"],[\"r\",\"4b825dc642cb6eb9a060e54bf8d69288fbee4904\"],[\"q\",\"606040c51ab0b8131b7e6c3ca860d45c310d300ec4aefa0bb3ee78649cb420c8\",\"wss://relay.example.com\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"merge-commit\",\"9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6\"]]";
  e := n34_event(tags, 1631); s := n34_status(e);
  #assert s.status == N34_OK && s.state == N34_STATE_RESOLVED && nt_raw(e, s.root.v[0]) == PY_ID[1];
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/repositories.coax --run-steps 250000000
```

`coax test` runs 371 checks, including the loaded files' own checks; all
pass.

### Checks of snippets and pull requests

The checks are in
[nostr/examples/checks/snippets-pull-requests.coax](../examples/checks/snippets-pull-requests.coax),
which loads the second example. They compare the code with values that
share no code with it:

- The ids of the four events and the tags `nc0_tags`, `n34_pull_tags` and
  `n34_pull_update_tags` write, from a Python script written from NIP-01,
  NIP-34 and NIP-C0.
- The tip worked by hand above, with the updates in both orders, with
  Dave's alone, and with Dave's update signed by Carol, which then counts.
- NIP-C0's example event's tags, verbatim, and NIP-34's pull request and
  update layouts with every tag they list, in their order.

<!-- source: nostr/examples/checks/snippets-pull-requests.coax check -->
```jai
// The ids and tags from a Python script written from NIP-01, NIP-34 and NIP-C0
// (it shares no code with Coax): the ids are hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False).
PUB :: [4] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8",
  "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517"];
PY_ID :: [4] string.[
  "cdee62118e1266c7871f0a4484d828062e47ce8fba14db75b5714f8922c8f5e9",
  "54c4d3eec90a68ba7625305610f782ced95cc56de287f19d368b5125ff1bd3a8",
  "158583eda69df86ffe5cf803fa7c0117b222620ceda12f56b8ab016ee37f6e53",
  "8d9c61e0e53de8d1db8759e94fb7a322eb23439a38d836f1b9e52641432fe073"];
PY_S1 :: "[[\"l\",\"rust\"],[\"extension\",\"rs\"],[\"name\",\"seconds.rs\"],[\"description\",\"Minutes and seconds as seconds\"],[\"runtime\",\"rustc 1.82\"],[\"license\",\"MIT\"],[\"repo\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"]]";
PY_PR1 :: "[[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"c\",\"9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6\"],[\"clone\",\"https://git.example.com/carol/tea-tools.git\"],[\"subject\",\"Count minutes past 59 seconds\"],[\"branch-name\",\"timer-minutes\"]]";
PY_U1 :: "[[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"c\",\"2b1d0c9f8e7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c\"],[\"clone\",\"https://git.example.com/carol/tea-tools.git\"],[\"E\",\"54c4d3eec90a68ba7625305610f782ced95cc56de287f19d368b5125ff1bd3a8\"],[\"P\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"]]";
PY_U2 :: "[[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"c\",\"7e6d5c4b3a291807f6e5d4c3b2a190807f6e5d4c\"],[\"clone\",\"https://git.example.com/dave/tea-tools.git\"],[\"E\",\"54c4d3eec90a68ba7625305610f782ced95cc56de287f19d368b5125ff1bd3a8\"],[\"P\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"]]";
PY_AUTHOR :: [4] int.[2, 2, 2, 3];
PY_AT :: [4] int.[0, 100, 200, 300];
PY_KIND :: [4] int.[1337, 1618, 1619, 1619];
PY_TAGS :: [4] string.[PY_S1, PY_PR1, PY_U1, PY_U2];
PY_CONTENT :: [4] string.["fn seconds(minutes: u32, seconds: u32) -> u32 {\n    minutes * 60 + seconds\n}\n", "The timer stopped at 0:59. This counts whole minutes.", "", ""];

// Event i of the script with a zero signature, read with its id recomputed;
// the signature is not checked.
py :: (i: int, out: [] u8, scratch: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[i]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[PY_AUTHOR[i]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + PY_AT[i]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, PY_KIND[i]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, PY_TAGS[i]);
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, PY_CONTENT[i]);
  n = json_raw(out, n, ",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}
PY_REPO :: "30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools";

#test "the ids are Python's, and so are the tags the library writes" {
  buf: [4][4096] u8; scratch: [4096] u8;
  for i: 0..<4 { e := py(i, buf[i][..], scratch[..]); #assert e.error == NE_OK && n34_same(e.computed_id, e.id); }
  tags: [2048] u8;
  st: NC0Tags; st.language = "rust"; st.extension = "rs"; st.name = "seconds.rs"; st.description = "Minutes and seconds as seconds";
  st.runtime = "rustc 1.82"; st.license = "MIT"; st.repo = PY_REPO;
  n := nc0_tags(st, tags[..]); #assert cast(string) tags[0..<n] == PY_S1;
  n = n34_pull_tags(PY_REPO, PUB[ALICE], "Count minutes past 59 seconds", "timer-minutes", TIP_1, CLONE, tags[..]); #assert cast(string) tags[0..<n] == PY_PR1;
  n = n34_pull_update_tags(PY_REPO, PUB[ALICE], PY_ID[1], PUB[CAROL], TIP_2, CLONE, tags[..]); #assert cast(string) tags[0..<n] == PY_U1;
  n = n34_pull_update_tags(PY_REPO, PUB[ALICE], PY_ID[1], PUB[CAROL], TIP_3, "https://git.example.com/dave/tea-tools.git", tags[..]); #assert cast(string) tags[0..<n] == PY_U2;
}

// By hand: PR1 (Carol, T0+100) opens at TIP_1. U1 (Carol, T0+200) names
// PR1 and tea-tools and moves the tip to TIP_2. U2 (Dave, T0+300) is newer
// and names the same pull request, but Dave did not open PR1, so it is
// ignored. The tip is TIP_2 from U1: counted 1, ignored 1.
#test "the tip is the author's newest update, in either order" {
  buf: [4][4096] u8; scratch: [4096] u8; ev: [4] N34Signed;
  for i: 0..<4 { ev[i] = nt_signed_of(py(i, buf[i][..], scratch[..])); }
  both := [2] N34Signed.[ev[2], ev[3]]; h := n34_head(ev[1], both[..]); tip := h.commit;
  #assert h.status == N34_OK && cast(string) tip[..] == TIP_2 && h.winner == 0 && h.counted == 1 && h.ignored == 1 && h.refused == 0;
  both = [2] N34Signed.[ev[3], ev[2]]; h = n34_head(ev[1], both[..]); tip = h.commit;
  #assert cast(string) tip[..] == TIP_2 && h.winner == 1 && h.counted == 1 && h.ignored == 1;
  // Dave's update alone: the tip stays the pull request's own.
  dave := [1] N34Signed.[ev[3]]; h = n34_head(ev[1], dave[..]); tip = h.commit;
  #assert cast(string) tip[..] == TIP_1 && h.winner == -1 && h.counted == 0 && h.ignored == 1;
  // The same update signed by Carol counts, and is the newest.
  carol := ev[3]; carol.public_key = ev[1].public_key; both = [2] N34Signed.[ev[2], carol]; h = n34_head(ev[1], both[..]); tip = h.commit;
  #assert cast(string) tip[..] == TIP_3 && h.winner == 1 && h.counted == 2 && h.ignored == 0;
  // The snippet reads as worked by hand: its repo is tea-tools' address.
  se := ev[0].e; sn := nc0_snippet(se);
  #assert sn.status == N34_OK && sn.repo_is == NC0_ADDRESS && nt_raw(se, sn.repo.v[0]) == PY_REPO && nt_raw(se, sn.language.v[0]) == "rust" && sn.licenses == 1;
  // A snippet is not a pull request, and a pull request is not an update.
  #assert n34_head(ev[0], both[..]).status == N34_KIND && n34_pull_update(ev[1].e).status == N34_KIND;
}

// NIP-C0's example event's tags, and NIP-34's pull request and update
// layouts with every tag they list, in their order. Where NIP-34 writes a
// placeholder, the check puts Python's keys and ids and this example's
// commits.
#test "the NIPs' own layouts" {
  e := n34_event("[[\"l\",\"javascript\"],[\"extension\",\"js\"],[\"name\",\"hello-world.js\"],[\"description\",\"A basic JavaScript function that prints 'Hello, Nostr!' to the console\"],[\"runtime\",\"node v18.15.0\"],[\"license\",\"MIT\"],[\"repo\",\"https://github.com/nostr-protocol/nostr\"]]", 1337);
  sn := nc0_snippet(e); #assert sn.status == N34_OK && sn.repo_is == NC0_URL && nt_raw(e, sn.name.v[0]) == "hello-world.js";
  e = n34_event("[[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"],[\"r\",\"4b825dc642cb6eb9a060e54bf8d69288fbee4904\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"],[\"subject\",\"Count minutes past 59 seconds\"],[\"t\",\"bug\"],[\"t\",\"timer\"],[\"c\",\"9c4f6e2bd3a1f0e8c7b6a5d4c3b2a1f0e9d8c7b6\"],[\"clone\",\"https://git.example.com/carol/tea-tools.git\",\"ssh://git@git.example.com/carol/tea-tools.git\"],[\"branch-name\",\"timer-minutes\"],[\"e\",\"cdee62118e1266c7871f0a4484d828062e47ce8fba14db75b5714f8922c8f5e9\"],[\"merge-base\",\"7e6d5c4b3a291807f6e5d4c3b2a190807f6e5d4c\"]]", 1618);
  p := n34_pull(e);
  #assert p.status == N34_OK && nt_raw(e, p.repo) == PY_REPO && nt_raw(e, p.tip.commit.v[0]) == TIP_1 && p.tip.clone.values == 2 && p.labels == 2;
  #assert nt_raw(e, p.branch.v[0]) == "timer-minutes" && nt_raw(e, p.revises.v[0]) == PY_ID[0] && nt_raw(e, p.tip.base.v[0]) == TIP_3;
  e = n34_event("[[\"a\",\"30617:f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9:tea-tools\"],[\"r\",\"4b825dc642cb6eb9a060e54bf8d69288fbee4904\"],[\"p\",\"f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"],[\"E\",\"54c4d3eec90a68ba7625305610f782ced95cc56de287f19d368b5125ff1bd3a8\"],[\"P\",\"dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8\"],[\"c\",\"2b1d0c9f8e7a6b5c4d3e2f1a0b9c8d7e6f5a4b3c\"],[\"clone\",\"https://git.example.com/carol/tea-tools.git\"],[\"merge-base\",\"7e6d5c4b3a291807f6e5d4c3b2a190807f6e5d4c\"]]", 1619);
  u := n34_pull_update(e);
  #assert u.status == N34_OK && nt_raw(e, u.pull.v[0]) == PY_ID[1] && nt_raw(e, u.author.v[0]) == PUB[CAROL] && nt_raw(e, u.tip.commit.v[0]) == TIP_2 && nt_raw(e, u.tip.base.v[0]) == TIP_3;
}
```

```shell
coax/bin/coax test nostr/examples/checks/snippets-pull-requests.coax --run-steps 250000000
```

`coax test` runs 393 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`nip34-patch.coax` has its own `#test` blocks: patches with and without
each optional tag and every refusal, kind 0 coordinates, maintainers in any
tag and position (and a key that is only a tag name, a `p` value or a
longer value, which are not maintainers), the resolver with authors,
maintainers, strangers, other roots, unreadable statuses, ties in either
order, the same event twice, drafts, kind 1634, and roots that name
another repository by author, by `d` or only by project. `nip34.coax`'s
checks cover the other readers.

```shell
coax/bin/coax test coax/lib/nostr/nip34-patch.coax
coax/bin/coax test coax/lib/nostr/nip34-event.coax
coax/bin/coax test coax/lib/nostr/nip34.coax
coax/bin/coax mutate coax/lib/nostr/nip34-patch.coax --root-only
```

On 2026-11-01, `coax test` passed all 218 checks of `nip34-patch.coax`
(with the loaded files'), and `coax mutate --root-only` killed all 178
mutants, with none surviving and none marked equivalent. `nip34.coax`
gained only comment lines (the listing markers); its 43 checks pass, and
its mutation run was not repeated for comments. The full mutation run
(`COAX_MUTATION_FULL=1`) and the project fold's transcript check were not
run.

`nip34-pr.coax` has its own `#test` blocks: a pull request with every
optional tag and with none, a kind 0 coordinate, twelve tag lists that
refuse, an update with each of `E` and `P` missing, valueless or short,
the lowercase `e` and `p` that are not an update's `E` and `P`, the tag
writers with each part of the wrong shape, and the tip with the author's
updates, a stranger's, another pull request's, another repository's, an
unreadable update, ties in either order and a pull request that doesn't
read. `nipc0.coax` checks NIP-C0's example read and written byte for
byte, a snippet with no tags, repeated licences and deps, every refusal,
and the URL shape with the bytes just outside each allowed range.

```shell
coax/bin/coax test coax/lib/nostr/nip34-pr.coax
coax/bin/coax test coax/lib/nostr/nipc0.coax
coax/bin/coax mutate coax/lib/nostr/nip34-pr.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nipc0.coax --root-only
```

On 2026-11-12, `coax test` passed all 56 checks of `nip34-pr.coax` and all
48 of `nipc0.coax` (with the loaded files'). `coax mutate --root-only`
killed all 200 mutants of `nip34-pr.coax` and all 143 of `nipc0.coax`,
with none surviving and none marked equivalent. The first runs left two
survivors in `nip34-pr.coax` (killed by a check with a kind 0 coordinate)
and three in `nipc0.coax` (killed by a one-character extension, by
removing a shape check that a later line already made, and by comparing a
length for equality). `nip34-patch.coax`
and `nip34.coax` did not change, so their runs were not repeated. The full
mutation run (`COAX_MUTATION_FULL=1`) was not run.

## Summary

- A repository announcement (30617) is addressed as
  `30617:<owner>:<d>`; `n34_repo` reads its tags and refuses a missing `d`,
  a valueless tag or a bad `euc`.
- Issues (1621) and patches (1617) point at the repository with that
  address in an `a` tag; `n34_issue` and `n34_patch` read them.
- `n34_status` reads a status's state (its kind less 1630) and its root.
- `n34_resolve` gives a root the state of the newest status by its author,
  the repository's author or a listed maintainer, ties to the lower id, and
  ignores everyone else's.
- A pull request (1618) names a commit and a clone URL; `n34_pull` reads
  it. `n34_head` gives its tip: the `c` of the newest update (1619) that
  its own author signed for the same repository, or its own `c`.
- A NIP-C0 snippet (1337) is code in the content with optional tags.
  `nc0_snippet` reads them and says whether `repo` is a URL or a NIP-34
  repository's address.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [52. Reposts, reactions and
  comments](ch052-reposts-reactions-and-comments.md)

## Sources

- [NIP-34](../nips/official/34.md)
- [NIP-C0](../nips/official/C0.md)
- [NIP-01](../nips/official/01.md)
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
