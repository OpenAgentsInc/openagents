# 62. Badges and statuses

Badges and statuses decorate a profile. A badge is a claim by its issuer
that a user earned it, and a user chooses which badges to show. A status
says what a user is doing now, and it can end. This chapter reads both. It
shows a badge only when its award comes from the badge's issuer and names
the profile's owner, and it shows a status only until it expires.

Part VI, Every NIP. Result: NIP-58 badge definitions (30009), awards (8) and
profile badges (10008, formerly 30008), and NIP-38 kind 30315 user statuses
with `expiration`.

## What this chapter teaches

- Kind 30009: a badge definition with `d`, `name`, `image`, `thumb` and
  `description`.
- Kind 8: an award with one `a` tag naming the definition and a `p` tag for
  each awardee. Only the definition's author can award its badge.
- Kind 10008: the ordered `a`/`e` pairs a user chooses to show, and the
  four checks that decide whether a pair is shown.
- NIP-38: kind 30315 with `d` `general` or `music`, `r` links, NIP-40
  `expiration` read against a fixed clock, and why an older status never
  comes back when the newest one expires.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nostr_tag(e, i)`, `nostr_tag_d(e)` | Reads and checks an `a`, `e`, `p` or `d` tag (chapter 22) |
| [nip32.coax](../../coax/lib/nostr/nip32.coax) | `n32_tag(e, i)` | Reads a tag of any name: the name and its first two values (chapter 55) |
| [nip58.coax](../../coax/lib/nostr/nip58.coax) | `n58_definition(e)` | A badge definition's `d`, name, image and thumbnails |
| [nip58.coax](../../coax/lib/nostr/nip58.coax) | `n58_award(e)` | An award's definition address and awardees, refused unless the issuer signed it |
| [nip58.coax](../../coax/lib/nostr/nip58.coax) | `n58_names(e, key)` | Whether an award's `p` tags name a key |
| [nip58.coax](../../coax/lib/nostr/nip58.coax) | `n58_profile(e, awards, out)` | A profile's pairs in order, each with its verdict |
| [nip38.coax](../../coax/lib/nostr/nip38.coax) | `n38_status(e, now)` | A user status read at time `now`: type, links, expiration, cleared |
| [nip38.coax](../../coax/lib/nostr/nip38.coax) | `n38_current(set, author, d, now)` | The newest status of one type by one author, read at `now` |
| [nip38.coax](../../coax/lib/nostr/nip38.coax) | `n38_shown(c)` | Whether a client shows that status |

Both library files are new with this chapter. Each is an `#authority` file:
it decides which badges and statuses a client shows, so every procedure
runs in a check and `coax mutate` leaves no survivors. Neither file fetches
an event or verifies a signature. The caller verifies the events
(chapter 21) and passes them in. The clock is the caller's too: every
status procedure takes `now` in Unix seconds, so a check or an example fixes
it.

## The records

Each reader returns a record with a status, the index of the tag that it
refused (or -1), and what it read. `N32Text` is chapter 55's decoded string
of at most 256 bytes, and `NostrTag` is chapter 22's checked tag.

<!-- source: coax/lib/nostr/nip58.coax record -->
```jai
N58Definition :: struct {
  status:int=N58_EVENT; at:int=-1;            // the refusing tag
  d:N32Text; name:N32Text;                    // the identifier and short name
  image:N32Text; image_size:N32Text;          // the first image and its WxH
  thumbs:int; thumb:N32Text;                  // thumb tags, and the first
  has_description:bool;
}
N58Award :: struct {
  status:int=N58_EVENT; at:int=-1;
  address:NostrTag;    // the single a tag: 30009, the issuer, the badge's d
  awardees:int;        // p tags
}
N58Pair :: struct {
  verdict:int;         // N58_SHOWN or why not
  at:int;              // the pair's a tag
  address:NostrTag;    // the definition the profile names
  award:[32]u8;        // the award the profile names
}
N58Profile :: struct {
  status:int=N58_EVENT; at:int=-1;
  set:bool;            // kind 30008 with a d other than profile_badges
  n:int; shown:int;    // pairs in out, and those shown
  unpaired:int;        // an a without its e, or an e without its a
  sets:int;            // a tags naming kind 30008 badge sets
  bad:int;             // a and e tags that don't read
}
```

<!-- source: coax/lib/nostr/nip38.coax record -->
```jai
N38Status :: struct {
  status:int=N38_EVENT; at:int=-1;   // the refusing tag
  type:int; d:N32Text;               // general, music or another type
  expiration:int=-1;                 // -1: none
  cleared:bool;                      // empty content: clear the status
  links:int;                         // r, p, e and a tags
  has_url:bool; url:N32Text;         // the first r tag
}
N38Current :: struct {
  index:int=-1;        // the newest status of that type in the set, or -1
  status:N38Status;    // its reading at now
  refused:int;         // the author's kind 30315 events that don't read
}
```

## Badge definitions

A badge definition is an addressable event of kind 30009, so its address is
`30009:<issuer>:<d>`. The issuer can replace it, and its address stays the
same. NIP-58 requires the `d` tag. `name`, `image`, `description` and
`thumb` are optional. The second value of `image` and `thumb` is the size,
such as `1024x1024`.

<!-- source: coax/lib/nostr/nip58.coax definition -->
```jai
// A badge definition: d is required and not empty; name, image, thumb and
// description, when present, need a value. The first of each counts.
n58_definition :: (e:NostrEvent)->N58Definition {
  r:N58Definition;if e.error!=NE_OK {return r;}
  if e.kind!=N58_KIND_DEFINITION {return n58_def_refuse(r,N58_KIND,-1);}
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n58_def_refuse(r,N58_LIMIT,i);}
    if t.error!=NT_OK {return n58_def_refuse(r,N58_EVENT,i);}
    name:=t.v[0];
    known:=n32_is(name,"d") || n32_is(name,"name") || n32_is(name,"image") || n32_is(name,"thumb") || n32_is(name,"description");
    if known && t.v[1].n==0 {return n58_def_refuse(r,N58_TAG,i);}
    if n32_is(name,"d") && r.d.n==0 {r.d=t.v[1];}
    if n32_is(name,"name") && r.name.n==0 {r.name=t.v[1];}
    if n32_is(name,"image") && r.image.n==0 {r.image=t.v[1];r.image_size=t.v[2];}
    if n32_is(name,"thumb") {if r.thumbs==0 {r.thumb=t.v[1];}r.thumbs+=1;}
    if n32_is(name,"description") {r.has_description=true;}
    i+=1;
  }
  if r.d.n==0 {return n58_def_refuse(r,N58_MISSING,-1);}
  r.status=N58_OK;return r;
}
```

The reader keeps the first `d`, `name` and `image`, counts every `thumb`
and keeps the first one. It refuses three things:

- A kind other than 30009 (`N58_KIND`).
- A `d`, `name`, `image`, `thumb` or `description` tag with no value or an
  empty value (`N58_TAG`, at that tag). An empty `d` would give every such
  badge the same address.
- No `d` tag (`N58_MISSING`).

A value longer than 256 bytes is refused with `N58_LIMIT`. This bound
comes from `n32_tag` and applies to a long `description` too.

## Badge awards

An award is a kind 8 event, a regular event that is never replaced. Its one
`a` tag names a kind 30009 definition, and it has a `p` tag for each user
who gets the badge. Anyone can sign a kind 8 event that names anyone's
badge. NIP-58 says the badge issuer publishes the award. The code
therefore compares the award's signer with the issuer in the `a` tag, the
pubkey between the two colons, and refuses the award when they differ.

<!-- source: coax/lib/nostr/nip58.coax award -->
```jai
// A badge award: one a tag naming a kind 30009 definition, at least one p
// tag, and signed by the definition's author. Anyone can sign a kind 8
// event naming anyone's badge; only the issuer's award counts.
n58_award :: (e:NostrEvent)->N58Award {
  r:N58Award;if e.error!=NE_OK {return r;}
  if e.kind!=N58_KIND_AWARD {return n58_award_refuse(r,N58_KIND,-1);}
  a_at:=-1;i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n58_award_refuse(r,N58_LIMIT,i);}
    if t.error!=NT_OK {return n58_award_refuse(r,N58_EVENT,i);}
    is_a:=n32_is(t.v[0],"a");is_p:=n32_is(t.v[0],"p");
    if is_a || is_p {
      s:=nostr_tag(e,i);
      if s.error!=NT_OK {return n58_award_refuse(r,N58_TAG,i);}
      if is_p {r.awardees+=1;}
      if is_a && s.address_kind!=N58_KIND_DEFINITION {return n58_award_refuse(r,N58_ADDRESS,i);}
      if is_a && a_at>=0 {return n58_award_refuse(r,N58_EXTRA,i);}
      if is_a {r.address=s;a_at=i;}
    }
    i+=1;
  }
  if a_at<0 || r.awardees==0 {return n58_award_refuse(r,N58_MISSING,-1);}
  if !ns_equal32(r.address.pubkey,e.public_key) {return n58_award_refuse(r,N58_ISSUER,a_at);}
  r.status=N58_OK;return r;
}
// Whether a p tag of the award names key.
n58_names :: (e:NostrEvent,key:[32]u8)->bool {
  i:=0;t:=nostr_tag(e,0);
  while t.error!=NT_INDEX && t.error!=NT_EVENT {
    if t.error==NT_OK && t.type==NT_P && ns_equal32(t.pubkey,key) {return true;}
    i+=1;t=nostr_tag(e,i);
  }
  return false;
}
```

The award reader refuses:

- A kind other than 8 (`N58_KIND`).
- An `a` or `p` tag that chapter 22's `nostr_tag` refuses, such as a short
  key or a bad relay hint (`N58_TAG`, at that tag).
- An `a` tag naming a kind other than 30009 (`N58_ADDRESS`). A kind 30008
  badge set is not a badge.
- A second `a` tag (`N58_EXTRA`). NIP-58 says an award has a single `a`
  tag, and two would make it unclear which badge was given.
- No `a` tag or no `p` tag (`N58_MISSING`).
- A signer that is not the `a` tag's author (`N58_ISSUER`, at the `a`
  tag). The record still holds the address, so a client can say whose
  badge was claimed.

`n58_names` reads only `p` tags. It skips any tag that does not read and
stops at the end of the tags or on a broken event.

## Profile badges

A user accepts badges by listing them in a kind 10008 event, a replaceable
event, so only the newest one counts. The list is a sequence of ordered,
consecutive pairs: an `a` tag naming a definition, then an `e` tag naming
the award. The order is the display order. An earlier version of NIP-58
used kind 30008 with `d` `profile_badges`, and NIP-58 asks clients to
treat that as kind 10008. Kind 30008 with any other `d` is a badge set,
which has the same pairs. `n58_profile` reads all three and sets `set` for
a badge set.

<!-- source: coax/lib/nostr/nip58.coax profile -->
```jai
// Whether two a tags name the same definition: kind, issuer and d.
n58_same_address :: (x:NostrTag,y:NostrTag)->bool {
  if x.address_kind!=y.address_kind || !ns_equal32(x.pubkey,y.pubkey) || x.d_n!=y.d_n {return false;}
  for k:0..<x.d_n {if x.d[k]!=y.d[k] {return false;}}
  return true;
}
// The verdict on one pair of owner's profile: the definition a, the award
// id, the verified awards the caller fetched, and the pairs before it.
n58_verdict :: (owner:NostrEvent,a:NostrTag,id:[32]u8,awards:[]NostrEvent,earlier:[]N58Pair)->int {
  for k:0..<earlier.count {if earlier[k].verdict==N58_SHOWN && ns_equal32(earlier[k].award,id) {return N58_DUPLICATE;}}
  found:=-1;
  for k:0..<awards.count {if awards[k].error==NE_OK && ns_equal32(awards[k].id,id) {found=k;break;}}
  if found<0 {return N58_UNFETCHED;}
  w:=n58_award(awards[found]);
  if w.status==N58_ISSUER {return N58_OTHER_ISSUER;}
  if w.status!=N58_OK {return N58_BAD_AWARD;}
  if !n58_same_address(w.address,a) {return N58_MISMATCH;}
  if !n58_names(awards[found],owner.public_key) {return N58_NOT_OWNER;}
  return N58_SHOWN;
}
// The pairs of a profile badges event, in order, each with its verdict.
// A pair is an a tag naming a kind 30009 definition and the e tag right
// after it. An a or e tag that is not part of a pair is counted as
// unpaired; an a tag naming a kind 30008 badge set is counted in sets.
n58_profile :: (e:NostrEvent,awards:[]NostrEvent,out:[]N58Pair)->N58Profile {
  r:N58Profile;if e.error!=NE_OK {return r;}
  if e.kind==N58_KIND_SET {
    d:=nostr_tag_d(e);
    if d.error!=NT_OK {return n58_profile_refuse(r,N58_EVENT,-1);}
    if d.d_n==0 {return n58_profile_refuse(r,N58_MISSING,-1);}
    r.set=cast(string)d.d[0..<d.d_n]!="profile_badges";
  } else if e.kind!=N58_KIND_PROFILE {return n58_profile_refuse(r,N58_KIND,-1);}
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n58_profile_refuse(r,N58_LIMIT,i);}
    if t.error!=NT_OK {return n58_profile_refuse(r,N58_EVENT,i);}
    if n32_is(t.v[0],"e") {r.unpaired+=1;i+=1;continue;}
    if !n32_is(t.v[0],"a") {i+=1;continue;}
    a:=nostr_tag(e,i);
    if a.error==NT_OK && a.address_kind==N58_KIND_SET {r.sets+=1;i+=1;continue;}
    if a.error!=NT_OK || a.address_kind!=N58_KIND_DEFINITION {r.bad+=1;i+=1;continue;}
    next:=n32_tag(e,i+1);
    if next.error!=NT_OK || !n32_is(next.v[0],"e") {r.unpaired+=1;i+=1;continue;}
    x:=nostr_tag(e,i+1);
    if x.error!=NT_OK {r.bad+=2;i+=2;continue;}
    if r.n==out.count {return n58_profile_refuse(r,N58_LIMIT,i);}
    p:N58Pair;p.at=i;p.address=a;p.award=x.id;
    p.verdict=n58_verdict(e,a,x.id,awards,out[0..<r.n]);
    if p.verdict==N58_SHOWN {r.shown+=1;}
    out[r.n]=p;r.n+=1;i+=2;
  }
  r.status=N58_OK;return r;
}
```

### Pairing the tags

`n58_profile` walks the tags in order:

1. An `e` tag that is not right after an `a` tag is unpaired. It is
   counted and ignored.
2. An `a` tag naming a kind 30008 badge set is counted in `sets`. NIP-58
   allows these in a kind 10008 list.
3. An `a` tag that doesn't read, or names a kind other than 30008 or
   30009, is counted in `bad`.
4. An `a` tag naming a kind 30009 definition pairs with the next tag if
   that tag is an `e` tag. Otherwise it is unpaired.
5. A pair whose `e` tag doesn't read counts two tags in `bad`.
6. Any other tag is skipped.

NIP-58 says clients should ignore an `a` tag without its `e` tag and an
`e` tag without its `a` tag. The code ignores them, and counts them so a
client can report them.

### The verdict on a pair

For each pair, `n58_verdict` checks these conditions in order. The first
one that fails gives the verdict:

1. The same award is not already shown by an earlier pair. Otherwise the
   verdict is `N58_DUPLICATE`.
2. The award is in the set of events the caller fetched. Otherwise the
   verdict is `N58_UNFETCHED`. An award that hasn't been fetched is not
   shown.
3. The award reads. If it was signed by a key other than the issuer
   in its `a` tag, the verdict is `N58_OTHER_ISSUER`. Any other refusal,
   or an event of another kind with that id, gives `N58_BAD_AWARD`.
4. The award's `a` tag names the same definition as the pair's `a` tag:
   kind, issuer and `d`. Otherwise the verdict is `N58_MISMATCH`.
5. The award's `p` tags name the profile's owner. Otherwise the verdict is
   `N58_NOT_OWNER`. A user can't show a badge given to someone else.

A pair that passes all five is `N58_SHOWN`. Together, checks 3 and 4 mean
that the issuer in the profile's `a` tag signed the award. A client that
keeps a list of issuers it considers reputable, as NIP-58 suggests, can
filter the shown pairs by `out[k].address.pubkey`.

`n58_profile` refuses only a profile it can't read: a kind other than
10008 or 30008, kind 30008 without a `d`, a tag that is not an array of
strings (`N58_EVENT`), a value over 256 bytes (`N58_LIMIT`), and more pairs
than `out` holds (`N58_LIMIT`, at the pair's `a` tag).

### Worked example: Bob's three pairs

The example uses BIP-340 test vectors 0 to 3 as Alice, Bob, Carol and
Mallory. Alice defines three badges: `bravery`, `honor` and `kindness`.
Three awards follow:

| Award | Signed by | `a` tag | `p` tag |
| --- | --- | --- | --- |
| W1 | Alice | `30009:<Alice>:bravery` | Bob |
| W2 | Mallory | `30009:<Alice>:honor` | Bob |
| W3 | Alice | `30009:<Alice>:kindness` | Carol |

Bob's kind 10008 lists three pairs:

```text
tag 0  a  30009:<Alice>:bravery    tag 1  e  W1
tag 2  a  30009:<Alice>:honor      tag 3  e  W2
tag 4  a  30009:<Alice>:kindness   tag 5  e  W3
```

Work out each pair by hand, with W1, W2 and W3 fetched:

1. Pair at tag 0. No earlier pair shows W1, and W1 is fetched. W1 is
   kind 8 with one `a` tag. Its issuer is Alice's key and its signer is
   Alice's key, so it reads. Its `a` tag is `30009`, Alice, `bravery`, the
   same as the pair's. Its `p` tag is Bob's key, and Bob owns the
   profile. Verdict: shown.
2. Pair at tag 2. W2 is fetched. Its `a` tag names Alice as the issuer,
   but Mallory signed it, so `n58_award` refuses it with `N58_ISSUER`.
   Verdict: award signed by another key. Mallory can't give out Alice's
   badge, even when the award names Bob and Bob lists it.
3. Pair at tag 4. W3 is fetched. Alice signed it, and its `a` tag is
   Alice's `kindness`, the same as the pair's. Its only `p` tag is Carol's
   key, not Bob's. Verdict: award does not name the owner.

Result: shown 1 of 3, unpaired 0, sets 0, bad 0. If none of the awards is
fetched, every pair is `award not fetched` and nothing is shown. Carol's
own profile, which lists `kindness` with W3, shows it.

## User statuses

A user status is an addressable event of kind 30315. Its `d` tag is the
status type, so a user has one current status per type. NIP-38 defines
`general` (for example, "Working" or "Hiking") and `music` (the track
playing now) and allows other types. A status may link to a URL with `r`,
or to a profile, note or address with `p`, `e` or `a`. A `music` status
should expire when the track ends, with a NIP-40 `expiration` tag. An
empty content clears the status.

<!-- source: coax/lib/nostr/nip38.coax read -->
```jai
// A NIP-40 time: one to 16 decimal digits, or -1.
n38_time :: (t:N32Text)->int {
  if t.n<1 || t.n>N38_MAX_DIGITS {return -1;}
  v:=0;
  for i:0..<t.n {
    c:=t.b[i];if c<'0' || c>'9' {return -1;}
    v=v*10+cast(int)(c-'0');
  }
  return v;
}
// A user status at time now. An expired status reads in full with status
// N38_EXPIRED, so a caller can tell it from a refused one.
n38_status :: (e:NostrEvent,now:int)->N38Status {
  r:N38Status;if e.error!=NE_OK {return r;}
  if e.kind!=N38_KIND_STATUS {return n38_refuse(r,N38_KIND,-1);}
  d:=nostr_tag_d(e);if d.error!=NT_OK {return r;}
  for k:0..<d.d_n {r.d.b[k]=d.d[k];}r.d.n=d.d_n;
  i:=0;
  while true {
    t:=n32_tag(e,i);
    if t.error==NT_INDEX {break;}
    if t.error==NT_LIMIT {return n38_refuse(r,N38_LIMIT,i);}
    if t.error!=NT_OK {return n38_refuse(r,N38_EVENT,i);}
    name:=t.v[0];
    if n32_is(name,"expiration") && r.expiration<0 {
      r.expiration=n38_time(t.v[1]);
      if r.expiration<0 {return n38_refuse(r,N38_EXPIRATION,i);}
    }
    if n32_is(name,"r") {
      if t.v[1].n==0 {return n38_refuse(r,N38_TAG,i);}
      if !r.has_url {r.has_url=true;r.url=t.v[1];}
      r.links+=1;
    }
    if n32_is(name,"p") || n32_is(name,"e") || n32_is(name,"a") {
      if nostr_tag(e,i).error!=NT_OK {return n38_refuse(r,N38_TAG,i);}
      r.links+=1;
    }
    i+=1;
  }
  if r.d.n==0 {return n38_refuse(r,N38_MISSING,-1);}
  r.type=ifx n32_is(r.d,"general") then N38_GENERAL else ifx n32_is(r.d,"music") then N38_MUSIC else N38_OTHER;
  r.cleared=e.content_from==e.content_to;
  if r.expiration>=0 && now>=r.expiration {return n38_refuse(r,N38_EXPIRED,-1);}
  r.status=N38_OK;return r;
}
```

`n38_status` keeps the first `expiration` tag and the first `r` tag, and
counts every link. A status is expired at and after its expiration time,
the rule the relay follows in chapter 39. An expired status still reads in
full, with status `N38_EXPIRED`, so a caller can tell it from a refused
one. The reader refuses:

- A kind other than 30315 (`N38_KIND`).
- No `d` tag or an empty one (`N38_MISSING`).
- An `expiration` that is not one to 16 decimal digits (`N38_EXPIRATION`,
  at that tag). The relay treats such a tag as no expiration. A client
  reader that did the same would show forever a status that its author
  meant to end, so this reader refuses it instead.
- An empty `r`, or a `p`, `e` or `a` tag that `nostr_tag` refuses
  (`N38_TAG`).
- A value over 256 bytes (`N38_LIMIT`).

### The current status

A client shows one status per user and type: the newest one. The newest
event replaces the older ones, as NIP-01 says for addressable events: a
later `created_at` wins, and on a tie the lower id wins. The check for
expiry comes after that choice. If the newest status has expired or is
empty, nothing is shown. An older status does not come back, because its
author replaced it.

<!-- source: coax/lib/nostr/nip38.coax current -->
```jai
// The status of type d that author shows at now: the newest kind 30315
// event of that type (later created_at, then the lower id, as NIP-01
// replaces), read at now. The newest wins even when it has expired or is
// empty; an older status never comes back.
n38_current :: (set:[]NostrEvent,author:[32]u8,d:string,now:int)->N38Current {
  c:N38Current;
  for k:0..<set.count {
    e:=set[k];
    if e.error!=NE_OK || e.kind!=N38_KIND_STATUS || !ns_equal32(e.public_key,author) {continue;}
    s:=n38_status(e,now);
    if s.status!=N38_OK && s.status!=N38_EXPIRED {c.refused+=1;continue;}
    if !n32_is(s.d,d) {continue;}
    if c.index>=0 {
      best:=set[c.index];
      if e.created_at<best.created_at || (e.created_at==best.created_at && !ns_before(e.id,best.id)) {continue;}
    }
    c.index=k;c.status=s;
  }
  return c;
}
// Whether a client shows it: a status that reads, is not expired and is
// not cleared.
n38_shown :: (c:N38Current)->bool {
  return c.index>=0 && c.status.status==N38_OK && !c.status.cleared;
}
```

### Worked example: Bob's statuses at three clocks

Bob signs three statuses, with times as seconds after T0 = 1761686400:

| Event | Created | `d` | Content | `expiration` |
| --- | --- | --- | --- | --- |
| S1 | T0+400 | `general` | Working | none |
| S2 | T0+420 | `general` | Hiking | T0+4000 |
| S3 | T0+440 | `music` | Intergalatic - Beastie Boys | T0+640 |

1. `general`: S1 and S2 have the same author and type. S2 is newer, so it
   is the current status at every clock.
2. At T0+600: S2 expires at T0+4000 and 600 < 4000, so it is shown. S3
   expires at T0+640 and 600 < 640, so it is shown.
3. At T0+700: S2 is still shown. For S3, 700 ≥ 640, so it has expired and
   is not shown.
4. At T0+4000: 4000 ≥ 4000, so S2 has expired. Nothing is shown for
   `general`. S1 has no expiration, but S2 replaced it, so it is not shown.

## The example

The example is
[nostr/examples/badges-statuses.coax](../examples/badges-statuses.coax). It
signs 10 events with the published test keys: Alice's three definitions,
the three awards, Bob's profile badges and Bob's three statuses. It reads
each one back with its id and signature checked, then judges Bob's pairs
with the awards fetched and with none fetched, and reads the statuses at
the three clocks. The refusals use unsigned events, because no signer here
would write them.

<!-- source: nostr/examples/badges-statuses.coax example -->
```jai
#load "../../coax/lib/nostr/nip58.coax";
#load "../../coax/lib/nostr/nip38.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

STATUS58 :: [9] string.["ok", "event", "kind", "bad tag", "missing tag", "a is not a definition", "second a", "signer is not the issuer", "limit"];
VERDICT :: [7] string.["shown", "award not fetched", "award does not read", "award signed by another key", "award names another badge", "award does not name the owner", "duplicate"];
STATUS38 :: [8] string.["ok", "event", "kind", "bad tag", "missing d", "bad expiration", "expired", "limit"];
TYPE38 :: [3] string.["general", "music", "other"];
// BIP-340 test vectors 0 to 3's public keys.
PEOPLE :: [4] string.["Alice", "Bob", "Carol", "Mallory"];
PUB :: [4] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
  "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8",
  "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517"];
// Their secret keys: published test values. Never sign with a real key in
// an example.
SECRET :: [4] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef",
  "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9",
  "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710"];
ALICE :: 0; BOB :: 1; CAROL :: 2; MALLORY :: 3;
T0 :: 1761686400;
// The clocks the statuses are read at, after T0.
NOW :: [3] int.[600, 700, 4000];

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
say :: (t: N32Text) { own := t; print("%", cast(string) own.b[0..<own.n]); }

Named :: struct { name: string; event: NostrEvent; }
EVENTS :: 10;
show_key :: (all: [] Named, key: [32] u8) {
  for n: all { if n.event.error == NE_OK && ns_equal32(n.event.id, key) { print("%", n.name); return; } }
  for k: 0..<PEOPLE.count { if ns_equal32(key_of(k), key) { print("%", PEOPLE[k]); return; } }
  text: [64] u8; print("%...", hex32(key, text[..])[0..<8]);
}
show_address :: (all: [] Named, a: NostrTag) {
  own := a; print("%", cast(string) own.d[0..<own.d_n]); print(" by "); show_key(all, a.pubkey);
}

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
// The tags with ^k replaced by person k's key and &k by event k's id.
with_keys :: (template: string, all: [] Named, out: [] u8) -> string {
  n := 0; i := 0; hex: [64] u8;
  while i < template.count {
    c := template[i];
    if c == '^' || c == '&' {
      k := cast(int) (template[i + 1] - '0');
      if c == '^' { n = json_raw(out, n, PUB[k]); } else { n = json_raw(out, n, hex32(all[k].event.id, hex[..])); }
      i += 2; continue;
    }
    n = json_raw(out, n, template[i..<i + 1]); i += 1;
  }
  #assert n > 0;
  return cast(string) out[0..<n];
}

show_definition :: (label: string, e: NostrEvent) {
  d := n58_definition(e);
  print("%: %", label, STATUS58[d.status]);
  if d.at >= 0 { print(" at tag %", d.at); }
  if d.status != N58_OK { print("\n"); return; }
  print(", d "); say(d.d);
  if d.name.n > 0 { print(", name "); say(d.name); }
  if d.image.n > 0 { print(", image "); say(d.image); print(" "); say(d.image_size); }
  print(", thumbs %\n", d.thumbs);
}
show_award :: (all: [] Named, label: string, e: NostrEvent) {
  w := n58_award(e);
  print("%: %", label, STATUS58[w.status]);
  if w.at >= 0 { print(" at tag %", w.at); }
  if w.status == N58_OK || w.status == N58_ISSUER { print(", "); show_address(all, w.address); }
  print(", signed by "); show_key(all, e.public_key);
  if w.status == N58_OK { print(", awardees %, names Bob %", w.awardees, n58_names(e, key_of(BOB))); }
  print("\n");
}
show_profile :: (all: [] Named, label: string, e: NostrEvent, awards: [] NostrEvent) {
  out: [8] N58Pair;
  r := n58_profile(e, awards, out[..]);
  print("%: %", label, STATUS58[r.status]);
  if r.at >= 0 { print(" at tag %", r.at); }
  print(", owner "); show_key(all, e.public_key); print("\n");
  for k: 0..<r.n {
    print("  pair at tag %: ", out[k].at); show_address(all, out[k].address);
    print(", award "); show_key(all, out[k].award); print(": %\n", VERDICT[out[k].verdict]);
  }
  print("  shown % of %, unpaired %, sets %, bad %\n", r.shown, r.n, r.unpaired, r.sets, r.bad);
}
show_status :: (label: string, e: NostrEvent, now: int) {
  s := n38_status(e, now);
  print("%: %", label, STATUS38[s.status]);
  if s.at >= 0 { print(" at tag %", s.at); }
  if s.status != N38_OK && s.status != N38_EXPIRED { print("\n"); return; }
  text: [256] u8; n := json_unescape(e.original[e.content_from..<e.content_to], text[..]);
  print(", % \"%\"", TYPE38[s.type], cast(string) text[0..<n]);
  if s.has_url { print(", r "); say(s.url); }
  if s.expiration >= 0 { print(", expires T0+%", s.expiration - T0); }
  print("\n");
}
show_current :: (all: [] Named, set: [] NostrEvent, d: string, now: int) {
  c := n38_current(set, key_of(BOB), d, now);
  print("Bob's % at T0+%: ", d, now - T0);
  if c.index < 0 { print("none\n"); return; }
  print("newest %, %, shown %\n", all[c.index].name, STATUS38[c.status.status], n38_shown(c));
}

// An unsigned event by Mallory with these tags, parsed but not verified:
// its id and sig are zeros. Used for the refusals, which no signer here
// would write.
unsigned :: (kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[MALLORY]); n = json_raw(out, n, "\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"x\",\"sig\":\"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

// The events: who signs, when (after T0), the kind, the tags with ^k for
// person k's key and &k for event k's id, and the content.
NAME :: [EVENTS] string.["D1", "D2", "D3", "W1", "W2", "W3", "P", "S1", "S2", "S3"];
WHO :: [EVENTS] int.[ALICE, ALICE, ALICE, ALICE, MALLORY, ALICE, BOB, BOB, BOB, BOB];
AT :: [EVENTS] int.[0, 60, 120, 180, 240, 300, 360, 400, 420, 440];
KIND :: [EVENTS] int.[30009, 30009, 30009, 8, 8, 8, 10008, 30315, 30315, 30315];
TAGS :: [EVENTS] string.[
  "[[\"d\",\"bravery\"],[\"name\",\"Medal of Bravery\"],[\"description\",\"Awarded to users demonstrating bravery\"],[\"image\",\"https://nostr.academy/awards/bravery.png\",\"1024x1024\"],[\"thumb\",\"https://nostr.academy/awards/bravery_256x256.png\",\"256x256\"]]",
  "[[\"d\",\"honor\"],[\"name\",\"Honor\"]]",
  "[[\"d\",\"kindness\"],[\"name\",\"Kindness\"]]",
  "[[\"a\",\"30009:^0:bravery\"],[\"p\",\"^1\",\"wss://relay.example.com\"]]",
  "[[\"a\",\"30009:^0:honor\"],[\"p\",\"^1\",\"wss://relay.example.com\"]]",
  "[[\"a\",\"30009:^0:kindness\"],[\"p\",\"^2\",\"wss://relay.example.com\"]]",
  "[[\"a\",\"30009:^0:bravery\"],[\"e\",\"&3\",\"wss://nostr.academy\"],[\"a\",\"30009:^0:honor\"],[\"e\",\"&4\",\"wss://nostr.academy\"],[\"a\",\"30009:^0:kindness\"],[\"e\",\"&5\",\"wss://nostr.academy\"]]",
  "[[\"d\",\"general\"]]",
  "[[\"d\",\"general\"],[\"r\",\"https://nostr.world\"],[\"expiration\",\"1761690400\"]]",
  "[[\"d\",\"music\"],[\"r\",\"spotify:search:Intergalatic%20-%20Beastie%20Boys\"],[\"expiration\",\"1761687040\"]]"];
CONTENT :: [EVENTS] string.["", "", "", "", "", "", "", "Working", "Hiking", "Intergalatic - Beastie Boys"];

main :: () {
  all: [EVENTS] Named; buf: [EVENTS][4096] u8; tags: [4096] u8;
  for k: 0..<EVENTS {
    all[k].name = NAME[k];
    all[k].event = sign(WHO[k], T0 + AT[k], KIND[k], with_keys(TAGS[k], all[..], tags[..]), CONTENT[k], buf[k][..]);
  }
  print("each event's id\n");
  for e: all { hex: [64] u8; print("%: %, verified %\n", e.name, hex32(e.event.id, hex[..]), e.event.verified); }

  print("badge definitions\n");
  for k: 0..<3 { show_definition(all[k].name, all[k].event); }
  print("badge awards\n");
  for k: 3..<6 { show_award(all[..], all[k].name, all[k].event); }
  awards: [3] NostrEvent; for k: 0..<3 { awards[k] = all[3 + k].event; }
  print("Bob's profile badges, with W1, W2 and W3 fetched\n");
  show_profile(all[..], "P", all[6].event, awards[..]);
  print("the same, with no awards fetched\n");
  show_profile(all[..], "P", all[6].event, awards[0..<0]);

  print("statuses read at T0+600\n");
  for k: 7..<10 { show_status(all[k].name, all[k].event, T0 + 600); }
  set: [EVENTS] NostrEvent; for k: 0..<EVENTS { set[k] = all[k].event; }
  for t: NOW { show_current(all[..], set[..], "general", T0 + t); show_current(all[..], set[..], "music", T0 + t); }

  print("refusals\n");
  bad: [4096] u8;
  show_definition("definition with an empty d", unsigned(30009, "[[\"d\",\"\"],[\"name\",\"x\"]]", bad[..]));
  show_definition("definition without d", unsigned(30009, "[[\"name\",\"x\"]]", bad[..]));
  show_award(all[..], "award with two a tags", unsigned(8, with_keys("[[\"a\",\"30009:^3:x\"],[\"a\",\"30009:^3:y\"],[\"p\",\"^1\"]]", all[..], tags[..]), bad[..]));
  show_award(all[..], "award naming no one", unsigned(8, with_keys("[[\"a\",\"30009:^3:x\"]]", all[..], tags[..]), bad[..]));
  show_award(all[..], "award of a badge set", unsigned(8, with_keys("[[\"a\",\"30008:^3:x\"],[\"p\",\"^1\"]]", all[..], tags[..]), bad[..]));
  show_profile(all[..], "profile with a lone e and a lone a", unsigned(10008, with_keys("[[\"e\",\"&3\"],[\"a\",\"30009:^0:bravery\"]]", all[..], tags[..]), bad[..]), awards[..]);
  show_profile(all[..], "profile pair naming the wrong badge", unsigned(10008, with_keys("[[\"a\",\"30009:^0:honor\"],[\"e\",\"&3\"]]", all[..], tags[..]), bad[..]), awards[..]);
  show_status("status expiring soon", unsigned(30315, "[[\"d\",\"general\"],[\"expiration\",\"soon\"]]", bad[..]), T0);
  show_status("status without d", unsigned(30315, "[[\"r\",\"https://nostr.world\"]]", bad[..]), T0);
}
```

Run it:

```shell
coax/bin/coax run nostr/examples/badges-statuses.coax --run-steps 250000000
```

It prints this transcript, which is kept in
[nostr/fixtures/badges-statuses.out](../fixtures/badges-statuses.out) and
which the book's checks compare in debug and release. It exits with
status 0.

```text
each event's id
D1: 7b0a2d339bac9d628504648a1c385eb0bd8c8500da95764741e6d11d8393585b, verified true
D2: 1d6e1d91df88d758611969fc7251fdf1dab457058c0898ebb24b83b5746ea52f, verified true
D3: 7c80784bcd0b23e8efb545d27e92d9fab2ac9db2ef3ed16a1b967cf373c17072, verified true
W1: ceb6245675d4a1f62a4b69f21213dc9aad929cebf6adb8c130196826b214cf27, verified true
W2: 7631462652cbedca4771b1f5824ec36d53ef627467a505191e65ae02cc9eacaa, verified true
W3: 441f90672dc75a13ab29c581e26c624362f48e7c9df40c0e1b66f388cecc3ba6, verified true
P: 384ede1ec51f5a2020159c6dca9d85cb1ccabc5fa5789a3fb041d44f9f53278b, verified true
S1: c2b27cc7dcb93fa09b17be6463e24e10206c0876dd6f1dfb1e3a3d98c9a90134, verified true
S2: d916c00c7d533c634c73c1c6fcb2244faafabff51384207eebb793df703fb206, verified true
S3: 19177dd6715669f17fea9458f2d5bfd0ff0bb541ab096a08f287c59f9e4fdb7c, verified true
badge definitions
D1: ok, d bravery, name Medal of Bravery, image https://nostr.academy/awards/bravery.png 1024x1024, thumbs 1
D2: ok, d honor, name Honor, thumbs 0
D3: ok, d kindness, name Kindness, thumbs 0
badge awards
W1: ok, bravery by Alice, signed by Alice, awardees 1, names Bob true
W2: signer is not the issuer at tag 0, honor by Alice, signed by Mallory
W3: ok, kindness by Alice, signed by Alice, awardees 1, names Bob false
Bob's profile badges, with W1, W2 and W3 fetched
P: ok, owner Bob
  pair at tag 0: bravery by Alice, award W1: shown
  pair at tag 2: honor by Alice, award W2: award signed by another key
  pair at tag 4: kindness by Alice, award W3: award does not name the owner
  shown 1 of 3, unpaired 0, sets 0, bad 0
the same, with no awards fetched
P: ok, owner Bob
  pair at tag 0: bravery by Alice, award W1: award not fetched
  pair at tag 2: honor by Alice, award W2: award not fetched
  pair at tag 4: kindness by Alice, award W3: award not fetched
  shown 0 of 3, unpaired 0, sets 0, bad 0
statuses read at T0+600
S1: ok, general "Working"
S2: ok, general "Hiking", r https://nostr.world, expires T0+4000
S3: ok, music "Intergalatic - Beastie Boys", r spotify:search:Intergalatic%20-%20Beastie%20Boys, expires T0+640
Bob's general at T0+600: newest S2, ok, shown true
Bob's music at T0+600: newest S3, ok, shown true
Bob's general at T0+700: newest S2, ok, shown true
Bob's music at T0+700: newest S3, expired, shown false
Bob's general at T0+4000: newest S2, expired, shown false
Bob's music at T0+4000: newest S3, expired, shown false
refusals
definition with an empty d: bad tag at tag 0
definition without d: missing tag
award with two a tags: second a at tag 1, signed by Mallory
award naming no one: missing tag, signed by Mallory
award of a badge set: a is not a definition at tag 0, signed by Mallory
profile with a lone e and a lone a: ok, owner Mallory
  shown 0 of 0, unpaired 2, sets 0, bad 0
profile pair naming the wrong badge: ok, owner Mallory
  pair at tag 0: honor by Alice, award W1: award names another badge
  shown 0 of 1, unpaired 0, sets 0, bad 0
status expiring soon: bad expiration at tag 1
status without d: missing d
```

The pairs and the statuses match the worked examples. The refusals show
each broken input and its exact diagnostic:

- `definition with an empty d`: `["d",""]`, at tag 0.
- `definition without d`: only a `name` tag.
- `award with two a tags`: a second `a` tag, at tag 1.
- `award naming no one`: an `a` tag and no `p` tag.
- `award of a badge set`: the `a` tag names kind 30008, at tag 0.
- `profile with a lone e and a lone a`: an `e` tag first, then an `a` tag
  with nothing after it. The profile reads, has no pairs and has 2
  unpaired tags.
- `profile pair naming the wrong badge`: the pair names `honor` and W1,
  but W1 is Alice's `bravery` award.
- `status expiring soon`: `["expiration","soon"]`, at tag 1.
- `status without d`: a kind 30315 with only an `r` tag.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/badges-statuses.coax](../examples/checks/badges-statuses.coax),
which loads the example, so running the example doesn't run them. They
compare the code with values that share no code with it:

- The ids of all 10 events, from a Python script written from NIP-01,
  NIP-58 and NIP-38 (`hashlib.sha256` over NIP-01's serialization). The
  script computes the award ids that Bob's `e` tags name itself. The
  checks read each event with Python's id and require that the recomputed
  id matches.
- The three verdicts by hand, with the awards in reverse order, with the
  definitions mixed into the fetched set, and Carol's own profile.
- The current statuses by hand at T0+600, T0+3999, T0+4000, T0+639 and
  T0+640, and S1 showing when S2 is missing.
- NIP-58's three examples, with `alice`, `bob` and `charlie` replaced by
  Alice's, Bob's and Carol's keys and the award ids filled in. The NIP's
  profile example names an honor award that it doesn't show; the check
  uses W1 for it, so that pair is a duplicate and only bravery is shown.
- NIP-38's two examples: the music status is live one second before
  1692845589 and expired at that second.

<!-- source: nostr/examples/checks/badges-statuses.coax check -->
```jai
// The ids from a Python script written from NIP-01, NIP-58 and NIP-38 (it
// shares no code with Coax): hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False), over the same events, with
// the award ids in P's e tags computed by the script itself.
PY_ID :: [EVENTS] string.[
  "7b0a2d339bac9d628504648a1c385eb0bd8c8500da95764741e6d11d8393585b",
  "1d6e1d91df88d758611969fc7251fdf1dab457058c0898ebb24b83b5746ea52f",
  "7c80784bcd0b23e8efb545d27e92d9fab2ac9db2ef3ed16a1b967cf373c17072",
  "ceb6245675d4a1f62a4b69f21213dc9aad929cebf6adb8c130196826b214cf27",
  "7631462652cbedca4771b1f5824ec36d53ef627467a505191e65ae02cc9eacaa",
  "441f90672dc75a13ab29c581e26c624362f48e7c9df40c0e1b66f388cecc3ba6",
  "384ede1ec51f5a2020159c6dca9d85cb1ccabc5fa5789a3fb041d44f9f53278b",
  "c2b27cc7dcb93fa09b17be6463e24e10206c0876dd6f1dfb1e3a3d98c9a90134",
  "d916c00c7d533c634c73c1c6fcb2244faafabff51384207eebb793df703fb206",
  "19177dd6715669f17fea9458f2d5bfd0ff0bb541ab096a08f287c59f9e4fdb7c"];
SIG :: "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";

// Event k with Python's id and a zero signature, read with its id
// recomputed; the signature is not checked. Earlier events give the ids
// that later tags name.
py :: (k: int, all: [] Named, out: [] u8, scratch: [] u8) -> NostrEvent {
  tags: [2048] u8;
  n := json_raw(out, 0, "{\"id\":\""); n = json_raw(out, n, PY_ID[k]);
  n = json_raw(out, n, "\",\"pubkey\":\""); n = json_raw(out, n, PUB[WHO[k]]);
  n = json_raw(out, n, "\",\"created_at\":"); n = json_write_int(out, n, T0 + AT[k]);
  n = json_raw(out, n, ",\"kind\":"); n = json_write_int(out, n, KIND[k]);
  n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, with_keys(TAGS[k], all, tags[..]));
  n = json_raw(out, n, ",\"content\":"); n = json_write_string(out, n, CONTENT[k]);
  n = json_raw(out, n, ",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return nostr_event_read(cast(string) out[0..<n], scratch, false);
}
read_all :: (all: [] Named, buf: [][2048] u8) {
  scratch: [4096] u8;
  for k: 0..<EVENTS { all[k].name = NAME[k]; all[k].event = py(k, all, buf[k][..], scratch[..]); }
}

#test "the ids are Python's" {
  buf: [EVENTS][2048] u8; all: [EVENTS] Named; read_all(all[..], buf[..]);
  for k: 0..<EVENTS { #assert all[k].event.error == NE_OK && ns_equal32(all[k].event.computed_id, all[k].event.id); }
}

// The worked example by hand. Pair 1 (bravery, W1): W1 is signed by Alice,
// its a tag is Alice's bravery and its p tag is Bob: shown. Pair 2 (honor,
// W2): W2's a tag names Alice as issuer but Mallory signed it: another key.
// Pair 3 (kindness, W3): signed by Alice, same badge, but its p tag is
// Carol, not Bob: not the owner. Shown 1 of 3.
#test "the profile verdicts match the hand values, in any order of awards" {
  buf: [EVENTS][2048] u8; all: [EVENTS] Named; read_all(all[..], buf[..]);
  aw: [3] NostrEvent; for k: 0..<3 { aw[k] = all[3 + k].event; }
  out: [4] N58Pair;
  r := n58_profile(all[6].event, aw[..], out[..]);
  #assert r.status == N58_OK && r.n == 3 && r.shown == 1 && r.unpaired == 0 && r.bad == 0;
  #assert out[0].verdict == N58_SHOWN && out[1].verdict == N58_OTHER_ISSUER && out[2].verdict == N58_NOT_OWNER;
  #assert out[0].at == 0 && out[1].at == 2 && out[2].at == 4;
  #assert ns_equal32(out[0].award, all[3].event.id) && ns_equal32(out[1].award, all[4].event.id);
  rev: [3] NostrEvent; for k: 0..<3 { rev[k] = aw[2 - k]; }
  r = n58_profile(all[6].event, rev[..], out[..]);
  #assert r.shown == 1 && out[0].verdict == N58_SHOWN && out[1].verdict == N58_OTHER_ISSUER && out[2].verdict == N58_NOT_OWNER;
  // The awards with the definitions mixed in: a definition is no award.
  mix: [6] NostrEvent; for k: 0..<6 { mix[k] = all[k].event; }
  r = n58_profile(all[6].event, mix[..], out[..]); #assert r.shown == 1;
  // Carol's own profile with W3 shows kindness.
  b: [2048] u8; t: [2048] u8;
  carol := ne_parse(cast(string) b[0..<carol_profile(all[..], b[..], t[..])]);
  r = n58_profile(carol, aw[..], out[..]); #assert r.status == N58_OK && r.shown == 1 && out[0].verdict == N58_SHOWN;
}
// Carol's kind 10008 listing W3, unsigned: only its tags and author matter.
carol_profile :: (all: [] Named, out: [] u8, tags: [] u8) -> int {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[CAROL]); n = json_raw(out, n, "\",\"created_at\":1761686400,\"kind\":10008,\"tags\":");
  n = json_raw(out, n, with_keys("[[\"a\",\"30009:^0:kindness\"],[\"e\",\"&5\"]]", all, tags));
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return n;
}

// The statuses by hand at each clock. General: S2 (created T0+420) replaces
// S1 (T0+400); S2 expires at T0+4000. Music: S3 expires at T0+640.
#test "the current statuses match the hand values" {
  buf: [EVENTS][2048] u8; all: [EVENTS] Named; read_all(all[..], buf[..]);
  set: [EVENTS] NostrEvent; for k: 0..<EVENTS { set[k] = all[k].event; }
  bob := key_of(BOB);
  c := n38_current(set[..], bob, "general", T0 + 600); #assert c.index == 8 && n38_shown(c);
  c = n38_current(set[..], bob, "general", T0 + 3999); #assert c.index == 8 && n38_shown(c);
  c = n38_current(set[..], bob, "general", T0 + 4000); #assert c.index == 8 && !n38_shown(c) && c.status.status == N38_EXPIRED;
  c = n38_current(set[..], bob, "music", T0 + 639); #assert c.index == 9 && n38_shown(c);
  c = n38_current(set[..], bob, "music", T0 + 640); #assert c.index == 9 && !n38_shown(c);
  // Without S2, S1 shows at any time: it has no expiration.
  set[8].error = NE_ID; c = n38_current(set[..], bob, "general", T0 + 1000000); #assert c.index == 7 && n38_shown(c);
  // Alice has no statuses.
  c = n38_current(set[..], key_of(ALICE), "general", T0); #assert c.index == -1;
}

// NIP-58's three examples with alice, bob and charlie as Alice, Bob and
// Carol and the award ids filled in. The profile example names an honor
// award the NIP doesn't show; here it is W1 again. W1 is already shown, so
// the honor pair is a duplicate (it would otherwise be a mismatch: W1 is for
// bravery), and only bravery is shown.
#test "NIP-58's examples read as the NIP says" {
  buf: [EVENTS][2048] u8; all: [EVENTS] Named; read_all(all[..], buf[..]);
  d := n58_definition(all[0].event);
  #assert d.status == N58_OK && n32_is(d.d, "bravery") && n32_is(d.name, "Medal of Bravery") && n32_is(d.image_size, "1024x1024") && d.thumbs == 1 && d.has_description;
  b: [2048] u8; t: [2048] u8;
  e := signed_by(ALICE, 8, with_keys("[[\"a\",\"30009:^0:bravery\"],[\"p\",\"^1\",\"wss://relay\"],[\"p\",\"^2\",\"wss://relay\"]]", all[..], t[..]), b[..]);
  w := n58_award(e); #assert w.status == N58_OK && w.awardees == 2 && n58_names(e, key_of(BOB)) && n58_names(e, key_of(CAROL)) && !n58_names(e, key_of(ALICE));
  c: [2048] u8; u: [2048] u8;
  p := signed_by(BOB, 10008, with_keys("[[\"a\",\"30009:^0:bravery\"],[\"e\",\"&3\",\"wss://nostr.academy\"],[\"a\",\"30009:^0:honor\"],[\"e\",\"&3\",\"wss://nostr.academy\"]]", all[..], u[..]), c[..]);
  aw: [1] NostrEvent; aw[0] = all[3].event; out: [2] N58Pair;
  r := n58_profile(p, aw[..], out[..]);
  #assert r.status == N58_OK && r.n == 2 && r.shown == 1 && out[0].verdict == N58_SHOWN && out[1].verdict == N58_DUPLICATE;
}
// An unsigned event by who: id and sig zero.
signed_by :: (who: int, kind: int, tags: string, out: [] u8) -> NostrEvent {
  n := json_raw(out, 0, "{\"id\":\"0000000000000000000000000000000000000000000000000000000000000000\",\"pubkey\":\"");
  n = json_raw(out, n, PUB[who]); n = json_raw(out, n, "\",\"created_at\":1761686400,\"kind\":");
  n = json_write_int(out, n, kind); n = json_raw(out, n, ",\"tags\":"); n = json_raw(out, n, tags);
  n = json_raw(out, n, ",\"content\":\"\",\"sig\":\""); n = json_raw(out, n, SIG); n = json_raw(out, n, "\"}");
  #assert n > 0;
  return ne_parse(cast(string) out[0..<n]);
}

// NIP-38's two examples: general with an r link; music with an r link and
// expiration 1692845589, live one second before and expired at that time.
#test "NIP-38's examples read as the NIP says" {
  b: [2048] u8;
  s := n38_status(signed_by(BOB, 30315, "[[\"d\",\"general\"],[\"r\",\"https://nostr.world\"]]", b[..]), 0);
  #assert s.status == N38_OK && s.type == N38_GENERAL && n32_is(s.url, "https://nostr.world") && s.expiration == -1;
  m := signed_by(BOB, 30315, "[[\"d\",\"music\"],[\"r\",\"spotify:search:Intergalatic%20-%20Beastie%20Boys\"],[\"expiration\",\"1692845589\"]]", b[..]);
  s = n38_status(m, 1692845588); #assert s.status == N38_OK && s.type == N38_MUSIC && s.expiration == 1692845589;
  s = n38_status(m, 1692845589); #assert s.status == N38_EXPIRED;
  // An empty content clears the status: it reads but is not shown.
  s = n38_status(signed_by(BOB, 30315, "[[\"d\",\"general\"]]", b[..]), 0); #assert s.status == N38_OK && s.cleared;
}
```

```shell
coax/bin/coax test nostr/examples/checks/badges-statuses.coax --run-steps 250000000
```

`coax test` runs 372 checks, including the loaded files' own
checks; all pass.

### The library's own checks

`nip58.coax` and `nip38.coax` have their own `#test` and `#forall` blocks:
NIP-58's and NIP-38's examples, each refusal and the tag index it reports,
the first-of-each-tag rules, every bound at its edge, broken tag arrays and
events, the pairing rules (lone `e`, `a` then `a`, badge sets, bad tags),
duplicates, an id shared by two events in the fetched set, expiry at the
exact second, ties on `created_at`, and an empty newest status.

```shell
coax/bin/coax test coax/lib/nostr/nip58.coax
coax/bin/coax test coax/lib/nostr/nip38.coax
coax/bin/coax mutate coax/lib/nostr/nip58.coax --root-only
coax/bin/coax mutate coax/lib/nostr/nip38.coax --root-only
```

On 2026-11-01, `coax test` passed all 212 and 208 checks (with the
loaded files'). `coax mutate --root-only` killed all 352 mutants of
`nip58.coax` and all 159 of `nip38.coax`, with none surviving and none
marked equivalent. The full mutation run (`COAX_MUTATION_FULL=1`) and a
mutation run of the example were not run.

### Not built

- Fetching definitions and awards from relays, and picking the newest
  kind 10008 of a user. The caller passes the events.
- Choosing the thumbnail size for the space available, and an allowlist of
  issuers. A client can filter shown pairs by issuer.
- NIP-30 custom emoji in a status's content.

## Summary

- `n58_definition` reads a kind 30009 badge definition and requires a
  non-empty `d`.
- `n58_award` reads a kind 8 award with one `a` tag naming a kind 30009
  definition and at least one `p` tag, and refuses it unless the
  definition's author signed it.
- `n58_profile` pairs each `a` tag with the `e` tag right after it,
  ignores unpaired tags, and shows a pair only when its award is fetched,
  signed by the issuer, names the same definition and names the profile's
  owner.
- `n38_status` reads a kind 30315 status at a caller-supplied time and
  marks it expired at and after its `expiration`.
- `n38_current` picks the newest status of a type. When that one has
  expired or is empty, nothing is shown, and an older status does not come
  back.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-58](../nips/official/58.md)
- [NIP-38](../nips/official/38.md)
- [NIP-40](../nips/official/40.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
