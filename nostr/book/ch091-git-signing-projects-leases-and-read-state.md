# 91. Git signing, projects, leases and read state

Block's developer NIPs use Nostr keys outside of chat. NIP-GS signs Git
commits and tags with a Nostr key. NIP-MP groups repositories into projects.
NIP-PL asks a relay to wake a phone when a matching event arrives. NIP-RS
keeps which messages you have read in step across your devices. This chapter
builds the part of each that decides a result from bytes and numbers, and
checks it against the NIPs' published vectors and fixtures.

Part VI, Every NIP. Result: NIP-GS signing hashes, signatures and armor that
reproduce both published vectors, with the status lines Git reads; the
NIP-MP relay ingest rules and the client fold, which give the stated result
for all 31 ingest fixtures and all 12 fold fixtures; the NIP-PL expiration
window and replacement order with a fixed clock; the NIP-RS max-merge,
`created_at` rule and ciphertext vector; and the four protobuf projections
that check an envelope against its original. What is not built is listed in
[What is not built](#what-is-not-built).

## What this chapter teaches

- GS: how Git calls a signing program, the signing hash, the armored JSON
  envelope with exactly one spelling, the owner attestation, and the
  published test vectors.
- MP: the `kind:30621` project event, the eight relay ingest rules, the fold
  that places repositories inside projects or on their own cards, and why
  paging by `until` alone loses events.
- PL: the `kind:30350` lease envelope, its expiration window, and the order a
  replacement must win.
- RS: the `kind:30078` read-state envelope, the max-merge, the `created_at`
  rule and the encrypt-to-self ciphertext vector.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [gs-projection.coax](../../coax/lib/nostr/adapters/gs-projection.coax) | `gsa_read(wire)`, `gsa_put`, `gsa_hex`, `gsa_decimal`, `gsa_base64` | Rebuilds the canonical NIP-GS armor from a protobuf projection and compares it byte for byte with the retained armor |
| [mp-projection.coax](../../coax/lib/nostr/adapters/mp-projection.coax) | `mpa_read(wire)` | Checks that a `kind:30621` projection agrees with its signed original |
| [pl-projection.coax](../../coax/lib/nostr/adapters/pl-projection.coax) | `pla_read(wire)` | Checks a `kind:30350` envelope: exactly `d`, `expiration`, `exec` and at most one `alt` |
| [rs-projection.coax](../../coax/lib/nostr/adapters/rs-projection.coax) | `rsa_read(wire)` | Checks a `kind:30078` envelope: one `d` of `read-state:` and 32 lowercase hex, one `["t", "read-state"]` |
| [dev-nips.coax](../examples/dev-nips.coax) | `gs_hash`, `oa_hash`, `gs_armor`, `gs_status` | The NIP-GS signing hash, the NIP-OA hash, the envelope and the status lines |
| [dev-nips.coax](../examples/dev-nips.coax) | `mp_ingest`, `mp_fold`, `mp_page`, `mp_enumerate` | The NIP-MP ingest rules, the fold and the two paging cursors |
| [dev-nips.coax](../examples/dev-nips.coax) | `pl_window`, `pl_replace` | The NIP-PL expiration window and replacement order |
| [dev-nips.coax](../examples/dev-nips.coax) | `rs_put`, `rs_merge`, `rs_may_publish`, `rs_created_at` | The NIP-RS merge and publishing rules |

The four projections are `#authority` files with their own checks and
mutation records (see [How the code is checked](#how-the-code-is-checked)).
Signing and verification are Bitcoin chapter 24's and Bitcoin chapter 23's; SHA-256 is
Bitcoin chapter 4's; the NIP-44 decryption is chapter 20's. The procedures in
`dev-nips.coax` are this chapter's code: no program outside the book calls
them, so they stay in the example rather than in `coax/lib/`. This chapter
changes no library code.

## Git signing (NIP-GS)

Git signs a commit by piping the commit object's bytes to a signing program.
With `gpg.format=x509`, it runs `<program> --status-fd=<N> -bsau <key>`,
reads a detached signature from standard output, and looks for
`[GNUPG:] SIG_CREATED` on file descriptor `N`. To verify, it runs
`<program> --status-fd=<N> --verify <file> -` and reads `GOODSIG`, `BADSIG`,
`VALIDSIG`, `ERRSIG` and `TRUST_*`. NIP-GS defines a program that answers
both calls with a Nostr key.

The signature is BIP-340 Schnorr over this hash:

```jai
hash = SHA-256("nostr:git:v1:" || decimal(t) || ":" || oa_binding || payload)
```

`t` is the signer's claimed time, 0 to 4294967295, written without leading
zeros. `oa_binding` is empty, or, when the signature carries an owner
attestation, its three strings each followed by `:`. The attestation is a
NIP-OA authorization: an owner key signs
`SHA-256("nostr:agent-auth:" || agent_pubkey || ":" || conditions)`. Because
the attestation is inside the hash, removing it breaks the signature.

The program writes compact JSON in the fixed order `v`, `pk`, `sig`, `t`,
`oa`, encodes it as one line of padded base64, and wraps it:

```jai
-----BEGIN SIGNED MESSAGE-----\n<base64>\n-----END SIGNED MESSAGE-----\n
```

The signature is stored inside the commit, so any change to its bytes
changes the commit hash. The verifier therefore accepts exactly one spelling
for each set of values: no spaces, no other key order, no `1.7e9`, lowercase
hex, no wrapped base64, and one optional extra LF at the end.
`gsa_read` enforces this by building the canonical armor from the projected
values and comparing bytes, so there is nothing to parse leniently.

### Worked example: the published vector's sizes

The test payload is a 170-byte commit with no final LF. With `t =
1700000000` and no attestation, the preimage is `13 + 10 + 1 + 170 = 194`
bytes. The JSON is:

| Piece | Bytes |
| --- | --- |
| `{"v":1,"pk":"` | 13 |
| the public key in hex | 64 |
| `","sig":"` | 9 |
| the signature in hex | 128 |
| `","t":` | 6 |
| `1700000000` | 10 |
| `}` | 1 |
| total | 231 |

231 bytes is 77 groups of 3, so the base64 line is `77 × 4 = 308`
characters. The armor adds 31 bytes before (`-----BEGIN SIGNED MESSAGE-----`
and LF) and 30 after (LF, `-----END SIGNED MESSAGE-----`, LF): 369 bytes.

With the attestation (owner key 1, empty conditions), the preimage adds
`64 + 1 + 0 + 1 + 128 + 1 = 195` bytes: 389. The JSON adds `,"oa":["`
(8), 64, `","` (3), 0, `","` (3), 128 and `"]` (2): 439 bytes, which is 147
base64 groups, 588 characters, and an armor of `31 + 588 + 30 = 649` bytes.
The NIP-OA preimage is `17 + 64 + 1 + 0 = 82` bytes.

The published signatures use auxiliary randomness of 32 zero bytes. BIP-340
signing is deterministic for a fixed key, message and auxiliary input, so
Bitcoin chapter 24's signer must reproduce them exactly.

<!-- source: nostr/examples/dev-nips.coax gs -->
```jai
// NIP-GS. The signing hash is SHA-256 of "nostr:git:v1:", decimal(t), ":",
// the owner attestation's three strings each followed by ":" (only when
// there is one), and the payload git pipes to the program.
GsOwner :: struct { present: bool; key: string; conditions: string; signature: string; }
gs_hash :: (t: u32, owner: GsOwner, payload: string, out: [] u8) -> int {
  pre: [1024] u8;
  at := gsa_put(pre[..], 0, "nostr:git:v1:");
  at = gsa_decimal(pre[..], at, t);
  at = gsa_put(pre[..], at, ":");
  if owner.present {
    at = gsa_put(pre[..], at, owner.key); at = gsa_put(pre[..], at, ":");
    at = gsa_put(pre[..], at, owner.conditions); at = gsa_put(pre[..], at, ":");
    at = gsa_put(pre[..], at, owner.signature); at = gsa_put(pre[..], at, ":");
  }
  at = gsa_put(pre[..], at, payload);
  #assert at > 0 && sha256_hash_checked(pre[0..<at], out, false) == SHA_OK;
  return at;
}
// The NIP-OA preimage an owner signs: "nostr:agent-auth:", the agent key, ":"
// and the conditions.
oa_hash :: (agent_hex: string, conditions: string, out: [] u8) -> int {
  pre: [512] u8;
  at := gsa_put(pre[..], 0, "nostr:agent-auth:");
  at = gsa_put(pre[..], at, agent_hex); at = gsa_put(pre[..], at, ":");
  at = gsa_put(pre[..], at, conditions);
  #assert at > 0 && sha256_hash_checked(pre[0..<at], out, false) == SHA_OK;
  return at;
}
// The armored signature: compact JSON in v, pk, sig, t, oa order, standard
// padded base64, and the three armor lines. Returns the length written.
gs_armor :: (pk: [] u8, sig: [] u8, t: u32, owner: GsOwner, out: [] u8) -> int {
  json: [GSA_JSON_MAX] u8;
  at := gsa_put(json[..], 0, "{\"v\":1,\"pk\":\""); at = gsa_hex(json[..], at, pk);
  at = gsa_put(json[..], at, "\",\"sig\":\""); at = gsa_hex(json[..], at, sig);
  at = gsa_put(json[..], at, "\",\"t\":"); at = gsa_decimal(json[..], at, t);
  if owner.present {
    at = gsa_put(json[..], at, ",\"oa\":[\""); at = gsa_put(json[..], at, owner.key);
    at = gsa_put(json[..], at, "\",\""); at = gsa_put(json[..], at, owner.conditions);
    at = gsa_put(json[..], at, "\",\""); at = gsa_put(json[..], at, owner.signature);
    at = gsa_put(json[..], at, "\"]");
  }
  at = gsa_put(json[..], at, "}");
  e := gsa_put(out, 0, GSA_BEGIN); e = gsa_base64(out, e, json[0..<at]); e = gsa_put(out, e, GSA_END);
  return e;
}
// The UTC date of t as YYYY-MM-DD, for VALIDSIG: days since 1970-01-01
// turned into a proleptic Gregorian date in 400-year eras of 146097 days.
gs_date :: (t: u32, out: [] u8) -> string {
  z := cast(int) t / 86400 + 719468;
  era := z / 146097; doe := z - era * 146097;
  yoe := (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
  doy := doe - (365 * yoe + yoe / 4 - yoe / 100);
  mp := (5 * doy + 2) / 153;
  day := doy - (153 * mp + 2) / 5 + 1;
  month := ifx mp < 10 then mp + 3 else mp - 9;
  year := yoe + era * 400 + (ifx month <= 2 then 1 else 0);
  digits := [8] int.[year / 1000, year / 100 % 10, year / 10 % 10, year % 10, month / 10, month % 10, day / 10, day % 10];
  at := 0;
  for i: 0..<8 {
    if i == 4 || i == 6 { out[at] = '-'; at += 1; }
    out[at] = cast(u8) ('0' + digits[i]); at += 1;
  }
  return cast(string) out[0..<at];
}
// The verifier's status lines for a signature that parsed (steps 8 to 11).
gs_status :: (good: bool, pk_hex: string, t: u32, trusted: bool) {
  print("[GNUPG:] NEWSIG\n");
  if !good { print("[GNUPG:] BADSIG % %\n", pk_hex, pk_hex); return; }
  print("[GNUPG:] GOODSIG % %\n", pk_hex, pk_hex);
  date: [10] u8;
  print("[GNUPG:] VALIDSIG % % % 0 - - - - - %\n", pk_hex, gs_date(t, date[..]), t, pk_hex);
  print("[GNUPG:] TRUST_% 0 shell\n", ifx trusted then "FULLY" else "UNDEFINED");
}
```

`VALIDSIG` carries the signing date. `gs_date` converts `t` to a UTC date:
1700000000 seconds is 19675 days after 1970-01-01 (with 80000 seconds left
over), which is 2023-11-14.

## Projects (NIP-MP)

A project is an addressable `kind:30621` event: one `d` slug, optional
`name`, `description`, `buzz-channel` and `buzz-visibility`, and zero to 64
`a` tags, each naming one repository announcement as
`30617:<owner>:<repo-d>`. Membership grants nothing: anyone may list anyone's
repository, and the project's signer gets no authority over it.

### Ingest

A relay refuses a project that breaks any of eight rules, checked in this
order: `d-cardinality` (exactly one `d`), `d-empty`, `member-cap` (at most 64
`a` tags, counting duplicates), `member-tag-arity` (two or three elements),
`member-coordinate-malformed`, `member-duplicate` (the same coordinate twice,
whatever the relay hints), `metadata-cardinality` and `metadata-length`
(`name` 256 bytes, `description` 2048, the two `buzz-` tags 256). A
coordinate splits on its first two colons only, so a repository `d` that
contains a colon is still addressable. Unknown tags and the content are
ignored.

<!-- source: nostr/examples/dev-nips.coax mp-ingest -->
```jai
// NIP-MP ingest: the eight relay rules, in the NIP's order. A tag is up to
// four strings. Returns the first rule that fires, or "accept".
MpTag :: struct { n: int; v: [4] string; }
mp_tag :: (a: string, b: string) -> MpTag { t: MpTag; t.n = 2; t.v[0] = a; t.v[1] = b; return t; }
mp_tag3 :: (a: string, b: string, c: string) -> MpTag { t := mp_tag(a, b); t.n = 3; t.v[2] = c; return t; }
mp_tag4 :: (a: string, b: string, c: string, d: string) -> MpTag { t := mp_tag3(a, b, c); t.n = 4; t.v[3] = d; return t; }
MP_MEMBER_CAP :: 64;
lower_hex :: (c: u8) -> bool { return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f'); }
// "30617:" + 64 lowercase hex + ":" + a non-empty d that may hold colons.
mp_coordinate :: (s: string) -> bool {
  if s.count < 72 || s[0..<6] != "30617:" || s[70] != ':' { return false; }
  for i: 6..<70 { if !lower_hex(s[i]) { return false; } }
  return true;
}
MP_META :: [4] string.["name", "description", "buzz-channel", "buzz-visibility"];
MP_META_MAX :: [4] int.[256, 2048, 256, 256];
mp_ingest :: (tags: [] MpTag) -> string {
  ds := 0; d := "";
  members := 0;
  for i: 0..<tags.count {
    if tags[i].v[0] == "d" { ds += 1; if tags[i].n > 1 { d = tags[i].v[1]; } }
    if tags[i].v[0] == "a" { members += 1; }
  }
  if ds != 1 { return "d-cardinality"; }
  if d.count == 0 { return "d-empty"; }
  if members > MP_MEMBER_CAP { return "member-cap"; }
  for i: 0..<tags.count { if tags[i].v[0] == "a" && (tags[i].n < 2 || tags[i].n > 3) { return "member-tag-arity"; } }
  for i: 0..<tags.count { if tags[i].v[0] == "a" && !mp_coordinate(tags[i].v[1]) { return "member-coordinate-malformed"; } }
  for i: 0..<tags.count {
    for j: 0..<i {
      if tags[i].v[0] == "a" && tags[j].v[0] == "a" && tags[i].v[1] == tags[j].v[1] { return "member-duplicate"; }
    }
  }
  for m: 0..<4 {
    seen := 0;
    for i: 0..<tags.count { if tags[i].v[0] == MP_META[m] { seen += 1; } }
    if seen > 1 { return "metadata-cardinality"; }
  }
  for m: 0..<4 {
    for i: 0..<tags.count {
      if tags[i].v[0] == MP_META[m] && tags[i].n > 1 && tags[i].v[1].count > MP_META_MAX[m] { return "metadata-length"; }
    }
  }
  return "accept";
}
```

### The fold

A client builds its collection from the latest heads. A project is *listing
eligible* when it is live, not `unlisted` and not hidden by this viewer. It
*claims* a member when it is eligible and its signer is the member's owner
(the key in the coordinate) or in the repository's own `maintainers` tag.
Then:

1. Every eligible project renders as a container with its members inside.
2. A member that resolves to no live repository renders as unavailable.
3. A repository the viewer hid is hidden everywhere, including inside
   projects.
4. A live, unhidden repository that no project claims also renders as its
   own card.

<!-- source: nostr/examples/dev-nips.coax mp-fold -->
```jai
// NIP-MP's client fold over already accepted heads. A repository is its
// coordinate, whether it is live, whether this viewer hid it, and its
// maintainers; a project adds its signer, members and visibility.
MpRepo :: struct { coordinate: string; live: bool; hidden: bool; maintainers: [2] string; created_at: int; }
MpProject :: struct { coordinate: string; signer: string; members: [4] string; member_count: int;
                      unlisted: bool; live: bool; hidden: bool; created_at: int; }
MpPlaced :: struct { coordinate: string; resolved: bool; }
MpContainer :: struct { project: string; members: [4] MpPlaced; count: int; }
MpCollection :: struct { containers: [8] MpContainer; count: int; cards: [8] string; card_count: int; }

mp_repo :: (coordinate: string) -> MpRepo { r: MpRepo; r.coordinate = coordinate; r.live = true; return r; }
mp_project :: (coordinate: string, signer: string) -> MpProject {
  p: MpProject; p.coordinate = coordinate; p.signer = signer; p.live = true; return p;
}
with_member :: (p: MpProject, coordinate: string) -> MpProject {
  r := p; r.members[r.member_count] = coordinate; r.member_count += 1; return r;
}

// Listing eligible: live, not unlisted, not hidden by this viewer.
mp_eligible :: (p: MpProject) -> bool { return p.live && !p.unlisted && !p.hidden; }
mp_find :: (repos: [] MpRepo, coordinate: string) -> int {
  for i: 0..<repos.count { if repos[i].coordinate == coordinate && repos[i].live { return i; } }
  return -1;
}
// A project claims a member when it is listing eligible and its signer is the
// member coordinate's owner or in the repository's own maintainers tag.
mp_claims :: (p: MpProject, r: MpRepo) -> bool {
  if !mp_eligible(p) { return false; }
  if p.signer == r.coordinate[6..<70] { return true; }
  for k: 0..<2 { if r.maintainers[k].count > 0 && r.maintainers[k] == p.signer { return true; } }
  return false;
}
mp_fold :: (repos: [] MpRepo, projects: [] MpProject) -> MpCollection {
  c: MpCollection;
  for i: 0..<projects.count {
    p := projects[i];
    if !mp_eligible(p) { continue; }
    box: MpContainer; box.project = p.coordinate;
    for m: 0..<p.member_count {
      at := mp_find(repos, p.members[m]);
      if at >= 0 && repos[at].hidden { continue; }  // hidden everywhere
      box.members[box.count] = MpPlaced.{coordinate = p.members[m], resolved = at >= 0};
      box.count += 1;
    }
    c.containers[c.count] = box; c.count += 1;
  }
  for i: 0..<repos.count {
    r := repos[i];
    if !r.live || r.hidden { continue; }
    claimed := false;
    for j: 0..<projects.count {
      for m: 0..<projects[j].member_count {
        if projects[j].members[m] == r.coordinate && mp_claims(projects[j], r) { claimed = true; }
      }
    }
    if !claimed { c.cards[c.card_count] = r.coordinate; c.card_count += 1; }
  }
  return c;
}
```

### Worked example: a stranger's project

Alice owns `30617:aaaa…:buzz`. Bob's project `borrowed` lists it.

1. `borrowed` is eligible, so it renders with `buzz` inside.
2. Bob's key is not the owner `aaaa…`, and the repository has no
   `maintainers`, so `borrowed` does not claim `buzz`.
3. No project claims `buzz`, so it also renders as its own card.

Add Alice's own project `platform` listing `buzz`. It claims `buzz`, one claim
suppresses the card, and `buzz` renders inside both containers and nowhere
else. Mark `platform` `unlisted` and drop `borrowed`: no container renders,
nothing claims `buzz`, and it returns as a card.

### Paging

The fold needs every head. A relay returns a page sorted by `created_at`
descending, and paging with `until` alone cannot resume inside a second.
Take five heads, three at 1000 (`buzz`, `buzz-infra`, `buzz-mobile`) and two
at 900, served two to a page:

| Page | `until` only, stepping to oldest − 1 | Composite cursor `(created_at, coordinate)` |
| --- | --- | --- |
| 1 | `buzz`, `buzz-infra`; next `until = 999` | `buzz`, `buzz-infra`; next after `(1000, buzz-infra)` |
| 2 | `platform`, `mobile`; next `until = 899` | `buzz-mobile`, `platform`; next after `(900, platform)` |
| 3 | nothing: stop | `mobile`: a short page, stop |
| Gathered | 4 of 5: `buzz-mobile` is lost | 5 of 5 |

The composite cursor resumes after the exact `(created_at, id)` pair it last
saw; the fixtures order by coordinate in place of the event id, which unsigned
fixtures do not have.

<!-- source: nostr/examples/dev-nips.coax mp-page -->
```jai
// Enumeration. Heads sorted by (created_at descending, coordinate ascending)
// are served in pages of size; the composite cursor resumes after the last
// (created_at, coordinate) seen. The coordinate stands in for the event id.
MpHead :: struct { created_at: int; coordinate: string; }
MP_UNTIL_ANY :: 2147483647;  // a first page's until: later than every head
mp_before :: (a: MpHead, b: MpHead) -> bool {
  if a.created_at != b.created_at { return a.created_at > b.created_at; }
  n := ifx a.coordinate.count < b.coordinate.count then a.coordinate.count else b.coordinate.count;
  for i: 0..<n { if a.coordinate[i] != b.coordinate[i] { return a.coordinate[i] < b.coordinate[i]; } }
  return a.coordinate.count < b.coordinate.count;
}
// One page: the first size heads (in order) after the cursor. composite
// false uses until alone: created_at <= until.
mp_page :: (heads: [] MpHead, size: int, composite: bool, until: int, after: string, out: [] MpHead) -> int {
  n := 0;
  for i: 0..<heads.count {
    h := heads[i];
    ok := ifx composite then h.created_at < until || (h.created_at == until && mp_before(MpHead.{created_at = until, coordinate = after}, h))
          else h.created_at <= until;
    if ok && n < size { out[n] = h; n += 1; }
  }
  return n;
}
// Page to exhaustion. Naive until-only paging steps to oldest - 1 after each
// page, as a client that ignores ties does. Returns the heads gathered.
mp_enumerate :: (heads: [] MpHead, size: int, composite: bool, out: [] MpHead) -> int {
  got := 0; until := MP_UNTIL_ANY; after := "";
  while true {
    page: [8] MpHead;
    n := mp_page(heads, size, composite, until, after, page[..]);
    for i: 0..<n { out[got] = page[i]; got += 1; }
    if n < size { break; }
    until = page[n - 1].created_at; after = page[n - 1].coordinate;
    if !composite { until -= 1; }
  }
  return got;
}
```

## Push leases (NIP-PL)

A lease is an addressable `kind:30350` event from a phone's installation. Its
public tags are exactly one `d` (a random installation id), one `expiration`,
one `exec` (the executor key the content is encrypted to) and at most one
`alt`; any other tag refuses it. The content is NIP-44 ciphertext holding the
push endpoint and the filters to wake on, which only the executor reads. A
match only wakes the device: the wake carries a fixed "reconnect" payload,
the device fetches events over its own authenticated connection, and the
relay checks the lease author's read access again at match time. Leases from
different origins never share state.

The executor accepts an expiration only when
`now − allowed_skew < expiration ≤ now + max_lease_ttl`, and a replacement
at the same address only when it wins NIP-01 ordering (a newer `created_at`,
or the same one and a lower event id) and raises the generation watermark.

<!-- source: nostr/examples/dev-nips.coax pl -->
```jai
// NIP-PL acceptance checks with numbers, not keys. The expiration must
// satisfy now - skew < expiration <= now + max_ttl.
pl_window :: (now: int, expiration: int, skew: int, max_ttl: int) -> string {
  if expiration <= now - skew { return "invalid: lease already expired"; }
  if expiration > now + max_ttl { return "invalid: lease ttl too long"; }
  return "ok";
}
// Check 8: a replacement at the same address must win NIP-01 ordering (newer
// created_at, or the same and a lower id) and raise the generation watermark.
pl_replace :: (stored_at: int, stored_id: string, watermark: int, at: int, id: string, generation: int) -> string {
  wins := at > stored_at || (at == stored_at && mp_before(MpHead.{created_at = 0, coordinate = id}, MpHead.{created_at = 0, coordinate = stored_id}));
  if !wins { return "invalid: stale replacement"; }
  if generation <= watermark { return "invalid: stale generation"; }
  return "ok";
}
```

### Worked example: the window with a fixed clock

With `now = 1769990000`, `allowed_skew = 900` (15 minutes) and
`max_lease_ttl = 2592000` (30 days), the window is
`(1769989100, 1772582000]`:

| `expiration` | Result |
| --- | --- |
| 1769989100 (`now − skew`) | `invalid: lease already expired` |
| 1769989101 | accepted |
| 1772582000 (`now + ttl`) | accepted |
| 1772582001 | `invalid: lease ttl too long` |

A stored lease at `created_at = 100`, id `bb`, watermark 3: a replacement at
101 with generation 4 wins; one at 100 with id `aa` wins the tie; one at 100
with id `cc` is `invalid: stale replacement`; one at 101 with generation 3 is
`invalid: stale generation`.

## Read state (NIP-RS)

Each of your clients publishes one addressable `kind:30078` event with
`["d", "read-state:<32 lowercase hex>"]` and one `["t", "read-state"]`. The
slot id is random, so a relay can recognize the coordinate without learning
which device it belongs to. The content is NIP-44 encrypted to yourself
(your key with your own public key) and holds `v`, a `client_id` and a map
from context ids to "read up to" timestamps. A reader ignores an event with
zero or two `d` tags, a different prefix, a malformed slot, or zero or two
read-state `t` tags; `rsa_read` decides exactly that.

The merge takes, for each context, the largest timestamp seen. That join is
commutative, associative and idempotent, so devices converge in any order
and a timestamp never goes back. A client writes only its own coordinate: if
the blob there has another `client_id`, it picks a new slot. When it
publishes, `created_at` must be greater than every `created_at` it fetched
for that `d`.

<!-- source: nostr/examples/dev-nips.coax rs -->
```jai
// NIP-RS. Read state is a grow-only max register per context.
RsEntry :: struct { context: string; at: int; }
RsState :: struct { entries: [16] RsEntry; count: int; }
rs_put :: (s: RsState, context: string, at: int) -> RsState {
  r := s;
  for i: 0..<r.count {
    if r.entries[i].context == context {
      if at > r.entries[i].at { r.entries[i].at = at; }
      return r;
    }
  }
  r.entries[r.count] = RsEntry.{context = context, at = at}; r.count += 1;
  return r;
}
rs_merge :: (a: RsState, b: RsState) -> RsState {
  r := a;
  for i: 0..<b.count { r = rs_put(r, b.entries[i].context, b.entries[i].at); }
  return r;
}
rs_get :: (s: RsState, context: string) -> int {
  for i: 0..<s.count { if s.entries[i].context == context { return s.entries[i].at; } }
  return -1;
}
// The same contexts with the same timestamps, in any order.
rs_same :: (a: RsState, b: RsState) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if rs_get(b, a.entries[i].context) != a.entries[i].at { return false; } }
  return true;
}
// A client may write its own coordinate only when the blob there is its own.
rs_may_publish :: (own_client: string, fetched_client: string) -> bool { return own_client == fetched_client; }
// created_at for the next publish: never at or below what was fetched.
rs_created_at :: (now: int, max_fetched: int) -> int { return ifx now <= max_fetched then max_fetched + 1 else now; }
```

### Worked example: two devices

| Context | Device A | Device B | Merged |
| --- | --- | --- | --- |
| `group:general` | 1700001000 | 1700001200 | 1700001200 |
| `group:dev` | 1700000500 | — | 1700000500 |
| `group:random` | — | 1700000800 | 1700000800 |

With the local clock at 1700001200 and fetched copies at 1700001000 and
1700001500, the next `created_at` is `1700001500 + 1 = 1700001501`.

## What is not built

- **The NIP-GS program.** Nothing reads standard input, parses the
  `-bsau` and `--verify` arguments, writes to the status descriptor, loads a
  key from the environment, or reads `user.signingkey` from Git
  configuration. `gs_status` prints the lines that program would write. The
  owner attestation's conditions are not evaluated.
- **The protobuf projections are not signature checks.** `gsa_read` covers
  verification steps 1 to 4 (armor, base64, JSON spelling, bounds), not
  `lift_x`, the BIP-340 check or trust. `mpa_read`, `pla_read` and `rsa_read`
  check that a projection agrees with its original; signature, freshness and
  acceptance are separate.
- **NIP-MP on the wire.** `mp_ingest` and `mp_fold` take tags and semantic
  heads, as the fixtures do. Relay admission of `kind:30621`, NIP-09
  deletion by the NIP-OA owner, and fetching heads from a relay are not
  wired. Mode-2 boundary-bucket draining is described but not built.
- **NIP-PL beyond the envelope.** The lease plaintext (schema, filter
  grammar, classes, quotas), executor discovery and its descriptor, origin
  binding, matching, wake delivery, the APNs gateway and `kind:30390` have no
  Coax code.
- **NIP-RS beyond the merge.** Parsing and validating the decrypted JSON
  (`v`, `client_id` length, 10,000-entry and 256-byte limits), the `esc:`
  namespace, the manual-unread override layer, full-state loads and the
  atomic snapshot extension are not built.

## The example

[nostr/examples/dev-nips.coax](../examples/dev-nips.coax) uses only
published test keys: secret 3 (the NIP-GS agent) and secret 1 (the NIP-OA
owner and the NIP-RS ciphertext key). It reproduces both NIP-GS vectors,
runs six NIP-MP ingest cases, four folds and the paging table, the NIP-PL
window and replacement table, and the NIP-RS merge, ciphertext, conflict and
clock vectors. Each section ends with the protobuf projection accepting a
matching envelope and refusing broken ones.

<!-- source: nostr/examples/dev-nips.coax example -->
```jai
#load "../../coax/lib/nostr/adapters/gs-projection.coax";
#load "../../coax/lib/nostr/adapters/mp-projection.coax";
#load "../../coax/lib/nostr/adapters/pl-projection.coax";
#load "../../coax/lib/nostr/adapters/rs-projection.coax";
#load "../../coax/lib/crypto/schnorr_sign.coax";
#load "../../coax/lib/nostr/nip44-key.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [256] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  #assert r.error == 0;
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex into out. Every literal here is valid hex.
unhex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0 && r.written == out.count;
}
dn_same :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
// The x-only public key of a secret: x(k·G).
public_x :: (secret: [] u8) -> [32] u8 {
  k := sc_reduce_bytes(secret).value;
  a := sg_to_affine(smc_multiply(k, sg_generator()).value).value;
  b: [32] u8;
  #assert sf_store(a.x, b[..]);
  return b;
}
// BIP-340 signing with Bitcoin chapter 24's signer, run until it stops asking for more.
sign :: (key: [] u8, aux: [] u8, message: [] u8, out: [] u8) -> bool {
  work: [1] SignWork;
  status := SIGN_MORE;
  while status == SIGN_MORE { status = schnorr_sign_step(key, aux, message, work[..], out, false); }
  return status == SIGN_DONE;
}
```

<!-- source: nostr/examples/dev-nips.coax main -->
```jai
GS_PAYLOAD :: "tree 4b825dc642cb6eb9a060e54bf899d69f7cb46101\nauthor Test User <test@example.com> 1700000000 +0000\ncommitter Test User <test@example.com> 1700000000 +0000\n\nInitial commit";
GS_SIG :: "c35062148d95b820068c18ab9cf69a8dd2322c606890366d084df7617570b96b7a1aca0a8fcabb2eb4032ebbdf5b43e6bf8633e0d85bcecce28a9e08705b875f";
GS_B64 :: "eyJ2IjoxLCJwayI6ImY5MzA4YTAxOTI1OGMzMTA0OTM0NGY4NWY4OWQ1MjI5YjUzMWM4NDU4MzZmOTliMDg2MDFmMTEzYmNlMDM2ZjkiLCJzaWciOiJjMzUwNjIxNDhkOTViODIwMDY4YzE4YWI5Y2Y2OWE4ZGQyMzIyYzYwNjg5MDM2NmQwODRkZjc2MTc1NzBiOTZiN2ExYWNhMGE4ZmNhYmIyZWI0MDMyZWJiZGY1YjQzZTZiZjg2MzNlMGQ4NWJjZWNjZTI4YTllMDg3MDViODc1ZiIsInQiOjE3MDAwMDAwMDB9";
OA_SIG :: "54b97dfd2b7d61c1bc1b5facab9d12a991fe0ac3dcb9044b3176f63bebb6f67340eb0ad866f2d5568b78b58ba234ee9f490f8c41e64a949c200315801520ed25";
GS_OA_SIG :: "15592857980b8656ff50303d86acaffcbda397b9c0bb40aebd2fb87a723e466fdb1a74404d39f9eb7ac220b4f2e061f27523f1af24cbdf991cf42ff9b47034c0";
ALICE :: "30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:buzz";
ALICE_KEY :: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
BOB_KEY :: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
RS_PLAIN :: "{\"v\":1,\"client_id\":\"test-vector-client\",\"contexts\":{\"group:general\":1700001000,\"group:dev\":1700000500}}";
RS_CIPHER :: "Akt10yui5aDIjfH+xED2Dr1NJ/SGWp85SC/r/bloiLRtj8K59rJrYhcfsNQMoMhpLlvhKqrN0HIGb9/V9BcYKxWV8HT/jjDdvfHLUVfo688I6WpapcX41GzL4VnGGDdFyUom53odJncjHszS3dpTrG1OKp2x9dtdG+924/+Ne49KN4nztd1pikqYeqQuxflKCmh+VcCFbDclQ8a9NUpqWkPpeoweISVVuZDnP9WFoKG5X6YcpXBWH6wjc69xK4cs6KkJ";

show_fold :: (label: string, c: MpCollection) {
  print("%:", label);
  for i: 0..<c.count {
    print(" [%:", c.containers[i].project[71..<c.containers[i].project.count]);
    for m: 0..<c.containers[i].count {
      x := c.containers[i].members[m];
      print(" %", x.coordinate[71..<x.coordinate.count]);
      if !x.resolved { print(" (unavailable)"); }
    }
    print("]");
  }
  print(" cards:");
  for i: 0..<c.card_count { print(" %", c.cards[i][71..<c.cards[i].count]); }
  print("\n");
}
show_projection :: (label: string, ok: bool) { print("%: %\n", label, ifx ok then "consistent" else "refused"); }

main :: () {
  // 1. NIP-GS: the published test key 3 and the 170-byte commit.
  key: [32] u8; unhex("0000000000000000000000000000000000000000000000000000000000000003", key[..]);
  pk := public_x(key[..]);
  pk_hex: [64] u8; #assert gsa_hex(pk_hex[..], 0, pk[..]) == 64;
  show("agent public key", pk[..]);
  print("payload: % bytes\n", GS_PAYLOAD.count);
  none: GsOwner;
  hash: [32] u8;
  print("preimage: % bytes\n", gs_hash(1700000000, none, GS_PAYLOAD, hash[..]));
  show("signing hash", hash[..]);
  zero: [32] u8; sig: [64] u8;
  #assert sign(key[..], zero[..], hash[..], sig[..]);
  want: [64] u8; unhex(GS_SIG, want[..]);
  print("signature with aux = 0 equals the NIP's: %\n", dn_same(sig[..], want[..]));
  armor: [GSA_ARMOR_MAX] u8;
  n := gs_armor(pk[..], sig[..], 1700000000, none, armor[..]);
  body := cast(string) armor[GSA_BEGIN.count..<n - GSA_END.count];
  print("armor: % bytes, base64 line equals the NIP's: %\n", n, body == GS_B64);
  print("[GNUPG:] SIG_CREATED D 8 1 00 1700000000 %\n", cast(string) pk_hex[..]);
  gs_status(schnorr_verify_vartime(hash[..], pk[..], sig[..]), cast(string) pk_hex[..], 1700000000, true);

  // The owner attestation: owner key 1 authorizes agent key 3, no conditions.
  owner_key: [32] u8; unhex("0000000000000000000000000000000000000000000000000000000000000001", owner_key[..]);
  owner_pk := public_x(owner_key[..]);
  owner_hex: [64] u8; #assert gsa_hex(owner_hex[..], 0, owner_pk[..]) == 64;
  oa: [32] u8; print("NIP-OA preimage: % bytes\n", oa_hash(cast(string) pk_hex[..], "", oa[..]));
  show("NIP-OA hash", oa[..]);
  oa_sig: [64] u8; #assert sign(owner_key[..], zero[..], oa[..], oa_sig[..]);
  oa_sig_hex: [128] u8; #assert gsa_hex(oa_sig_hex[..], 0, oa_sig[..]) == 128;
  print("owner signature equals the NIP's: %\n", cast(string) oa_sig_hex[..] == OA_SIG);
  owner := GsOwner.{present = true, key = cast(string) owner_hex[..], conditions = "", signature = cast(string) oa_sig_hex[..]};
  owned: [32] u8; print("preimage with oa: % bytes\n", gs_hash(1700000000, owner, GS_PAYLOAD, owned[..]));
  show("signing hash with oa", owned[..]);
  owned_sig: [64] u8; #assert sign(key[..], zero[..], owned[..], owned_sig[..]);
  owned_want: [64] u8; unhex(GS_OA_SIG, owned_want[..]);
  print("signature with oa equals the NIP's: %\n", dn_same(owned_sig[..], owned_want[..]));
  oa_armor: [GSA_ARMOR_MAX] u8;
  print("armor with oa: % bytes\n", gs_armor(pk[..], owned_sig[..], 1700000000, owner, oa_armor[..]));

  // Refusals: one payload byte changed, and the oa stripped from a signature
  // made with it. Both verify as BADSIG.
  edited: [256] u8; for i: 0..<GS_PAYLOAD.count { edited[i] = GS_PAYLOAD[i]; }
  edited[GS_PAYLOAD.count - 1] = 'h';  // "Initial commih"
  bad: [32] u8; #assert 194 == gs_hash(1700000000, none, cast(string) edited[0..<GS_PAYLOAD.count], bad[..]);
  print("payload edited:\n");
  gs_status(schnorr_verify_vartime(bad[..], pk[..], sig[..]), cast(string) pk_hex[..], 1700000000, true);
  print("oa stripped:\n");
  gs_status(schnorr_verify_vartime(hash[..], pk[..], owned_sig[..]), cast(string) pk_hex[..], 1700000000, true);

  // The projection over protobuf: the NIP's vector, and two spellings it refuses.
  a := GS_FIX_OK; show_projection("GS projection, NIP vector", gsa_read(a[..]).consistent);
  b := GS_FIX_SPACE; show_projection("GS projection, a space in the JSON", gsa_read(b[..]).consistent);
  c := GS_FIX_SELF_OWNER; show_projection("GS projection, oa[0] equals pk", gsa_read(c[..]).consistent);

  // 2. NIP-MP ingest: three accepted and three refused tag lists.
  full := [7] MpTag.[mp_tag("d", "platform"), mp_tag("name", "Platform"), mp_tag("description", "Relay, desktop, and mobile for the platform team."),
                     mp_tag("a", ALICE), mp_tag("a", "30617:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:buzz-infra"),
                     mp_tag("buzz-channel", "3580ca9b-47b4-4af9-b22a-1068778f26c6"), mp_tag("buzz-visibility", "listed")];
  print("valid_full: %\n", mp_ingest(full[..]));
  colon := [2] MpTag.[mp_tag("d", "platform"), mp_tag("a", "30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:buzz:infra")];
  print("valid_member_dtag_contains_colon: %\n", mp_ingest(colon[..]));
  wide: [66] MpTag; wide[0] = mp_tag("d", "wide");
  names: [65] [80] u8;
  for i: 0..<65 {
    at := gsa_put(names[i][..], 0, "30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:repo-");
    names[i][at] = cast(u8) ('0' + i / 10); names[i][at + 1] = cast(u8) ('0' + i % 10);
    wide[i + 1] = mp_tag("a", cast(string) names[i][0..<at + 2]);
  }
  print("valid_member_cap_boundary (64 members): %\n", mp_ingest(wide[0..<65]));
  print("invalid_member_cap_exceeded (65 members): %\n", mp_ingest(wide[..]));
  arity := [2] MpTag.[mp_tag("d", "platform"), mp_tag4("a", ALICE, "wss://relay.example.com", "unexpected")];
  print("invalid_member_tag_arity_four_elements: %\n", mp_ingest(arity[..]));
  hints := [3] MpTag.[mp_tag("d", "platform"), mp_tag3("a", ALICE, "wss://relay-one.example.com"), mp_tag3("a", ALICE, "wss://relay-two.example.com")];
  print("invalid_member_duplicate_differing_relay_hints: %\n", mp_ingest(hints[..]));
  d := MP_FIX_OK; show_projection("MP projection, matching original", mpa_read(d[..]).consistent);
  e := MP_FIX_BAD_D; show_projection("MP projection, d differs from the original", mpa_read(e[..]).consistent);

  // 3. NIP-MP fold: Alice's repository in Alice's and Bob's projects.
  repos := [1] MpRepo.[mp_repo(ALICE)];
  mine := mp_project("30621:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:platform", ALICE_KEY);
  mine = with_member(mine, ALICE);
  theirs := mp_project("30621:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:borrowed", BOB_KEY);
  theirs = with_member(theirs, ALICE);
  only_bob := [1] MpProject.[theirs];
  show_fold("stranger only", mp_fold(repos[..], only_bob[..]));
  both := [2] MpProject.[mine, theirs];
  show_fold("owner and stranger", mp_fold(repos[..], both[..]));
  mine.unlisted = true;
  unlisted := [1] MpProject.[mine];
  show_fold("owner's project unlisted", mp_fold(repos[..], unlisted[..]));
  ghost := mp_project("30621:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:platform", ALICE_KEY);
  ghost = with_member(ghost, "30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:ghost");
  ghosts := [1] MpProject.[ghost]; no_repos: [1] MpRepo;
  show_fold("member resolves to nothing", mp_fold(no_repos[0..<0], ghosts[..]));

  // Enumeration in pages of 2: three heads at 1000, two at 900.
  heads := [5] MpHead.[MpHead.{created_at = 1000, coordinate = ALICE},
                       MpHead.{created_at = 1000, coordinate = "30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:buzz-infra"},
                       MpHead.{created_at = 1000, coordinate = "30617:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:buzz-mobile"},
                       MpHead.{created_at = 900, coordinate = "30621:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:platform"},
                       MpHead.{created_at = 900, coordinate = "30621:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:mobile"}];
  got: [8] MpHead;
  print("composite cursor: % of 5 heads\n", mp_enumerate(heads[..], 2, true, got[..]));
  m := mp_enumerate(heads[..], 2, false, got[..]);
  print("until only, stepping to oldest - 1: % of 5 heads:", m);
  for i: 0..<m { print(" %", got[i].coordinate[71..<got[i].coordinate.count]); }
  print("\n");

  // 4. NIP-PL: a fixed clock, 15 minutes of skew, 30 days of TTL.
  now := 1769990000; skew := 900; ttl := 2592000;
  print("lease window: (%, %]\n", now - skew, now + ttl);
  exps := [5] int.[now - skew, now - skew + 1, now + 3600, now + ttl, now + ttl + 1];
  for i: 0..<5 { print("expiration %: %\n", exps[i], pl_window(now, exps[i], skew, ttl)); }
  print("newer created_at, generation 4 over 3: %\n", pl_replace(100, "bb", 3, 101, "cc", 4));
  print("same created_at, lower id: %\n", pl_replace(100, "bb", 3, 100, "aa", 4));
  print("same created_at, higher id: %\n", pl_replace(100, "bb", 3, 100, "cc", 4));
  print("newer created_at, generation 3 again: %\n", pl_replace(100, "bb", 3, 101, "cc", 3));
  f := PL_FIX_OK; show_projection("PL projection, d, expiration, exec and alt", pla_read(f[..]).consistent);
  g := PL_FIX_LEADING_ZERO; show_projection("PL projection, expiration with a leading zero", pla_read(g[..]).consistent);
  h := PL_FIX_EXTRA_TAG; show_projection("PL projection, an extra public tag", pla_read(h[..]).consistent);

  // 5. NIP-RS: devices A and B, merged.
  da: RsState; da = rs_put(da, "group:general", 1700001000); da = rs_put(da, "group:dev", 1700000500);
  db: RsState; db = rs_put(db, "group:general", 1700001200); db = rs_put(db, "group:random", 1700000800);
  ab := rs_merge(da, db); ba := rs_merge(db, da);
  print("merged:");
  for i: 0..<ab.count { print(" % %", ab.entries[i].context, ab.entries[i].at); }
  print("\n");
  print("merge(B, A) equals merge(A, B): %\n", rs_same(ab, ba));
  print("merge(AB, AB) equals AB: %\n", rs_same(rs_merge(ab, ab), ab));
  // The ciphertext vector: key 1 encrypts to itself.
  k1: [32] u8; unhex("0000000000000000000000000000000000000000000000000000000000000001", k1[..]);
  p1 := public_x(k1[..]);
  scratch: [1] EcdhScratch; ck: [32] u8;
  #assert nip44_conversation_key_from_keys_checked(k1[..], p1[..], scratch[..], ck[..], false) == NIP44_OK;
  show("conversation key with itself", ck[..]);
  work: [512] u8; plain: [256] u8;
  r := nip44_decrypt_checked(NIP44_COMPAT, ck[..], RS_CIPHER, work[..], plain[..], false);
  print("decrypt: ok %, % bytes, equals the NIP's plaintext: %\n", r.error == NIP44_OK, r.written, cast(string) plain[0..<r.written] == RS_PLAIN);
  wrong := ck; wrong[0] ^= 1;
  r2 := nip44_decrypt_checked(NIP44_COMPAT, wrong[..], RS_CIPHER, work[..], plain[..], false);
  print("decrypt with one key bit flipped: refused %\n", r2.error == NIP44_MAC);
  print("own slot holds client-B, I am client-A: may publish %\n", rs_may_publish("client-A", "client-B"));
  print("created_at, clock 1700001200, fetched up to 1700001500: %\n", rs_created_at(1700001200, 1700001500));
  print("created_at, clock 1700002000, fetched up to 1700001500: %\n", rs_created_at(1700002000, 1700001500));
  i := RS_FIX_OK; show_projection("RS projection, one d and one read-state t", rsa_read(i[..]).consistent);
  j := RS_FIX_UPPER; show_projection("RS projection, uppercase slot", rsa_read(j[..]).consistent);
  k := RS_FIX_TWO_T; show_projection("RS projection, two read-state t tags", rsa_read(k[..]).consistent);
}
```

It prints:

```text
agent public key: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
payload: 170 bytes
preimage: 194 bytes
signing hash: a11a32173aa35125aaefaad8854f2eda5a144268a4a355905c841f79ff44aa18
signature with aux = 0 equals the NIP's: true
armor: 369 bytes, base64 line equals the NIP's: true
[GNUPG:] SIG_CREATED D 8 1 00 1700000000 f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
[GNUPG:] NEWSIG
[GNUPG:] GOODSIG f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9 f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
[GNUPG:] VALIDSIG f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9 2023-11-14 1700000000 0 - - - - - f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
[GNUPG:] TRUST_FULLY 0 shell
NIP-OA preimage: 82 bytes
NIP-OA hash: 05113b24677b87bedf6498a3addad720003e6af36820e859a26814f149f5a837
owner signature equals the NIP's: true
preimage with oa: 389 bytes
signing hash with oa: b61f1658836a4f63a2d2f5d621014a064435dde0765dd9c1dc79c9530fe879f0
signature with oa equals the NIP's: true
armor with oa: 649 bytes
payload edited:
[GNUPG:] NEWSIG
[GNUPG:] BADSIG f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9 f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
oa stripped:
[GNUPG:] NEWSIG
[GNUPG:] BADSIG f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9 f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
GS projection, NIP vector: consistent
GS projection, a space in the JSON: refused
GS projection, oa[0] equals pk: refused
valid_full: accept
valid_member_dtag_contains_colon: accept
valid_member_cap_boundary (64 members): accept
invalid_member_cap_exceeded (65 members): member-cap
invalid_member_tag_arity_four_elements: member-tag-arity
invalid_member_duplicate_differing_relay_hints: member-duplicate
MP projection, matching original: consistent
MP projection, d differs from the original: refused
stranger only: [borrowed: buzz] cards: buzz
owner and stranger: [platform: buzz] [borrowed: buzz] cards:
owner's project unlisted: cards: buzz
member resolves to nothing: [platform: ghost (unavailable)] cards:
composite cursor: 5 of 5 heads
until only, stepping to oldest - 1: 4 of 5 heads: buzz buzz-infra platform mobile
lease window: (1769989100, 1772582000]
expiration 1769989100: invalid: lease already expired
expiration 1769989101: ok
expiration 1769993600: ok
expiration 1772582000: ok
expiration 1772582001: invalid: lease ttl too long
newer created_at, generation 4 over 3: ok
same created_at, lower id: ok
same created_at, higher id: invalid: stale replacement
newer created_at, generation 3 again: invalid: stale generation
PL projection, d, expiration, exec and alt: consistent
PL projection, expiration with a leading zero: refused
PL projection, an extra public tag: refused
merged: group:general 1700001200 group:dev 1700000500 group:random 1700000800
merge(B, A) equals merge(A, B): true
merge(AB, AB) equals AB: true
conversation key with itself: 3b4610cb7189beb9cc29eb3716ecc6102f1247e8f3101a03a1787d8908aeb54e
decrypt: ok true, 103 bytes, equals the NIP's plaintext: true
decrypt with one key bit flipped: refused true
own slot holds client-B, I am client-A: may publish false
created_at, clock 1700001200, fetched up to 1700001500: 1700001501
created_at, clock 1700002000, fetched up to 1700001500: 1700002000
RS projection, one d and one read-state t: consistent
RS projection, uppercase slot: refused
RS projection, two read-state t tags: refused
```

The values match the worked examples above: the 194- and 389-byte preimages,
the 231-byte JSON as a 369-byte armor, the window `(1769989100, 1772582000]`
and the merged table.

The refusals:

- **NIP-GS.** One payload byte changed (`Initial commih`) gives a different
  hash, and the signature is `BADSIG`. The attested signature checked without
  its `oa` binding is `BADSIG` too: removing the attestation breaks the commit
  signature. The projection refuses a space in the JSON and an attestation
  whose owner key equals `pk`.
- **NIP-MP.** 65 members are `member-cap`; a fourth tag element is
  `member-tag-arity`; one coordinate with two relay hints is
  `member-duplicate`. The projection refuses a `d` that differs from the
  original. Paging by `until` alone gathers 4 of 5 heads.
- **NIP-PL.** The two window edges and the two stale replacements. The
  projection refuses an expiration with a leading zero and an extra public
  tag.
- **NIP-RS.** One flipped bit of the conversation key fails the NIP-44 MAC.
  `client-A` may not publish over `client-B`'s blob. The projection refuses
  an uppercase slot and two read-state `t` tags.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/dev-nips.out](../fixtures/dev-nips.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/dev-nips.coax](../examples/checks/dev-nips.coax),
which loads the example. They compare with data that shares no code with it:

- both NIP-GS vectors: the published signatures verify with Bitcoin chapter 23's
  verifier over `gs_hash`, and the armor `gs_armor` builds equals byte for
  byte the armor in the protoc-encoded fixtures `GS_FIX_OK` and
  `GS_FIX_OWNER`, which `generate_gs_signature_fixtures.py` builds from the
  NIP's own JSON and base64;
- `VALIDSIG` dates against Python's `datetime` for eight times, including
  2000-02-29, 2100-02-28 and `t = 4294967295`;
- every case of
  [NIP-MP.fixtures.json](../nips/block/NIP-MP.fixtures.json): the 11
  accepted and 20 refused templates, each refused one for the rule it names;
- every case of
  [NIP-MP.fold-fixtures.json](../nips/block/NIP-MP.fold-fixtures.json),
  compared as sets as the file requires; the pagination case is served in
  pages of 2 and gathered with the composite cursor before folding;
- the NIP-PL window over every `now` in 1000..1039 and `expiration` in
  950..2099 against the inequality itself;
- the NIP-RS merged state, commutativity, idempotence and associativity, the
  larger of two timestamps for every pair below 20, the `created_at` rule
  and the ciphertext vector.

The fixture cases were transcribed mechanically from the two JSON files.

<!-- source: nostr/examples/checks/dev-nips.coax check -->
```jai
// Fold results compare as sets: the containers, each container's members
// (with their render state) and the implicit cards.
same_members :: (a: MpContainer, b: MpContainer) -> bool {
  if a.project != b.project || a.count != b.count { return false; }
  for i: 0..<a.count {
    found := false;
    for j: 0..<b.count { if a.members[i].coordinate == b.members[j].coordinate && a.members[i].resolved == b.members[j].resolved { found = true; } }
    if !found { return false; }
  }
  return true;
}
same_collection :: (got: MpCollection, want: MpCollection) -> bool {
  if got.count != want.count || got.card_count != want.card_count { return false; }
  for i: 0..<want.count {
    found := false;
    for j: 0..<got.count { if same_members(got.containers[j], want.containers[i]) { found = true; } }
    if !found { return false; }
  }
  for i: 0..<want.card_count {
    found := false;
    for j: 0..<got.card_count { if got.cards[j] == want.cards[i] { found = true; } }
    if !found { return false; }
  }
  return true;
}
// The pagination fixture: serve every head sorted by (created_at descending,
// coordinate ascending) in pages, gather them with the composite cursor, and
// count how often each repository and project arrived. The fold then reads only
// those.
Seen :: struct { repo: [8] int; project: [8] int; pages: int; heads: int; }
paged :: (repos: [] MpRepo, projects: [] MpProject, size: int) -> Seen {
  heads: [16] MpHead; n := 0;
  for i: 0..<repos.count { heads[n] = MpHead.{created_at = repos[i].created_at, coordinate = repos[i].coordinate}; n += 1; }
  for i: 0..<projects.count { heads[n] = MpHead.{created_at = projects[i].created_at, coordinate = projects[i].coordinate}; n += 1; }
  for i: 1..<n {
    j := i;
    while j > 0 && mp_before(heads[j], heads[j - 1]) { t := heads[j]; heads[j] = heads[j - 1]; heads[j - 1] = t; j -= 1; }
  }
  // The first page: everything at or below the newest second, after "".
  s: Seen; until := heads[0].created_at; after := "";
  while true {
    page: [8] MpHead;
    got := mp_page(heads[0..<n], size, true, until, after, page[..]);
    s.pages += 1; s.heads += got;
    for k: 0..<got {
      for i: 0..<repos.count { if repos[i].coordinate == page[k].coordinate { s.repo[i] += 1; } }
      for i: 0..<projects.count { if projects[i].coordinate == page[k].coordinate { s.project[i] += 1; } }
    }
    if got < size { break; }
    until = page[got - 1].created_at; after = page[got - 1].coordinate;
  }
  return s;
}

#test "GS: both NIP vectors, signed again and checked through the projection" #steps 200000000 {
  // The second vector's full JSON from the NIP, armored by the projection's own writers.
  owner := GsOwner.{present = true, key = "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", conditions = "", signature = OA_SIG};
  pk: [32] u8; unhex("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9", pk[..]);
  sig: [64] u8; unhex(GS_OA_SIG, sig[..]);
  hash: [32] u8; #assert gs_hash(1700000000, owner, GS_PAYLOAD, hash[..]) == 13 + 10 + 1 + 64 + 1 + 0 + 1 + 128 + 1 + 170;
  #assert schnorr_verify_vartime(hash[..], pk[..], sig[..]);
  oa: [32] u8; #assert oa_hash("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9", "", oa[..]) == 82;
  owner_pk: [32] u8; unhex(owner.key, owner_pk[..]);
  osig: [64] u8; unhex(OA_SIG, osig[..]);
  #assert schnorr_verify_vartime(oa[..], owner_pk[..], osig[..]);
  // The armor our writer builds is the one the protoc fixture carries.
  armor: [GSA_ARMOR_MAX] u8; n := gs_armor(pk[..], sig[..], 1700000000, owner, armor[..]);
  fixture := GS_FIX_OWNER; r := gsa_read(fixture[..]);
  #assert r.consistent && r.owner && dn_same(r.armor, armor[0..<n]);
  plain := GS_FIX_OK; p := gsa_read(plain[..]);
  none: GsOwner; sig1: [64] u8; unhex(GS_SIG, sig1[..]);
  m := gs_armor(pk[..], sig1[..], 1700000000, none, armor[..]);
  #assert p.consistent && dn_same(p.armor, armor[0..<m]);
  // t = 0 and the largest t spell as "0" and "4294967295".
  #assert gs_hash(0, none, "", hash[..]) == 15 && gs_hash(4294967295, none, "", hash[..]) == 24;
}
#test "GS VALIDSIG dates, against Python's datetime" {
  d: [10] u8;
  #assert gs_date(1700000000, d[..]) == "2023-11-14" && gs_date(0, d[..]) == "1970-01-01";
  #assert gs_date(951782400, d[..]) == "2000-02-29" && gs_date(951868800, d[..]) == "2000-03-01";
  #assert gs_date(4107456000, d[..]) == "2100-02-28" && gs_date(4107542400, d[..]) == "2100-03-01";
  #assert gs_date(4294967295, d[..]) == "2106-02-07" && gs_date(1704067199, d[..]) == "2023-12-31";
}
#test "MP coordinates split on the first two colons" {
  #assert mp_coordinate(ALICE) && mp_coordinate("30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:a:b:c");
  #assert !mp_coordinate("30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:");
  #assert !mp_coordinate("30618:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:buzz");
  #assert !mp_coordinate("30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:buzz");
  #assert !mp_coordinate("30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaG:buzz");
  #assert !mp_coordinate("30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/:buzz");
  #assert !mp_coordinate("30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`:buzz");
}
#test "MP until-only paging loses a tied head; the composite cursor does not" {
  heads := [5] MpHead.[MpHead.{created_at = 1000, coordinate = "a"}, MpHead.{created_at = 1000, coordinate = "b"},
                       MpHead.{created_at = 1000, coordinate = "c"}, MpHead.{created_at = 900, coordinate = "d"},
                       MpHead.{created_at = 900, coordinate = "e"}];
  got: [8] MpHead;
  #assert mp_enumerate(heads[..], 2, true, got[..]) == 5 && got[2].coordinate == "c";
  #assert mp_enumerate(heads[..], 2, false, got[..]) == 4 && got[2].coordinate == "d";
  #assert mp_enumerate(heads[..], 5, false, got[..]) == 5;
}
#test "PL window edges and replacement order" {
  #assert pl_replace(5, "b", 1, 5, "b", 2) == "invalid: stale replacement";
  #assert pl_replace(5, "b", 1, 4, "a", 9) == "invalid: stale replacement";
  #assert pl_replace(5, "ab", 1, 5, "aa", 2) == "ok";
  #assert pl_replace(5, "b", 7, 6, "c", 7) == "invalid: stale generation";
}
#test "RS: the NIP's merged state, and the merge is a join" {
  a: RsState; a = rs_put(a, "group:general", 1700001000); a = rs_put(a, "group:dev", 1700000500);
  b: RsState; b = rs_put(b, "group:general", 1700001200); b = rs_put(b, "group:random", 1700000800);
  ab := rs_merge(a, b); ba := rs_merge(b, a);
  #assert rs_get(ab, "group:general") == 1700001200 && rs_get(ab, "group:dev") == 1700000500 && rs_get(ab, "group:random") == 1700000800 && ab.count == 3;
  #assert rs_same(ab, ba) && rs_same(rs_merge(ab, ab), ab) && rs_same(rs_merge(rs_merge(a, b), a), rs_merge(a, rs_merge(b, a)));
  #assert rs_created_at(1700001200, 1700001500) == 1700001501 && rs_created_at(1700001500, 1700001500) == 1700001501 && rs_created_at(1700001501, 1700001500) == 1700001501;
  #assert !rs_may_publish("client-A", "client-B") && rs_may_publish("client-A", "client-A");
}
#forall "PL accepts exactly now - skew < expiration <= now + max_ttl" now: 1000..<1040, e: 950..<2100 {
  #assert (pl_window(now, e, 15, 1000) == "ok") == (now - 15 < e && e <= now + 1000);
}
#forall "RS keeps the larger of two timestamps for one context, in either order" x: 0..<20, y: 0..<20 {
  s: RsState; s = rs_put(s, "c", x); s = rs_put(s, "c", y);
  t: RsState; t = rs_put(t, "c", y); t = rs_put(t, "c", x);
  #assert rs_get(s, "c") == (ifx x > y then x else y) && rs_same(s, t);
}
#test "RS: the ciphertext vector decrypts with key 1 to itself" #steps 200000000 {
  k1: [32] u8; unhex("0000000000000000000000000000000000000000000000000000000000000001", k1[..]);
  p1 := public_x(k1[..]);
  scratch: [1] EcdhScratch; ck: [32] u8;
  #assert nip44_conversation_key_from_keys_checked(k1[..], p1[..], scratch[..], ck[..], false) == NIP44_OK;
  work: [512] u8; plain: [256] u8;
  r := nip44_decrypt_checked(NIP44_COMPAT, ck[..], RS_CIPHER, work[..], plain[..], false);
  #assert r.error == NIP44_OK && cast(string) plain[0..<r.written] == RS_PLAIN;
}

#test "MP the comparison helpers tell collections apart" {
  a: MpContainer; a.project = "p"; a.count = 2;
  a.members[0] = MpPlaced.{coordinate = "x", resolved = true}; a.members[1] = MpPlaced.{coordinate = "y", resolved = false};
  b := a; b.members[0] = a.members[1]; b.members[1] = a.members[0];
  #assert same_members(a, b);
  c := a; c.project = "q"; #assert !same_members(a, c);
  c = a; c.count = 1; #assert !same_members(a, c);
  c = a; c.members[1].resolved = true; #assert !same_members(a, c);
  c = a; c.members[1].coordinate = "z"; #assert !same_members(a, c);
  c = a; c.members[0].coordinate = "z"; #assert !same_members(c, a);
  c = a; c.members[0].resolved = false; #assert !same_members(c, a);
  w: MpCollection; w.count = 2; w.containers[0] = a; w.containers[1] = a; w.containers[1].project = "q";
  w.card_count = 2; w.cards[0] = "x"; w.cards[1] = "y";
  g := w; g.containers[0] = w.containers[1]; g.containers[1] = w.containers[0]; g.cards[0] = "y"; g.cards[1] = "x";
  #assert same_collection(g, w);
  g = w; g.count = 1; #assert !same_collection(g, w);
  g = w; g.count = 3; g.containers[2] = a; #assert !same_collection(g, w);
  g = w; g.card_count = 1; #assert !same_collection(g, w);
  g = w; g.card_count = 3; g.cards[2] = "x"; #assert !same_collection(g, w);
  g = w; g.containers[0].project = "r"; #assert !same_collection(g, w);
  g = w; g.cards[0] = "z"; #assert !same_collection(g, w);
  g = w; g.containers[1].project = "r"; #assert !same_collection(g, w);
  g = w; g.cards[1] = "z"; #assert !same_collection(g, w);
}
#test "MP paging sorts what it serves and sees every head once" {
  repos: [3] MpRepo;
  repos[0] = mp_repo("30617:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:buzz-mobile"); repos[0].created_at = 1000;
  repos[1] = mp_repo("30617:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:old"); repos[1].created_at = 800;
  repos[2] = mp_repo(ALICE); repos[2].created_at = 1000;
  projects: [2] MpProject;
  projects[0] = mp_project("30621:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb:mobile", BOB_KEY); projects[0].created_at = 900;
  projects[1] = mp_project("30621:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:platform", ALICE_KEY); projects[1].created_at = 1100;
  seen := paged(repos[..], projects[..], 2);
  #assert seen.pages == 3 && seen.heads == 5;
  for i: 0..<3 { #assert seen.repo[i] == 1; }
  for i: 0..<2 { #assert seen.project[i] == 1; }
  one := paged(repos[..], projects[..], 1);
  #assert one.pages == 6 && one.heads == 5;
  // The first two out of order, and nothing later to carry them past each other.
  pair := [2] MpRepo.[repos[1], repos[2]];
  two := paged(pair[..], projects[0..<0], 1);
  #assert two.heads == 2 && two.repo[0] == 1 && two.repo[1] == 1;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/dev-nips.coax --run-steps 250000000
coax/bin/coax mutate nostr/examples/checks/dev-nips.coax --root-only
```

On 2026-11-01, `coax test` passed 501 checks, including the loaded files'
own checks (the four projections, the event projection and the signer), and
the root-only mutation run killed 93 of 93 mutants in the checks' helpers.
The book's checks ran the example in debug and release.

### The library's records

These ran when the projection files were written and were not repeated for
this chapter, because those files did not change:

- [2026-10-31-nostr-block-gs.md](../../docs/verification/2026-10-31-nostr-block-gs.md):
  `gs-projection.coax` passed 8 checks, root-only mutation killed 214 of
  214, and `block_git_signature.py` ran 48 cases through the generated
  reader, checked and release, native ASan/UBSan and WASI.
- [2026-10-29-nostr-block-mp.md](../../docs/verification/2026-10-29-nostr-block-mp.md):
  the MP codec and `block_project.py` in checked and release, native
  ASan/UBSan and WASI.
- [2026-10-31-nostr-block-pl-rs.md](../../docs/verification/2026-10-31-nostr-block-pl-rs.md):
  202 checks each, root-only mutation PL 95 of 95 and RS 69 of 69, and
  `block_pl_rs.py` 35 PL and 22 RS cases in the same four builds.

Full-reference mutation, with the generated protobuf readers included, has
not been run for GS, PL or RS.

## Summary

- NIP-GS signs `SHA-256("nostr:git:v1:" || t || ":" || oa_binding ||
  payload)` with BIP-340, and the armored JSON has exactly one valid
  spelling. Bitcoin chapter 24's signer reproduces both published signatures.
- NIP-MP's relay refuses eight kinds of malformed project; the client fold
  lets only eligible, authorized projects claim a repository, so a stranger
  cannot remove it from where its owner expects it. Paging needs a composite
  cursor.
- NIP-PL leases are accepted only inside `now − skew < expiration ≤ now +
  ttl`, and a replacement must win both NIP-01 ordering and the generation
  watermark.
- NIP-RS merges read state with a per-context maximum and never publishes at
  or below a fetched `created_at`.
- The four projections check envelopes against their originals; they are not
  signature checks.

## Dependencies

- [63. Git on Nostr](ch063-git-on-nostr.md)
- [Bitcoin chapter 24. Sign with Schnorr](../../bitcoin/book/ch024-sign-with-schnorr.md)

## Sources

- [Block NIP-GS](../nips/block/NIP-GS.md), Test Vectors and Invalid Cases
- [Block NIP-MP](../nips/block/NIP-MP.md), with
  [NIP-MP.fixtures.json](../nips/block/NIP-MP.fixtures.json) and
  [NIP-MP.fold-fixtures.json](../nips/block/NIP-MP.fold-fixtures.json)
- [Block NIP-PL](../nips/block/NIP-PL.md)
- [Block NIP-RS](../nips/block/NIP-RS.md), Test Vectors
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
