# 55. Labels, reports and trusted assertions

Clients need signals from other people: labels on notes, reports of spam,
and scores that a service computes. This chapter reads each of them as a
statement by its signer, never as a fact, and counts them only from the
signers that you choose to trust.

Part X, Every NIP. Result: NIP-32 kind 1985 labels in `L` namespaces, NIP-56
kind 1984 reports with report types, and NIP-85 kinds 30382–30385 assertions
from declared providers (kind 10040).

## What this chapter teaches

- NIP-32: `["L", namespace]` and `["l", value, namespace]`, the targets a
  kind 1985 label names, and self-labels on any other kind.
- NIP-56: `p`, `e` and `x` tags with one of the seven report types
  `nudity`, `malware`, `profanity`, `illegal`, `spam`, `impersonation` and
  `other`.
- Grouping labels by namespace and signer, and tallying reports by type,
  from a set of signers that the caller supplies. Nothing is trusted by
  default.
- NIP-85: assertion kinds per subject type, and kind 10040, where a user
  names the providers they trust for each result.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)` | Reads tag `i`: an `e`, `p` or `a` tag's values (chapter 22) |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads tag `i` of any name: the name and its first two values |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_labels(e)` | An event's labels, their namespaces and their targets |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_group(set, trusted, target, out)` | The labels about one target from trusted signers, sorted by namespace, signer and value |
| [nip56.coax](../../coax/lib/nostr/nip56.coax) | `n56_report(e)` | A report's subject, its type, and the user, note or blob it names |
| [nip56.coax](../../coax/lib/nostr/nip56.coax) | `n56_tally(set, trusted, target)` | The reports about one target from trusted reporters, by type |
| [nip85.coax](../../coax/lib/nostr/nip85.coax) | `n85_providers(e)` | The providers a kind 10040 names, per kind and result |
| [nip85.coax](../../coax/lib/nostr/nip85.coax) | `n85_assertion(e)` | An assertion's kind, subject and provider |
| [nip85.coax](../../coax/lib/nostr/nip85.coax) | `n85_result(e, providers, name)` | One result of an assertion, and whether the user named its provider for it |

All three library files are new with this chapter. Each one is an
`#authority` file: it decides which statements count, so every procedure
runs in a check and `coax mutate` leaves no survivors. None of them fetches
an event, verifies a signature or decides whom to trust. The caller
verifies the events (chapter 21), then passes them with the keys it trusts.

## Reading any tag

Chapter 22's `nostr_tag` reads the standard `e`, `p`, `a` and `d` tags and
checks their values. Labels, reports and provider lists use other names,
and a report puts something other than a relay hint in an `e` or `p` tag's
third position. `n32_tag` reads a tag of any name. It decodes the name and
the first two values, and counts the rest without reading them.

<!-- source: coax/lib/nostr/nip32.coax record -->
```jai
N32Text :: struct {b:[NT_MAX_VALUE]u8; n:int;}         // a decoded string
// A tag's name and its first two values, decoded; values counts them all.
N32Tag :: struct {error:int=NT_EVENT; values:int; v:[3]N32Text;}
N32Label :: struct {value:N32Text; ns:N32Text; at:int;}  // at: its l tag
N32Target :: struct {type:int; key:[32]u8;}              // NT_E or NT_P
N32Labels :: struct {
  status:int=N32_EVENT; at:int=-1; tag_error:int;        // the refusing tag
  self:bool;                                             // not kind 1985: about itself
  labels:[N32_MAX_LABELS]N32Label; n:int;
  namespaces:[N32_MAX_NAMESPACES]N32Text; ns_n:int;      // the L tags
  targets:[N32_MAX_TARGETS]N32Target; targets_n:int;     // e and p targets
  others:int;                                            // a, r and t targets
}
```

<!-- source: coax/lib/nostr/nip32.coax tag -->
```jai
// The index'th tag of any name: its name and first two values decoded, and
// how many values it has. Later values are counted, not read.
n32_tag :: (e:NostrEvent,index:int)->N32Tag {
  t:N32Tag;j:[1]Json;if !nt_start(j[..],e) {return t;}
  k:=0;tok:=json_next(j[..]);
  while tok==JSON_ARRAY && k<index {skipped:=json_skip(j[..]);k+=1;tok=json_next(j[..]);}
  if index<0 || tok==JSON_ARRAY_END {return n32_fail(t,NT_INDEX);}
  if tok!=JSON_ARRAY {return t;}
  v:=json_next(j[..]);
  while v!=JSON_ARRAY_END {
    if v!=JSON_STRING {return n32_fail(t,NT_EVENT);}
    if t.values<3 {
      n:=json_unescape(json_text(j[..]),t.v[t.values].b[..]);
      if n<0 {return n32_fail(t,NT_LIMIT);}
      t.v[t.values].n=n;
    }
    t.values+=1;v=json_next(j[..]);
  }
  if t.values==0 {return n32_fail(t,NT_SHAPE);}
  return n32_fail(t,NT_OK);
}
```

The decoded values are at most 256 bytes each (`NT_MAX_VALUE`, as in
chapter 22). A longer name or first or second value refuses with
`NT_LIMIT`. A value after the second can be any length, because it is not
read.

## Labels

A label is a short string in a namespace. NIP-32 uses two tags:

- `["L", namespace]` declares a namespace, such as `ISO-639-1` or
  `social.nos.ontology`.
- `["l", value, namespace]` puts a label in it, such as `en` or `NS-spm`.

A kind 1985 event labels other things. It must have one or more tags that
name them: `e` (an event), `p` (a user), `a` (an addressable event), `r` (a
relay or URL) or `t` (a topic). Any other kind can carry `l` and `L` tags
too. Then the labels are about the event itself: a self-label. A note can
say that it is in English with `["L", "ISO-639-1"], ["l", "en", "ISO-639-1"]`.

### Namespace rules

NIP-32 says that when an event has `L` tags, every `l` tag must carry a
mark that matches one of them. Without `L` tags, an unmarked label is in
the `ugc` (user generated content) namespace. `n32_labels` applies these
rules:

1. An `l` tag marked with a namespace needs an `L` tag with that namespace,
   or it refuses with `N32_NAMESPACE`. The one exception is the mark `ugc`.
2. An unmarked `l` tag is in `ugc` when the event has no `L` tags. When it
   has some, the unmarked label refuses with `N32_NAMESPACE`.
3. An `L` tag that no label uses is allowed.

Rule 1 is stricter than NIP-32, which only says that a label without `L`
tags should still carry a mark. With rule 1, a label's namespace is always
one that the event declared.

### The other refusals

- `N32_TARGET`: a kind 1985 event without an `e`, `p`, `a`, `r` or `t` tag
  labels nothing.
- `N32_MISSING`: a kind 1985 event without an `l` tag has no label.
- `N32_SHAPE`: an `L`, `l`, `r` or `t` tag without a value, or with an empty
  one.
- `N32_TAG`: an `e`, `p` or `a` target that chapter 22's `nostr_tag`
  refuses, with its error in `tag_error`. On a self-labelled event these
  tags aren't targets, so they aren't read.
- `N32_LIMIT`: more than 8 labels, 4 namespaces or 8 `e` and `p` targets,
  or a value over the bound.

The refusing tag's index is in `at`, or `-1` when the refusal names no tag.

<!-- source: coax/lib/nostr/nip32.coax read -->
```jai
// The labels an event carries. Kind 1985 labels the targets its e, p, a, r
// and t tags name; any other kind labels itself.
n32_labels :: (e:NostrEvent)->N32Labels {
  r:N32Labels;if e.error!=NE_OK {return r;}
  r.self=e.kind!=N32_KIND;
  marks:[N32_MAX_LABELS]N32Text;marked:[N32_MAX_LABELS]bool;
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n32_refuse(r,N32_LIMIT,i);}
    if t.error!=NT_OK {return n32_refuse(r,N32_EVENT,i);}
    name:=t.v[0];
    if n32_is(name,"L") {
      if t.v[1].n==0 {return n32_refuse(r,N32_SHAPE,i);}
      if n32_find(r.namespaces[0..<r.ns_n],t.v[1])<0 {
        if r.ns_n==N32_MAX_NAMESPACES {return n32_refuse(r,N32_LIMIT,i);}
        r.namespaces[r.ns_n]=t.v[1];r.ns_n+=1;
      }
    } else if n32_is(name,"l") {
      if t.v[1].n==0 {return n32_refuse(r,N32_SHAPE,i);}
      if r.n==N32_MAX_LABELS {return n32_refuse(r,N32_LIMIT,i);}
      r.labels[r.n].value=t.v[1];r.labels[r.n].at=i;
      marked[r.n]=t.v[2].n>0;marks[r.n]=t.v[2];r.n+=1;
    } else if !r.self && (n32_is(name,"e") || n32_is(name,"p") || n32_is(name,"a")) {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {r.tag_error=s.error;return n32_refuse(r,N32_TAG,i);}
      if s.type==NT_A {r.others+=1;}
      else {
        if r.targets_n==N32_MAX_TARGETS {return n32_refuse(r,N32_LIMIT,i);}
        r.targets[r.targets_n].type=s.type;
        r.targets[r.targets_n].key=ifx s.type==NT_E then s.id else s.pubkey;
        r.targets_n+=1;
      }
    } else if !r.self && (n32_is(name,"r") || n32_is(name,"t")) {
      if t.v[1].n==0 {return n32_refuse(r,N32_SHAPE,i);}
      r.others+=1;
    }
    i+=1;
  }
  // Each label's namespace: its mark, which must be an L tag's value, or
  // "ugc" when it is unmarked and there are no L tags, or marked "ugc".
  for k:0..<r.n {
    ugc:N32Text;ugc.b[0]='u';ugc.b[1]='g';ugc.b[2]='c';ugc.n=3;
    if !marked[k] {
      if r.ns_n>0 {return n32_refuse(r,N32_NAMESPACE,r.labels[k].at);}
      r.labels[k].ns=ugc;
    } else {
      if n32_find(r.namespaces[0..<r.ns_n],marks[k])<0 && !n32_is(marks[k],"ugc") {return n32_refuse(r,N32_NAMESPACE,r.labels[k].at);}
      r.labels[k].ns=marks[k];
    }
  }
  if !r.self && r.targets_n+r.others==0 {return n32_refuse(r,N32_TARGET,-1);}
  if !r.self && r.n==0 {return n32_refuse(r,N32_MISSING,-1);}
  r.status=N32_OK;return r;
}
```

`n32_labels` keeps each `e` and `p` target's 32-byte key in `targets`, and
counts the `a`, `r` and `t` targets in `others`. The grouping below matches
a target by event id or pubkey, so labels on an address, a URL or a topic
read but aren't grouped.

## Reports

A report is a kind 1984 event. Its tags name what it reports:

- `["p", pubkey, type]`: a user. NIP-56 says a report must have one.
- `["e", id, type]`: a note. A report of a note also has the author's `p`
  tag.
- `["x", hash, type]`: a blob, by its SHA-256 hash. A blob report must also
  have an `e` tag for the event that holds the blob.

The type is the tag's third entry, and it is one of seven values. The set
is closed: a client that reads `scam` can't know what the reporter meant.

| Type | Meaning |
| --- | --- |
| `nudity` | depictions of nudity, porn and so on |
| `malware` | a virus, a trojan, spyware and so on |
| `profanity` | profanity or hateful speech |
| `illegal` | something that may be illegal somewhere |
| `spam` | spam |
| `impersonation` | someone pretending to be someone else |
| `other` | anything else |

### The subject and its type

`n56_report` takes the report's subject to be its `x` tag when it has one,
else its `e` tag, else its `p` tag. The subject's tag carries the report
type. In NIP-56's examples, the note report types the `e` tag and leaves
the `p` tag untyped, and the blob report types both `x` and `e`.

The reader refuses:

- `N56_KIND`: not kind 1984.
- `N56_TYPE`: a `p`, `e` or `x` tag whose third entry is not one of the
  seven types. This applies to every such tag, not only the subject's.
- `N56_UNTYPED`: the subject's tag has no type.
- `N56_MISSING`: no `p` tag and no `x` tag.
- `N56_BLOB`: an `x` tag without an `e` tag.
- `N56_TAG`: a key, id or hash that isn't 64 lowercase hex digits.

NIP-56 says that a report must have a `p` tag, but its own blob example has
only `x` and `e` tags. `n56_report` accepts that example: a report needs a
`p` tag or an `x` tag.

A report's third entry is where NIP-01 puts a relay hint, so
`["p", key, "spam"]` would fail chapter 22's relay check. That is why
`n56_report` reads its tags with `n32_tag`, not `nostr_tag`.

<!-- source: coax/lib/nostr/nip56.coax record -->
```jai
N56Report :: struct {
  status:int=N56_EVENT; at:int=-1;            // the refusing tag
  type:int=-1;                                // the subject's report type
  subject:int;                                // p, e or x
  has_pubkey:bool; pubkey:[32]u8;             // the first p tag
  has_event:bool; event:[32]u8;               // the first e tag
  has_blob:bool; blob:[32]u8;                 // the first x tag
}
N56Tally :: struct {
  counts:[7]int;      // trusted reporters per type, one per reporter and type
  ignored:int;        // reports about the target by reporters not trusted
  duplicates:int;     // a type a trusted reporter already gave
  refused:int;        // kind 1984 events that don't read
}
```

<!-- source: coax/lib/nostr/nip56.coax read -->
```jai
N56_TYPES :: [7] string.["nudity", "malware", "profanity", "illegal", "spam", "impersonation", "other"];
// The report type a string names, or -1: the set is closed.
n56_type :: (s:string)->int {
  for k:0..<N56_TYPES.count {if s==N56_TYPES[k] {return k;}}
  return -1;
}
// A report's subject, its type and whom and what it names.
n56_report :: (e:NostrEvent)->N56Report {
  r:N56Report;if e.error!=NE_OK {return r;}
  if e.kind!=N56_KIND_REPORT {return n56_refuse(r,N56_KIND,-1);}
  types:[3]int;   // each is set with its tag
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n56_refuse(r,N56_LIMIT,i);}
    if t.error!=NT_OK {return n56_refuse(r,N56_EVENT,i);}
    s:=ifx n32_is(t.v[0],"p") then N56_SUBJECT_P else ifx n32_is(t.v[0],"e") then N56_SUBJECT_E else ifx n32_is(t.v[0],"x") then N56_SUBJECT_X else -1;
    if s>=0 {
      key:[32]u8;hex:=t.v[1];
      if !ne_lower_hex(cast(string)hex.b[0..<hex.n],key[..]) {return n56_refuse(r,N56_TAG,i);}
      ty:= -1;
      if t.v[2].n>0 {
        mark:=t.v[2];ty=n56_type(cast(string)mark.b[0..<mark.n]);
        if ty<0 {return n56_refuse(r,N56_TYPE,i);}
      }
      if s==N56_SUBJECT_P && !r.has_pubkey {r.has_pubkey=true;r.pubkey=key;types[s]=ty;}
      if s==N56_SUBJECT_E && !r.has_event {r.has_event=true;r.event=key;types[s]=ty;}
      if s==N56_SUBJECT_X && !r.has_blob {r.has_blob=true;r.blob=key;types[s]=ty;}
    }
    i+=1;
  }
  if r.has_blob && !r.has_event {return n56_refuse(r,N56_BLOB,-1);}
  if !r.has_pubkey && !r.has_blob {return n56_refuse(r,N56_MISSING,-1);}
  r.subject=ifx r.has_blob then N56_SUBJECT_X else ifx r.has_event then N56_SUBJECT_E else N56_SUBJECT_P;
  r.type=types[r.subject];
  if r.type<0 {return n56_refuse(r,N56_UNTYPED,-1);}
  r.status=N56_OK;return r;
}
```

A report can also carry NIP-32 `L` and `l` tags to qualify it. They are
self-labels on the report, and `n32_labels` reads them.

## Counting only whom you trust

NIP-56 warns that reports are easily gamed: anyone can make keys and sign
a thousand reports. It suggests that a client act on reports from people
you know, for example by blurring a profile that three or more of your
friends report for nudity. The same holds for labels.

So the counting procedures take the trusted signers as an argument. There
is no global list in the library, and no default: an empty list trusts no
one. A statement from anyone else is counted as ignored and never by its
value, so a client can show "3 reports from people you don't follow"
without acting on them.

### Tallying reports

`n56_tally` counts, for each type, the trusted reporters who reported the
target. A report is about the target when its `p` key, `e` id or `x` hash
equals it. One reporter counts once per type: a second spam report from the
same person is a duplicate.

<!-- source: coax/lib/nostr/nip56.coax tally -->
```jai
// Whether the report names target as its user, note or blob.
n56_about :: (r:N56Report,target:[32]u8)->bool {
  return (r.has_pubkey && ns_equal32(r.pubkey,target)) || (r.has_event && ns_equal32(r.event,target)) || (r.has_blob && ns_equal32(r.blob,target));
}
// The reports about target from the trusted reporters in set, by type: one
// per reporter and type, so a reporter who reports twice counts once.
// Reports by anyone else are ignored. Other kinds are passed over.
n56_tally :: (set:[]NostrEvent,trusted:[][32]u8,target:[32]u8)->N56Tally {
  t:N56Tally;
  for i:0..<set.count {
    if set[i].kind!=N56_KIND_REPORT {continue;}
    r:=n56_report(set[i]);
    if r.status!=N56_OK {t.refused+=1;continue;}
    if !n56_about(r,target) {continue;}
    if !n32_trusted(trusted,set[i].public_key) {t.ignored+=1;continue;}
    seen:=false;
    for k:0..<i {
      if !ns_equal32(set[k].public_key,set[i].public_key) || set[k].kind!=N56_KIND_REPORT {continue;}
      q:=n56_report(set[k]);
      if q.status==N56_OK && q.type==r.type && n56_about(q,target) {seen=true;}
    }
    if seen {t.duplicates+=1;continue;}
    t.counts[r.type]+=1;
  }
  return t;
}
```

### Worked example: three reports about one note

Dave posts note N. Alice trusts Bob and Carol. Three reports name N:

| Report | Reporter | Tags | Trusted |
| --- | --- | --- | --- |
| R1 | Bob | `["e", N, "spam"]`, `["p", Dave]` | yes |
| R2 | Carol | `["e", N, "spam"]`, `["p", Dave]` | yes |
| R3 | Mallory | `["e", N, "illegal"]`, `["p", Dave]` | no |

1. R1 reads: kind 1984, a `p` tag, and its subject is the `e` tag (there is
   no `x` tag). The `e` tag's type is `spam`, which is in the set. Its `e`
   id is N, so it is about N. Bob is trusted, and no earlier report by Bob
   has type `spam`. `counts[spam]` becomes 1.
2. R2 reads the same way. Carol is trusted and has no earlier spam report.
   `counts[spam]` becomes 2.
3. R3 reads, with type `illegal`, and is about N. Mallory is not trusted,
   so `ignored` becomes 1 and `counts[illegal]` stays 0.

The result: spam 2, illegal 0, ignored 1, duplicates 0, refused 0. If Alice
also trusted Mallory, illegal would be 1. All three reports also name
Dave's key in their `p` tags, so a tally about Dave gives the same counts.

### Grouping labels

`n32_group` collects the labels about a target from trusted signers. Each
entry is one namespace, signer and value. The entries are sorted by
namespace, then signer, then value, so each namespace's labels sit
together, signer by signer. A self-label is about the event that carries
it, and its signer is that event's author.

<!-- source: coax/lib/nostr/nip32.coax entry -->
```jai
// One label about the target: its namespace, its signer and its value.
N32Entry :: struct {ns:N32Text; signer:[32]u8; value:N32Text;}
N32Tally :: struct {
  status:int=N32_OK; n:int;   // entries written, sorted by namespace, signer, value
  ignored:int;                // events about the target by signers not trusted
  duplicates:int;             // a label a trusted signer already gave
  refused:int;                // events whose labels don't read
}
```

<!-- source: coax/lib/nostr/nip32.coax group -->
```jai
// Whether key is one of the signers the caller trusts.
n32_trusted :: (trusted:[][32]u8,key:[32]u8)->bool {
  for k:0..<trusted.count {if ns_equal32(trusted[k],key) {return true;}}
  return false;
}
// Whether the labels l, read from e, are about target: kind 1985 names it
// in an e or p tag; a self-label is about the event itself.
n32_about :: (e:NostrEvent,l:N32Labels,target:[32]u8)->bool {
  if l.self {return ns_equal32(e.id,target);}
  for k:0..<l.targets_n {if ns_equal32(l.targets[k].key,target) {return true;}}
  return false;
}
// The labels about target from the trusted signers in set, one entry per
// namespace, signer and value, sorted in that order so each namespace's
// labels sit together, signer by signer. Labels by anyone else are ignored.
n32_group :: (set:[]NostrEvent,trusted:[][32]u8,target:[32]u8,out:[]N32Entry)->N32Tally {
  t:N32Tally;
  for i:0..<set.count {
    l:=n32_labels(set[i]);
    if l.status!=N32_OK {t.refused+=1;continue;}
    if l.n==0 || !n32_about(set[i],l,target) {continue;}
    if !n32_trusted(trusted,set[i].public_key) {t.ignored+=1;continue;}
    for k:0..<l.n {
      x:N32Entry;x.ns=l.labels[k].ns;x.signer=set[i].public_key;x.value=l.labels[k].value;
      at:=0;
      while at<t.n && n32_entry_order(out[at],x)<0 {at+=1;}
      if at<t.n && n32_entry_order(out[at],x)==0 {t.duplicates+=1;continue;}
      if t.n==out.count {t.status=N32_LIMIT;return t;}
      m:=t.n;
      while m>at {out[m]=out[m-1];m-=1;}
      out[at]=x;t.n+=1;
    }
  }
  return t;
}
// How many trusted signers gave this label: entries are one per signer.
n32_signers :: (entries:[]N32Entry,ns:string,value:string)->int {
  c:=0;
  for k:0..<entries.count {if n32_is(entries[k].ns,ns) && n32_is(entries[k].value,value) {c+=1;}}
  return c;
}
```

The caller supplies `out`. When it is full and another entry arrives,
`n32_group` stops with `N32_LIMIT` and `n` entries written.

### Worked example: the labels about N

Besides the reports, four kind 1985 events label N, and N labels itself:

| Event | Signer | Label | Trusted |
| --- | --- | --- | --- |
| N | Dave | `ISO-639-1` `en` (self-label) | no |
| L1 | Bob | `social.nos.ontology` `NS-spm` | yes |
| L2 | Carol | `social.nos.ontology` `NS-spm` | yes |
| L3 | Mallory | `social.nos.ontology` `NS-hat` | no |
| L4 | Bob | `ISO-639-1` `en` | yes |

1. N and L3 are by signers Alice doesn't trust: `ignored` is 2.
2. Three entries remain: (`social.nos.ontology`, Bob, `NS-spm`),
   (`social.nos.ontology`, Carol, `NS-spm`) and (`ISO-639-1`, Bob, `en`).
3. Sort by namespace: `I` is byte `0x49` and `s` is `0x73`, so `ISO-639-1`
   comes first.
4. Within `social.nos.ontology`, sort by signer key: Carol's key starts
   `dd30` and Bob's `dff1`, so Carol comes first.

The result, in order: `ISO-639-1` / Bob / `en`, `social.nos.ontology` /
Carol / `NS-spm`, `social.nos.ontology` / Bob / `NS-spm`.
`n32_signers(out, "social.nos.ontology", "NS-spm")` counts 2.

## Trusted assertions

Some numbers, such as a follower count or a web-of-trust rank, need more
events and more computing than a phone has. NIP-85 lets a service provider
compute them and publish the results as signed, addressable events. The `d`
tag names the subject, and the kind says what the subject is:

| Kind | Subject | `d` tag |
| --- | --- | --- |
| 30382 | a user | the pubkey, 64 hex digits |
| 30383 | an event | the event id, 64 hex digits |
| 30384 | an addressable event | its address, `kind:pubkey:d` |
| 30385 | a NIP-73 identifier | the identifier |

The results are tags, such as `["rank", "89"]` or `["followers", "1200"]`.

`n85_assertion` checks the kind and reads the first `d` tag. It refuses
with `N85_MISSING` when there is no `d` tag or it is empty, and with
`N85_SUBJECT` when the subject doesn't fit the kind: a pubkey or id that
isn't 64 lowercase hex digits, or an address that chapter 22's
`nt_read_address` refuses.

<!-- source: coax/lib/nostr/nip85.coax record -->
```jai
N85Provider :: struct {kind:int; result:N32Text; key:[32]u8; relay:N32Text;}
N85Providers :: struct {
  status:int=N85_EVENT; at:int=-1;            // the refusing tag
  list:[N85_MAX_PROVIDERS]N85Provider; n:int;
}
N85Assertion :: struct {
  status:int=N85_EVENT; at:int=-1;
  kind:int; subject:N32Text;                  // the d tag
  provider:[32]u8;                            // the signer
}
N85Result :: struct {
  status:int=N85_EVENT;                       // the assertion's status
  present:bool; value:N32Text;                // the first tag of the result's name
  is_number:bool; number:int;                 // the value as an integer
  trusted:bool;                               // the user named this provider for it
}
```

<!-- source: coax/lib/nostr/nip85.coax assertion -->
```jai
// An assertion's kind, subject and provider. The subject is the first d
// tag's value, read by the kind: a pubkey, an event id, an address, or any
// NIP-73 identifier.
n85_assertion :: (e:NostrEvent)->N85Assertion {
  a:N85Assertion;if e.error!=NE_OK {return a;}
  if e.kind<N85_USER || e.kind>N85_EXTERNAL {return n85_refuse(a,N85_KIND,-1);}
  a.kind=e.kind;a.provider=e.public_key;
  i:=0;at:= -1;
  while at<0 {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n85_refuse(a,N85_LIMIT,i);}
    if t.error!=NT_OK {return n85_refuse(a,N85_EVENT,i);}
    if n32_is(t.v[0],"d") {at=i;a.subject=t.v[1];}
    i+=1;
  }
  if a.subject.n==0 {return n85_refuse(a,N85_MISSING,-1);}
  s:=a.subject;text:=cast(string)s.b[0..<s.n];
  key:[32]u8;ok:=true;
  if e.kind==N85_USER || e.kind==N85_NOTE {ok=ne_lower_hex(text,key[..]);}
  if e.kind==N85_ADDRESS {probe:NostrTag;probe.error=NT_OK;ok=nt_read_address(probe,text).error==NT_OK;}
  if !ok {return n85_refuse(a,N85_SUBJECT,at);}
  a.status=N85_OK;return a;
}
```

### Naming the providers you trust

Anyone can sign a kind 30382 with any rank. A user says whose assertions
they trust in a kind 10040 event. Each tag names a kind and result, the
provider's key and a relay hint:

```text
["30382:rank", "4fd5e210...fe5f12fe", "wss://nip85.nostr.band"]
```

`n85_providers` reads these tags. A tag whose name has no colon is passed
over. A tag whose name has a colon must have a kind from 30382 to 30385, a
result name after the colon, a 64-digit key and, when present, a `ws://` or
`wss://` relay. Otherwise the list refuses with `N85_TAG` at that tag.

<!-- source: coax/lib/nostr/nip85.coax providers -->
```jai
// The providers a user names in kind 10040: each "kind:result" tag gives
// the provider's key and a relay hint. Tags of other names are passed over.
n85_providers :: (e:NostrEvent)->N85Providers {
  p:N85Providers;if e.error!=NE_OK {return p;}
  if e.kind!=N85_KIND_PROVIDERS {p.status=N85_KIND;return p;}
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error!=NT_OK {p.status=ifx t.error==NT_LIMIT then N85_LIMIT else N85_EVENT;p.at=i;return p;}
    name:=t.v[0];colon:= -1;
    for c:0..<name.n {if name.b[c]==':' {colon=c;break;}}
    if colon>=0 {
      x:N85Provider;key:=t.v[1];relay:=t.v[2];
      kind:=json_int(cast(string)name.b[0..<colon]);
      ok:=kind.ok && kind.value>=N85_USER && kind.value<=N85_EXTERNAL && colon+1<name.n;
      ok=ok && ne_lower_hex(cast(string)key.b[0..<key.n],x.key[..]);
      ok=ok && (relay.n==0 || nt_relay_ok(cast(string)relay.b[0..<relay.n]));
      if !ok {p.status=N85_TAG;p.at=i;return p;}
      if p.n==N85_MAX_PROVIDERS {p.status=N85_LIMIT;p.at=i;return p;}
      x.kind=kind.value;x.relay=relay;
      for c:colon+1..<name.n {x.result.b[c-colon-1]=name.b[c];}
      x.result.n=name.n-colon-1;
      p.list[p.n]=x;p.n+=1;
    }
    i+=1;
  }
  p.status=N85_OK;return p;
}
```

### Trusted or not

`n85_result` reads one result and sets `trusted` only when the user's list
names the assertion's signer for that kind and that result. A provider
that the user named for `30382:rank` is not trusted for
`30382:followers`, or for `30383:rank`. A provider list that doesn't read
trusts no one.

<!-- source: coax/lib/nostr/nip85.coax result -->
```jai
// The result named name in an assertion, and whether the user's providers
// name its signer for this kind and result. A provider list that doesn't
// read trusts nobody.
n85_result :: (e:NostrEvent,providers:N85Providers,name:string)->N85Result {
  r:N85Result;a:=n85_assertion(e);r.status=a.status;
  if a.status!=N85_OK || name=="d" {return r;}
  i:=0;
  while !r.present {
    t:=n32_tag(e,i);
    if t.error!=NT_OK {break;}
    if n32_is(t.v[0],name) {
      r.present=true;r.value=t.v[1];
      v:=t.v[1];n:=json_int(cast(string)v.b[0..<v.n]);r.is_number=n.ok;r.number=n.value;
    }
    i+=1;
  }
  if providers.status!=N85_OK {return r;}
  for k:0..<providers.n {
    p:=providers.list[k];
    if p.kind==e.kind && n32_is(p.result,name) && ns_equal32(p.key,a.provider) {r.trusted=true;}
  }
  return r;
}
```

The provider list doesn't record who signed it. The caller passes the
user's own kind 10040, after verifying it.

### Worked example: Dave's rank

Alice's kind 10040 (P) has one tag:
`["30382:rank", Erin's key, "wss://relay.example.com"]`.

1. Erin signs A1, a kind 30382 with `d` Dave's key, `rank` 12 and
   `reports_cnt_recd` 3. `n85_result(A1, P, "rank")` finds kind 30382,
   result `rank` and Erin's key in P: trusted.
2. `n85_result(A1, P, "reports_cnt_recd")` reads 3, but P names no one for
   `30382:reports_cnt_recd`: not trusted.
3. Mallory signs A2 with `rank` 100. P doesn't name Mallory: the value
   reads, and it is not trusted.

### Not built

Kind 10040 may also carry a private provider list: the same tags as a JSON
string, encrypted to the user with NIP-44 in the content. This chapter
doesn't decrypt it. A caller that decrypts it with chapter 20's code can
pass the decrypted tags as an event's tags. NIP-85's appendix on finding
providers through their kind 0 profiles is not built either.

## The example

The example is
[nostr/examples/labels-reports.coax](../examples/labels-reports.coax). It
signs 11 events with published test keys: BIP-340 test vectors 0 to 3 as
Alice, Bob, Carol and Mallory, and the secret keys 1 and 2 as Dave and
Erin. Dave posts N, which labels itself as English. Bob, Carol and Mallory
report and label N, Alice names Erin as her rank provider, and Erin and
Mallory rank Dave. Then it reads every event back, tallies the reports,
groups the labels and checks the assertions, trusting Bob and Carol and
Alice's provider list. The refusals use unsigned events, because no signer
here would write them.

<!-- source: nostr/examples/labels-reports.coax example -->
```jai
#load "../../coax/lib/nostr/nip56.coax";
#load "../../coax/lib/nostr/nip85.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

LABEL :: [8] string.["ok", "event", "tag", "shape", "no matching L", "label on nothing", "no label", "limit"];
REPORT :: [9] string.["ok", "event", "kind", "tag", "missing p", "unknown report type", "no report type", "x without e", "limit"];
SUBJECT :: [3] string.["p", "e", "x"];
ASSERTION :: [7] string.["ok", "event", "kind", "bad provider tag", "missing d", "subject", "limit"];
// BIP-340 test vectors 0 to 3's public keys, then the public keys of the
// secret keys 1 and 2.
PEOPLE :: [6] string.["Alice", "Bob", "Carol", "Mallory", "Dave", "Erin"];
PUB :: [6] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8",
  "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517",
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"];
// Their secret keys: published test values. Never sign with a real key in
// an example.
SECRET :: [6] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9",
  "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710",
  "0000000000000000000000000000000000000000000000000000000000000001",
  "0000000000000000000000000000000000000000000000000000000000000002"];
ALICE :: 0; BOB :: 1; CAROL :: 2; MALLORY :: 3; DAVE :: 4; ERIN :: 5;
RELAY :: "wss://relay.example.com";
T0 :: 1761686400;

// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
hex32 :: (id: [32] u8, out: [] u8) -> string {
  own := id; r := hex_encode_checked(cast(string) own[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<64];
}
key_of :: (who: int) -> [32] u8 { k: [32] u8; decode_hex(PUB[who], k[..]); return k; }

// The events, by name, so the output names ids and keys by name.
Named :: struct { name: string; event: NostrEvent; }
EVENTS :: 11;
show_id :: (all: [] Named, id: [32] u8) {
  for n: all { if n.event.error == NE_OK && ns_equal32(n.event.id, id) { print("%", n.name); return; } }
  for k: 0..<PEOPLE.count { if ns_equal32(key_of(k), id) { print("%", PEOPLE[k]); return; } }
  text: [64] u8; print("%...", hex32(id, text[..])[0..<8]);
}
say :: (t: N32Text) { own := t; print("%", cast(string) own.b[0..<own.n]); }

// Sign an event into out and read it back with its ID and signature checked.
sign :: (who: int, created_at: int, kind: int, tags: string, content: string, out: [] u8) -> NostrEvent {
  key: [32] u8; decode_hex(SECRET[who], key[..]);
  aux: [32] u8; work: [1] NostrSignWork; scratch: [8192] u8; n := EVENT_MORE;
  while n == EVENT_MORE {
    n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false);
  }
  #assert n > 0;
  check: [8192] u8;
  e := nostr_event_verify_vartime(cast(string) out[0..<n], check[..], false);
  #assert e.error == NE_OK && e.verified;
  return e;
}
// The tags with each @ replaced by the note N's id and each % by Dave's key.
with_note :: (template: string, note: NostrEvent, out: [] u8) -> string {
  n := 0; i := 0; hex: [64] u8;
  while i < template.count {
    if template[i] == '@' { n = json_raw(out, n, hex32(note.id, hex[..])); i += 1; continue; }
    if template[i] == '%' { n = json_raw(out, n, PUB[DAVE]); i += 1; continue; }
    n = json_raw(out, n, template[i..<i + 1]); i += 1;
  }
  #assert n > 0;
  return cast(string) out[0..<n];
}

show_report :: (all: [] Named, label: string, e: NostrEvent) {
  r := n56_report(e);
  print("%: %", label, REPORT[r.status]);
  if r.at >= 0 { print(" at tag %", r.at); }
  if r.status != N56_OK { print("\n"); return; }
  print(", % about % ", N56_TYPES[r.type], SUBJECT[r.subject]);
  if r.subject == N56_SUBJECT_E { show_id(all, r.event); } else if r.subject == N56_SUBJECT_P { show_id(all, r.pubkey); }
  if r.has_pubkey && r.subject != N56_SUBJECT_P { print(", user "); show_id(all, r.pubkey); }
  print(", by "); show_id(all, e.public_key); print("\n");
}
show_labels :: (all: [] Named, label: string, e: NostrEvent) {
  l := n32_labels(e);
  print("%: %", label, LABEL[l.status]);
  if l.at >= 0 { print(" at tag %", l.at); }
  if l.status != N32_OK { print("\n"); return; }
  for k: 0..<l.n { print(", "); say(l.labels[k].ns); print(" "); say(l.labels[k].value); }
  if l.self { print(", about itself"); }
  for k: 0..<l.targets_n { print(", about "); show_id(all, l.targets[k].key); }
  print(", by "); show_id(all, e.public_key); print("\n");
}
show_result :: (all: [] Named, label: string, e: NostrEvent, providers: N85Providers, name: string) {
  r := n85_result(e, providers, name);
  print("%: %", label, ASSERTION[r.status]);
  if r.status != N85_OK { print("\n"); return; }
  a := n85_assertion(e); s := a.subject; subject: [32] u8; decode_hex(cast(string) s.b[0..<s.n], subject[..]);
  print(", kind %, about ", a.kind); show_id(all, subject);
  print(", % ", name); if r.present { say(r.value); } else { print("absent"); }
  print(", by "); show_id(all, a.provider);
  print(", trusted %\n", r.trusted);
}

// An unsigned event by Mallory with these tags, parsed but not verified:
// its id and sig are zeros. Used for the refusals, which no signer here
// would write.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[MALLORY]); n = json_raw(out, n, "\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

// The events: who signs, when (after T0), the kind, the tags with @ for
// N's id and % for Dave's key, and the content.
NAME :: [EVENTS] string.["N", "R1", "R2", "R3", "L1", "L2", "L3", "L4", "P", "A1", "A2"];
WHO :: [EVENTS] int.[DAVE, BOB, CAROL, MALLORY, BOB, CAROL, MALLORY, BOB, ALICE, ERIN, MALLORY];
AT :: [EVENTS] int.[0, 60, 120, 180, 240, 300, 360, 420, 480, 540, 600];
KIND :: [EVENTS] int.[1, 1984, 1984, 1984, 1985, 1985, 1985, 1985, 10040, 30382, 30382];
TAGS :: [EVENTS] string.[
  "[[\"L\",\"ISO-639-1\"],[\"l\",\"en\",\"ISO-639-1\"]]",
  "[[\"e\",\"@\",\"spam\"],[\"p\",\"%\"]]",
  "[[\"e\",\"@\",\"spam\"],[\"p\",\"%\"]]",
  "[[\"e\",\"@\",\"illegal\"],[\"p\",\"%\"]]",
  "[[\"L\",\"social.nos.ontology\"],[\"l\",\"NS-spm\",\"social.nos.ontology\"],[\"e\",\"@\",\"wss://relay.example.com\"]]",
  "[[\"L\",\"social.nos.ontology\"],[\"l\",\"NS-spm\",\"social.nos.ontology\"],[\"e\",\"@\",\"wss://relay.example.com\"]]",
  "[[\"L\",\"social.nos.ontology\"],[\"l\",\"NS-hat\",\"social.nos.ontology\"],[\"e\",\"@\",\"wss://relay.example.com\"]]",
  "[[\"L\",\"ISO-639-1\"],[\"l\",\"en\",\"ISO-639-1\"],[\"e\",\"@\",\"wss://relay.example.com\"]]",
  "[[\"30382:rank\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"wss://relay.example.com\"]]",
  "[[\"d\",\"%\"],[\"rank\",\"12\"],[\"reports_cnt_recd\",\"3\"]]",
  "[[\"d\",\"%\"],[\"rank\",\"100\"]]"];
CONTENT :: [EVENTS] string.["Buy followers now at example.com!", "", "Same link in 40 replies.", "", "", "", "", "", "", "", ""];

main :: () {
  all: [EVENTS] Named; buf: [EVENTS][8192] u8; tags: [4096] u8;
  for k: 0..<EVENTS {
    all[k].name = NAME[k];
    all[k].event = sign(WHO[k], T0 + AT[k], KIND[k], with_note(TAGS[k], all[0].event, tags[..]), CONTENT[k], buf[k][..]);
  }
  print("each event's id\n");
  for e: all { hex: [64] u8; print("%: %, verified %\n", e.name, hex32(e.event.id, hex[..]), e.event.verified); }

  print("reports about N\n");
  for k: 1..<4 { show_report(all[..], all[k].name, all[k].event); }
  set: [EVENTS] NostrEvent; for k: 0..<EVENTS { set[k] = all[k].event; }
  trusted: [2][32] u8; trusted[0] = key_of(BOB); trusted[1] = key_of(CAROL);
  print("the tally for N, trusting Bob and Carol\n");
  t := n56_tally(set[..], trusted[..], all[0].event.id);
  for k: 0..<7 { if t.counts[k] > 0 { print("% %, ", N56_TYPES[k], t.counts[k]); } }
  print("illegal %, ignored %, duplicates %, refused %\n", t.counts[N56_ILLEGAL], t.ignored, t.duplicates, t.refused);

  print("labels about N\n");
  show_labels(all[..], "N", all[0].event);
  for k: 4..<8 { show_labels(all[..], all[k].name, all[k].event); }
  print("grouped by namespace and signer, trusting Bob and Carol\n");
  out: [8] N32Entry;
  g := n32_group(set[..], trusted[..], all[0].event.id, out[..]);
  for k: 0..<g.n { say(out[k].ns); print(" / "); show_id(all[..], out[k].signer); print(" / "); say(out[k].value); print("\n"); }
  print("entries %, ignored %, duplicates %, refused %\n", g.n, g.ignored, g.duplicates, g.refused);
  print("social.nos.ontology NS-spm from % trusted signers\n", n32_signers(out[0..<g.n], "social.nos.ontology", "NS-spm"));

  print("trusted assertions about Dave\n");
  providers := n85_providers(all[8].event);
  print("P: %, % provider: % ", ASSERTION[providers.status], providers.n, providers.list[0].kind);
  say(providers.list[0].result); print(", "); show_id(all[..], providers.list[0].key); print("\n");
  show_result(all[..], "A1", all[9].event, providers, "rank");
  show_result(all[..], "A1", all[9].event, providers, "reports_cnt_recd");
  show_result(all[..], "A2", all[10].event, providers, "rank");

  print("refusals\n");
  bad: [4096] u8;
  show_report(all[..], "report of type scam", unsigned(1984, with_note("[[\"e\",\"@\",\"scam\"],[\"p\",\"%\"]]", all[0].event, tags[..]), bad[..]));
  show_report(all[..], "report without p", unsigned(1984, with_note("[[\"e\",\"@\",\"spam\"]]", all[0].event, tags[..]), bad[..]));
  show_report(all[..], "report without a type", unsigned(1984, with_note("[[\"e\",\"@\"],[\"p\",\"%\"]]", all[0].event, tags[..]), bad[..]));
  show_report(all[..], "blob report without e", unsigned(1984, "[[\"x\",\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",\"malware\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\"]]", bad[..]));
  show_labels(all[..], "l without its L", unsigned(1985, with_note("[[\"L\",\"social.nos.ontology\"],[\"l\",\"NS-spm\",\"nostr.mod\"],[\"e\",\"@\"]]", all[0].event, tags[..]), bad[..]));
  show_labels(all[..], "label on nothing", unsigned(1985, "[[\"L\",\"social.nos.ontology\"],[\"l\",\"NS-spm\",\"social.nos.ontology\"]]", bad[..]));
  show_labels(all[..], "self-label without its L", unsigned(1, "[[\"l\",\"en\",\"ISO-639-1\"]]", bad[..]));
  p := n85_providers(unsigned(10040, "[[\"30382:rank\",\"c6047f94\"]]", bad[..]));
  print("provider list with a short key: % at tag %\n", ASSERTION[p.status], p.at);
  show_result(all[..], "assertion without d", unsigned(30382, "[[\"rank\",\"100\"]]", bad[..]), providers, "rank");
}
```

Run it with:

```shell
coax/bin/coax run nostr/examples/labels-reports.coax --run-steps 250000000
```

It prints this transcript, which `nostr/fixtures/labels-reports.out` holds
and the book's checks compare in debug and release. It exits with status 0.

```text
each event's id
N: a74fb478fce08ec9047e0e78edf913e7795fd32ea9b9163e5a8e1141d218ecd4, verified true
R1: a8d3832c382553d85dbd9727463fdb1ee09c6041d608e2673637fc83672471d8, verified true
R2: 5931a73187a06aa48e8da03f14a3db0302615075226477a745d47d3440277acd, verified true
R3: 9ae5f873b96895d5635867f06096515f78ad79852199ad6716a3e8c2104cee6e, verified true
L1: c83ecc6c363509cc1059fd3d1c437113c01ddd085acce5036e29e3ebe7659d8a, verified true
L2: 29f7c0ea938d8b448fccdb5e58b28d8a4e56a108db79a896e5b01f5c5091be65, verified true
L3: a1a2b80c611dc0b06366109df595296b77fea8c36bdb224840426e9b3df01907, verified true
L4: 2685e2342412b4e6b50588210ff7f8e740689125c5e1b27b63d4e82aeace0d1c, verified true
P: fce1d7ba776d6b29473334903324c613c193fa8e60dc114717eb3ad67213b856, verified true
A1: ccedac91cfec707ba2c0497a5ba6052e1037a936717b2f063364c76be46d06aa, verified true
A2: 2169377eee55425e9668627f51d8f6a106366b25e88f68697314c8e6c3e59ddb, verified true
reports about N
R1: ok, spam about e N, user Dave, by Bob
R2: ok, spam about e N, user Dave, by Carol
R3: ok, illegal about e N, user Dave, by Mallory
the tally for N, trusting Bob and Carol
spam 2, illegal 0, ignored 1, duplicates 0, refused 0
labels about N
N: ok, ISO-639-1 en, about itself, by Dave
L1: ok, social.nos.ontology NS-spm, about N, by Bob
L2: ok, social.nos.ontology NS-spm, about N, by Carol
L3: ok, social.nos.ontology NS-hat, about N, by Mallory
L4: ok, ISO-639-1 en, about N, by Bob
grouped by namespace and signer, trusting Bob and Carol
ISO-639-1 / Bob / en
social.nos.ontology / Carol / NS-spm
social.nos.ontology / Bob / NS-spm
entries 3, ignored 2, duplicates 0, refused 0
social.nos.ontology NS-spm from 2 trusted signers
trusted assertions about Dave
P: ok, 1 provider: 30382 rank, Erin
A1: ok, kind 30382, about Dave, rank 12, by Erin, trusted true
A1: ok, kind 30382, about Dave, reports_cnt_recd 3, by Erin, trusted false
A2: ok, kind 30382, about Dave, rank 100, by Mallory, trusted false
refusals
report of type scam: unknown report type at tag 0
report without p: missing p
report without a type: no report type
blob report without e: x without e
l without its L: no matching L at tag 1
label on nothing: label on nothing
self-label without its L: no matching L at tag 0
provider list with a short key: bad provider tag at tag 0
assertion without d: missing d
```

The tally and the grouping match the worked examples. The refusals show
each broken input and its exact diagnostic:

- `report of type scam`: the `e` tag's type `scam` is not one of the seven,
  at tag 0.
- `report without p`: an `e` tag and no `p` or `x` tag.
- `report without a type`: the subject `e` tag has no third entry.
- `blob report without e`: an `x` tag and a `p` tag, but no `e` tag.
- `l without its L`: the label is marked `nostr.mod`, and the only `L` tag
  is `social.nos.ontology`, at tag 1.
- `label on nothing`: a kind 1985 with a label and no target tag.
- `self-label without its L`: a kind 1 marked `ISO-639-1` with no `L` tag.
- `provider list with a short key`: the key is 8 hex digits, at tag 0.
- `assertion without d`: a kind 30382 that names no subject.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/labels-reports.coax](../examples/checks/labels-reports.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The ids of all 11 events, from a Python script written from NIP-01,
  NIP-32, NIP-56 and NIP-85 (`hashlib.sha256` over NIP-01's
  serialization). The checks read each event with Python's id and require
  that the recomputed id matches.
- The report tally of N by hand: spam 2, illegal 0, ignored 1. Also in
  reverse order, trusting Mallory as well (illegal 1), trusting no one
  (ignored 3), and about Dave's key.
- The label grouping of N by hand, in the order worked out above, and
  trusting only Dave (his self-label alone).
- The assertions: Erin's rank trusted, her `reports_cnt_recd` and
  Mallory's rank not.
- NIP-32's six examples, NIP-56's four examples and NIP-85's two examples,
  with their placeholders filled, each read as the NIP describes.

<!-- source: nostr/examples/checks/labels-reports.coax check -->
```jai
// The ids from a Python script written from NIP-01, NIP-32, NIP-56 and
// NIP-85 (it shares no code with Coax): hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False), over the same events.
PY_ID :: [EVENTS] string.[
  "a74fb478fce08ec9047e0e78edf913e7795fd32ea9b9163e5a8e1141d218ecd4",
  "a8d3832c382553d85dbd9727463fdb1ee09c6041d608e2673637fc83672471d8",
  "5931a73187a06aa48e8da03f14a3db0302615075226477a745d47d3440277acd",
  "9ae5f873b96895d5635867f06096515f78ad79852199ad6716a3e8c2104cee6e",
  "c83ecc6c363509cc1059fd3d1c437113c01ddd085acce5036e29e3ebe7659d8a",
  "29f7c0ea938d8b448fccdb5e58b28d8a4e56a108db79a896e5b01f5c5091be65",
  "a1a2b80c611dc0b06366109df595296b77fea8c36bdb224840426e9b3df01907",
  "2685e2342412b4e6b50588210ff7f8e740689125c5e1b27b63d4e82aeace0d1c",
  "fce1d7ba776d6b29473334903324c613c193fa8e60dc114717eb3ad67213b856",
  "ccedac91cfec707ba2c0497a5ba6052e1037a936717b2f063364c76be46d06aa",
  "2169377eee55425e9668627f51d8f6a106366b25e88f68697314c8e6c3e59ddb"];
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

// Event k with Python's id and a zero signature, read with its id
// recomputed; the signature is not checked.
py :: (k: int, note: NostrEvent, out: [] u8, scratch: [] u8) -> NostrEvent {
  tags: [1024] u8;
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[k]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[WHO[k]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + AT[k]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, KIND[k]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, with_note(TAGS[k], note, tags[..]));
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, CONTENT[k]);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}
read_all :: (ev: [] NostrEvent, buf: [][2048] u8) {
  scratch: [4096] u8;
  ev[0] = py(0, ev[0], buf[0][..], scratch[..]);
  for k: 1..<EVENTS { ev[k] = py(k, ev[0], buf[k][..], scratch[..]); }
}

#test "the ids are Python's" {
  buf: [EVENTS][2048] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..]);
  for k: 0..<EVENTS { #assert ev[k].error == NE_OK && ns_equal32(ev[k].computed_id, ev[k].id); }
}

// The tally by hand. R1 (Bob, spam) and R2 (Carol, spam) are by trusted
// reporters: spam 2. R3 (Mallory, illegal) is not: ignored 1, illegal 0.
// No one reports twice; nothing else in the set is kind 1984.
#test "the report tally of N matches the hand count, in any order and trust" {
  buf: [EVENTS][2048] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..]);
  trusted: [3][32] u8; trusted[0] = key_of(BOB); trusted[1] = key_of(CAROL); trusted[2] = key_of(MALLORY);
  t := n56_tally(ev[..], trusted[0..<2], ev[0].id);
  #assert t.counts[N56_SPAM] == 2 && t.counts[N56_ILLEGAL] == 0 && t.ignored == 1 && t.duplicates == 0 && t.refused == 0;
  for k: 0..<7 { if k != N56_SPAM { #assert t.counts[k] == 0; } }
  rev: [EVENTS] NostrEvent; for k: 0..<EVENTS { rev[k] = ev[EVENTS - 1 - k]; }
  t = n56_tally(rev[..], trusted[0..<2], ev[0].id);
  #assert t.counts[N56_SPAM] == 2 && t.ignored == 1;
  // Trust Mallory too: illegal 1. Trust no one: everything ignored.
  t = n56_tally(ev[..], trusted[..], ev[0].id); #assert t.counts[N56_SPAM] == 2 && t.counts[N56_ILLEGAL] == 1 && t.ignored == 0;
  t = n56_tally(ev[..], trusted[0..<0], ev[0].id); #assert t.counts[N56_SPAM] == 0 && t.ignored == 3;
  // The reports name Dave's key too, so his tally is the same.
  t = n56_tally(ev[..], trusted[0..<2], key_of(DAVE)); #assert t.counts[N56_SPAM] == 2 && t.ignored == 1;
}

// The grouping by hand, trusting Bob and Carol: L1 (Bob) and L2 (Carol) give
// social.nos.ontology NS-spm, L4 (Bob) ISO-639-1 en. L3 (Mallory) and N's
// self-label (Dave) are ignored. Namespaces sort "ISO-639-1" before
// "social.nos.ontology" ('I' is 0x49, 's' 0x73); within one, Carol's key
// (dd30...) before Bob's (dff1...).
#test "the label grouping of N matches the hand order" {
  buf: [EVENTS][2048] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..]);
  trusted: [2][32] u8; trusted[0] = key_of(BOB); trusted[1] = key_of(CAROL);
  out: [8] N32Entry; g := n32_group(ev[..], trusted[..], ev[0].id, out[..]);
  #assert g.status == N32_OK && g.n == 3 && g.ignored == 2 && g.duplicates == 0 && g.refused == 0;
  #assert n32_is(out[0].ns, "ISO-639-1") && n32_is(out[0].value, "en") && ns_equal32(out[0].signer, key_of(BOB));
  #assert n32_is(out[1].ns, "social.nos.ontology") && n32_is(out[1].value, "NS-spm") && ns_equal32(out[1].signer, key_of(CAROL));
  #assert n32_is(out[2].ns, "social.nos.ontology") && ns_equal32(out[2].signer, key_of(BOB));
  // Trusting Dave, his self-label on N joins the ISO-639-1 group.
  dave: [1][32] u8; dave[0] = key_of(DAVE);
  g = n32_group(ev[..], dave[..], ev[0].id, out[..]);
  #assert g.n == 1 && n32_is(out[0].ns, "ISO-639-1") && ns_equal32(out[0].signer, key_of(DAVE)) && g.ignored == 4;
}

// Alice names Erin for 30382:rank only. A1's rank is trusted, A1's
// reports_cnt_recd is not (Alice named no one for it), A2's rank is not
// (Alice did not name Mallory).
#test "assertions are trusted only from the providers Alice named" {
  buf: [EVENTS][2048] u8; ev: [EVENTS] NostrEvent; read_all(ev[..], buf[..]);
  p := n85_providers(ev[8]); #assert p.status == N85_OK && p.n == 1 && ns_equal32(p.list[0].key, key_of(ERIN));
  r := n85_result(ev[9], p, "rank"); #assert r.trusted && r.number == 12;
  r = n85_result(ev[9], p, "reports_cnt_recd"); #assert !r.trusted && r.number == 3;
  r = n85_result(ev[10], p, "rank"); #assert !r.trusted && r.number == 100;
  // The provider list carries no signer: the caller passes the user's own.
  a := n85_assertion(ev[9]); #assert a.status == N85_OK && n32_is(a.subject, PUB[DAVE]);
}

// NIP-32's six examples, placeholders filled with Dave's key, N's id and the
// relay. Each reads with the label, namespace and targets the NIP describes.
#test "NIP-32's examples read as the NIP says" {
  b: [2048] u8;
  pk := "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
  l := n32_labels(unsigned(1985, "[[\"L\",\"#t\"],[\"l\",\"permies\",\"#t\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"wss://relay.example.com\"],[\"p\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"wss://relay.example.com\"]]", b[..]));
  #assert l.status == N32_OK && l.n == 1 && n32_is(l.labels[0].ns, "#t") && n32_is(l.labels[0].value, "permies") && l.targets_n == 2;
  l = n32_labels(unsigned(1985, "[[\"L\",\"com.example.ontology\"],[\"l\",\"VI-hum\",\"com.example.ontology\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"wss://relay.example.com\"],[\"p\",\"c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5\",\"wss://relay.example.com\"]]", b[..]));
  #assert l.status == N32_OK && n32_is(l.labels[0].value, "VI-hum") && l.targets[1].key[0] == 0xc6;
  l = n32_labels(unsigned(1985, "[[\"L\",\"nip28.moderation\"],[\"l\",\"approve\",\"nip28.moderation\"],[\"e\",\"a74fb478fce08ec9047e0e78edf913e7795fd32ea9b9163e5a8e1141d218ecd4\",\"wss://relay.example.com\"]]", b[..]));
  #assert l.status == N32_OK && n32_is(l.labels[0].value, "approve") && l.targets[0].type == NT_E && l.targets[0].key[0] == 0xa7;
  l = n32_labels(unsigned(1985, "[[\"L\",\"license\"],[\"l\",\"MIT\",\"license\"],[\"e\",\"a74fb478fce08ec9047e0e78edf913e7795fd32ea9b9163e5a8e1141d218ecd4\",\"wss://relay.example.com\"]]", b[..]));
  #assert l.status == N32_OK && n32_is(l.labels[0].ns, "license") && n32_is(l.labels[0].value, "MIT");
  l = n32_labels(unsigned(1, "[[\"L\",\"ISO-3166-2\"],[\"l\",\"IT-MI\",\"ISO-3166-2\"]]", b[..]));
  #assert l.status == N32_OK && l.self && n32_is(l.labels[0].value, "IT-MI");
  l = n32_labels(unsigned(1, "[[\"L\",\"ISO-639-1\"],[\"l\",\"en\",\"ISO-639-1\"]]", b[..]));
  #assert l.status == N32_OK && l.self && n32_is(l.labels[0].ns, "ISO-639-1") && pk.count == 64;
}

// NIP-56's four examples, placeholders filled. Each reads with the type and
// subject the NIP describes; the first also carries a NIP-32 label.
#test "NIP-56's examples read as the NIP says" {
  b: [2048] u8;
  e := unsigned(1984, "[[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"nudity\"],[\"L\",\"social.nos.ontology\"],[\"l\",\"NS-nud\",\"social.nos.ontology\"]]", b[..]);
  r := n56_report(e); #assert r.status == N56_OK && r.type == N56_NUDITY && r.subject == N56_SUBJECT_P;
  l := n32_labels(e); #assert l.status == N32_OK && l.self && n32_is(l.labels[0].value, "NS-nud");
  r = n56_report(unsigned(1984, "[[\"e\",\"a74fb478fce08ec9047e0e78edf913e7795fd32ea9b9163e5a8e1141d218ecd4\",\"illegal\"],[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\"]]", b[..]));
  #assert r.status == N56_OK && r.type == N56_ILLEGAL && r.subject == N56_SUBJECT_E;
  r = n56_report(unsigned(1984, "[[\"p\",\"79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798\",\"impersonation\"]]", b[..]));
  #assert r.status == N56_OK && r.type == N56_IMPERSONATION;
  r = n56_report(unsigned(1984, "[[\"x\",\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",\"malware\"],[\"e\",\"a74fb478fce08ec9047e0e78edf913e7795fd32ea9b9163e5a8e1141d218ecd4\",\"malware\"],[\"server\",\"https://you-may-find-the-blob-here.com/path-to-url.ext\"]]", b[..]));
  #assert r.status == N56_OK && r.type == N56_MALWARE && r.subject == N56_SUBJECT_X && !r.has_pubkey;
}

// NIP-85's examples: the rank-89 assertion and the three public provider tags.
#test "NIP-85's examples read as the NIP says" {
  b: [2048] u8; c: [2048] u8;
  p := n85_providers(unsigned(10040, N85_T_LIST, b[..]));
  #assert p.status == N85_OK && p.n == 3 && n32_is(p.list[2].result, "zap_amt_sent") && n32_is(p.list[1].relay, "wss://nostr.wine");
  a := unsigned(30382, "[[\"d\",\"e88a691e98d9987c964521dff60025f60700378a4879180dcbbb4a5027850411\"],[\"rank\",\"89\"]]", c[..]);
  r := n85_result(a, p, "rank"); #assert r.status == N85_OK && r.number == 89 && !r.trusted;   // signed by Mallory here
}
```

```shell
coax/bin/coax test nostr/examples/checks/labels-reports.coax --run-steps 250000000
```

`coax test` runs 367 checks, including the loaded files' own checks; all
pass.

### The library's own checks

`nip32.coax`, `nip56.coax` and `nip85.coax` have their own `#test` blocks:
each reading and refusal and the tag index it reports, the namespace rules
with and without `L` tags, every bound at its edge, forged tag arrays, the
first-of-each-tag rules, the ordering of entries, duplicates and other
targets in the tally and the grouping, and trust by kind, result and key.

```shell
coax/bin/coax test coax/lib/nostr/nip32.coax
coax/bin/coax test coax/lib/nostr/nip56.coax
coax/bin/coax test coax/lib/nostr/nip85.coax
coax/bin/coax mutate coax/lib/nostr/nip32.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip56.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip85.coax --root-only
```

On 2026-11-01, `coax test` passed all 202, 207 and 206 checks (with the
loaded files'). `coax mutate --root-only` killed all 289 mutants of
`nip32.coax`, all 141 of `nip56.coax` and all 184 of `nip85.coax`, with
none surviving and none marked equivalent. The full mutation run
(`COAX_MUTATION_FULL=1`) and a mutation run of the example were not run.

## Summary

- `n32_labels` reads an event's labels: kind 1985 labels the targets its
  `e`, `p`, `a`, `r` and `t` tags name, any other kind labels itself. Every
  marked label needs its `L` tag, except `ugc`.
- `n56_report` reads a report's subject (`x`, else `e`, else `p`) and its
  type from the closed set of seven, and refuses any other type.
- `n56_tally` and `n32_group` count only the signers the caller trusts,
  one per signer, and count everyone else as ignored.
- `n85_result` marks an assertion's result as trusted only when the user's
  kind 10040 names its provider for that kind and result.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-32](../nips/official/32.md)
- [NIP-56](../nips/official/56.md)
- [NIP-85](../nips/official/85.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
