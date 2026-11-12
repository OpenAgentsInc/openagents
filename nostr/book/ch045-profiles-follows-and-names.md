# 45. Profiles, follows and names

A person on Nostr is a key. Three kinds of data hang off that key: a kind 0
profile, a kind 3 follow list, and an optional NIP-05 name that a domain can
vouch for. This chapter reads all three, offline, and shows that a newer
follow list replaces an older one in the store. It then reads a fourth: a
NIP-39 kind 10011 list of the accounts the key says it controls on other
platforms, with the URL where each claim's proof is published.

## What this chapter teaches

- Kind 0 content: a JSON object inside a string, with `name`, `about`,
  `picture` and, from NIP-05, `nip05`.
- Kind 3: `p` tags of the form `["p", key, relay, petname]`. Kind 3 is
  replaceable, so the newest list is the whole set.
- NIP-05: `name@domain` maps to
  `https://domain/.well-known/nostr.json?name=name`. A response counts only
  when its status is 200, never a redirect, and its `names` object maps the
  name to the event's pubkey in lowercase hex.
- NIP-39: a kind 10011 event lists accounts on other platforms in `i`
  tags, `["i", "platform:identity", "proof"]`. A claim counts only after a
  client fetches the proof from the platform and finds the key's `npub`
  in it.
- None of these proves who a person is. A profile is what the key says about
  itself, a follow is a choice the signer published, and a NIP-05 match says
  only that the domain names this key today.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [metadata.coax](../../coax/lib/nostr/metadata.coax) | `nostr_metadata_parse`, `nostr_metadata_read` | Reads kind 0 `name`, `about` and `picture` (chapter 22) |
| [nip05.coax](../../coax/lib/nostr/nip05.coax) | `nip05_parse`, `nip05_url`, `nip05_from_metadata`, `nip05_check` | Splits an identifier, writes its URL, reads `nip05` from kind 0 content, checks a response |
| [follows.coax](../../coax/lib/nostr/follows.coax) | `nostr_follow` | Reads one tag of a kind 3 event as a follow |
| [store_core.coax](../../nostr/relay/store_core.coax) | `ns_store_admit` | Stores verified events and replaces replaceable ones by chapter 22's rule |
| [nip39.coax](../../coax/lib/nostr/nip39.coax) | `n39_claims(e, out)` | Reads the `i` tags of a kind 10011 event as platform, identity and proof |
| [nip39.coax](../../coax/lib/nostr/nip39.coax) | `n39_proof_url(e, c, out)` | The URL where a claim's proof is published, for the six platforms NIP-39 defines |
| [nip39.coax](../../coax/lib/nostr/nip39.coax) | `n39_text(npub, out)`, `n39_proves(body, npub)` | The verification text, and whether a fetched proof holds the key's `npub` |

`nip05.coax`, `follows.coax` and `nip39.coax` are new in this chapter. All
three are `#authority` files. `nip39.coax` reads events through chapter
63's tag readers in [nip34.coax](../../coax/lib/nostr/nip34.coax).

### What is not built yet

- **The fetch.** `nip05_check` takes the HTTP status and body that the host
  got. DNS, TLS (chapter 28) and the GET request itself are the host's. The
  host must not follow redirects. If it reports a 3xx status, the check
  refuses.
- **The `relays` object** of a NIP-05 response is skipped, not read.
- **NIP-24's extra kind 0 fields** (`display_name`, `website`, `banner`,
  `bot`, `birthday`) stay unread in the signed content. `metadata.coax` skips
  them.
- **Fetching a NIP-39 proof.** `n39_proof_url` gives the URL and
  `n39_proves` checks a body the host fetched. The GET request is the
  host's, and so is checking that the page was published by the claimed
  account and not by someone who copied the text. Discord proofs can be
  read only by members of the server.
- **Petname paths** such as `~/erin/charlie` are not resolved.
  `nostr_follow` reports only whether one petname has the characters that
  path resolution allows.

## Kind 0 profiles

A kind 0 event's `content` is a JSON object written as a string. NIP-05's
example event carries this content:

```text
{"name": "bob", "nip05": "bob@example.com"}
```

Chapter 22's `nostr_metadata_parse` reads `name`, `about` and `picture`. The
new `nip05_from_metadata` walks the same object for `nip05`. By hand:

1. The first token is `{`, so the content is an object. Anything else
   returns −1.
2. The key `name` is not `nip05`, so its value `"bob"` is skipped.
3. The key `nip05` matches. Its value is a string, so it is unescaped into
   the caller's buffer: `bob@example.com`, 15 bytes.

Keys are compared after unescaping, so `"nip05"` also matches. A
`nip05` value that is not a string, or that does not fit the buffer, returns
−1. Nested objects are skipped whole, so a `nip05` key inside one is never
read.

## NIP-05 names

### The identifier

`nip05_parse` splits the identifier at its `@`:

- The local part must be 1 to 64 bytes of `a-z`, `0-9`, `-`, `_` and `.`.
  NIP-05 says the local part MUST use only these characters.
- The domain must be 1 to 253 bytes of dot-separated labels, each at least
  one byte of `a-z`, `0-9` and `-`. A port, a path, an empty label or an
  uppercase letter refuses. DNS ignores case, but this reader refuses
  uppercase instead of folding it, so lowercase the domain first.
- `_@domain` is the root identifier. `Nip05Id.root` is true, and a client
  may show only the domain.

### Worked example: the URL by hand

For `bob@example.com`:

1. The local part is `bob`: three allowed characters, then `@` at byte 3.
2. The domain is `example.com`: the labels `example` and `com`, no empty
   label.
3. The URL is `https://` (8 bytes) + `example.com` (11) +
   `/.well-known/nostr.json?name=` (29) + `bob` (3), 51 bytes:

```text
https://example.com/.well-known/nostr.json?name=bob
```

The local part goes in the query, not the path, so one static file can
serve many names. Every allowed character is safe in a query, so nothing
needs percent-escaping.

### Worked example: the response by hand

NIP-05's example response is:

```text
{"names": {"bob": "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9"}}
```

`nip05_check(id, status, body, pubkey)` decides it in this order:

1. The status is 200. A status from 300 to 399 is `redirect`; any other
   status is `status`.
2. The body is at most 65,536 bytes of valid JSON, and its first token is
   `{`.
3. The top-level key `names` holds an object. A second `names` is
   `duplicate`.
4. Inside it, `bob` appears once. Its value is a string of exactly 64
   characters, each with ASCII bit 5 set (uppercase `A`–`F` lack it), and
   it decodes as hex to 32 bytes.
5. The 32 bytes equal the event's pubkey `b0635d6a...650b07b9`, so the
   result is `ok`.

An absent name is `unknown name`. An `npub` value is `hex`, even for the
right key, because NIP-05 requires hex. Other keys in `names` and other
top-level keys, such as `relays`, are skipped.

NIP-05 says clients must follow the pubkey, never the name. If the domain
later names a different key, the client stops showing the name; the follow
stays on the old key.

## Kind 3 follow lists

A kind 3 event lists followed keys in `p` tags. NIP-02 gives each tag up to
three values after the name:

```text
["p", "b0635d6a...650b07b9", "wss://relay.example.com", "bob"]
```

`nostr_follow(e, index)` reads tag `index` of a parsed kind 3 event:

1. The event must be parsed and its tag span must lie inside its text
   (`event`). Its kind must be 3 (`kind`).
2. Chapter 22's `nostr_tag` reads the tag. A past-the-end index is `index`.
   Any other refusal from `nostr_tag` is `tag`, with its status in
   `NostrFollow.tag`. Another tag name is `not p`.
3. The pubkey and the relay come from `nostr_tag`. An empty relay is
   allowed and reads as none.
4. The fourth value, if present, is the petname, unescaped. `resolvable` is
   true when the petname is not empty and uses only ASCII letters, digits
   and `_`, the characters NIP-02 allows in a petname path.

The content of a kind 3 event is not used.

### Replacement

Kind 3 is replaceable (chapter 22): one pubkey keeps one kind 3 event, the
one with the later `created_at`, and on a tie the lower ID. NIP-02 says each
new list overwrites the old one, so it must hold every follow.

The example signs three lists with secret key 3 (BIP-340 vector 0). Their
IDs are SHA-256 over `[0, pubkey, created_at, 3, tags, ""]`:

| List | `created_at` | Follows | ID begins |
| --- | ---: | ---: | --- |
| older | 1761686400 | 1 | `af17d275` |
| newer | 1761686460 | 2 | `2a0b4f55` |
| a minute before | 1761686340 | 1 | `42748780` |

Admitted in this order, by hand:

1. The older list is the first for this key: `accept`.
2. The newer list has the same address (kind 3, same pubkey) and a later
   `created_at`: `accept`. The older record is retired.
3. The older list again: its ID is still known, so `duplicate`.
4. The list from a minute before is new, but older than the live list:
   `superseded`.
5. The newer list again: `duplicate`.

The store ends with one live record, the newer list.

## External identities (NIP-39)

A NIP-05 name ties a key to a domain. NIP-39 ties a key to accounts on
other platforms. The key publishes a kind 10011 event with one `i` tag for
each account:

```text
["i", "github:semisol", "9721ce4ee4fceb91c9711ca2a6c9a5ab"]
```

The first value is the platform name and the identity on that platform,
joined by a colon. The second is the proof: where on that platform the
account says the same thing about the key. NIP-39: "An `i` tag MUST have
two parameters".

The tag alone proves nothing. Anyone can sign a kind 10011 event that
claims any account. The claim counts when both sides agree: the key names
the account in the event, and the account names the key in the proof. So a
client does three things:

1. Read the claims from the event (`n39_claims`).
2. Work out each proof's URL (`n39_proof_url`) and fetch it. The fetch is
   the host's.
3. Check that the fetched text holds the key's `npub` (`n39_proves`).

### Reading the claims

<!-- source: coax/lib/nostr/nip39.coax claims -->
```jai
// Whether raw is a platform name: at least one byte, each a-z, 0-9 or one
// of . _ - /.
n39_name :: (raw:string)->bool #effects(all) {
 for c:raw {if !((c>='a' && c<='z') || (c>='0' && c<='9') || c=='.' || c=='_' || c=='-' || c=='/') {return false;}}
 return raw.count>0;
}
// The claims of a kind 10011 event, in order, into out.
n39_claims :: (e:NtEvent,out:[]N39Claim)->N39Claims #effects(all) {
 r:N39Claims;r.status=n34_start(e,N39_KIND_IDENTITIES);if r.status!=N34_OK {return r;}
 n:=nt_tags(e,"i");
 if n>out.count {r.status=N34_OUTPUT;return r;}
 for k:0..<n {
  t:=nt_tag(e,"i",k);raw:=nt_raw(e,t.v[0]);colon:=0;r.at=k;
  while colon<raw.count && raw[colon]!=':' {colon+=1;}
  // No colon, or nothing after it, is no identity.
  if t.values<2 || colon+1>=raw.count {r.status=N34_MALFORMED;return r;}
  if !n39_name(raw[0..<colon]) {r.status=N39_PLATFORM;return r;}
  out[k]=N39Claim.{platform=NtSpan.{from=t.v[0].from,to=t.v[0].from+colon},identity=NtSpan.{from=t.v[0].from+colon+1,to=t.v[0].to},proof=t.v[1]};
 }
 r.at=-1;r.n=n;return r;
}
```

`n39_claims` splits the first value at its first colon. NIP-39 says a
platform name "MUST NOT include `:`", so everything before the first colon
is the platform and everything after it is the identity, further colons
included. Both sides must be non-empty.

NIP-39 also says a platform name "SHOULD only include `a-z`, `0-9` and the
characters `._-/`". This reader refuses any other byte with
`N39_PLATFORM`. If it accepted `GitHub`, a client would see `GitHub` and
`github` as two platforms.

A tag with one value refuses (`N34_MALFORMED`), and the record's `at` says
which `i` tag refused. Values after the second are kept and not read:
"Clients SHOULD process any `i` tags with more than 2 values for future
extensibility."

Kind 10011 is replaceable, like kind 3. The newest event is the whole
list, an event with no `i` tag withdraws every claim, and the store picks
the newest by the [replacement](#replacement) rule above.

### The proof's URL

NIP-39 defines six claim types. Each says where the proof is:

| Platform | Identity | Proof | URL |
| --- | --- | --- | --- |
| `github` | A username | A Gist id | `https://gist.github.com/<identity>/<proof>` |
| `twitter` | A username | A tweet id | `https://twitter.com/<identity>/status/<proof>` |
| `mastodon` | `<instance>/@<username>` | A post id | `https://<identity>/<proof>` |
| `telegram` | A user id | `<channel>/<message id>` | `https://t.me/<proof>` |
| `bluesky` | A handle | A post's record key | `https://bsky.app/profile/<identity>/post/<proof>` |
| `discord` | A username | `<server>/<channel>/<message>` | `https://discord.com/channels/<proof>` |

<!-- source: coax/lib/nostr/nip39.coax platforms -->
```jai
// The platforms NIP-39 defines, and each proof's URL: before, then the
// identity and between when the URL names the identity, then the proof.
// slashes is how many '/' the proof has: telegram's is <channel>/<message>,
// discord's <server>/<channel>/<message>.
N39_PLATFORMS :: [6] string.["github","twitter","mastodon","telegram","bluesky","discord"];
N39_BEFORE :: [6] string.["https://gist.github.com/","https://twitter.com/","https://","https://t.me/","https://bsky.app/profile/","https://discord.com/channels/"];
N39_NAMED :: [6] bool.[true,true,true,false,true,false];
N39_BETWEEN :: [6] string.["/","/status/","/","","/post/",""];
N39_SLASHES :: [6] int.[0,0,0,1,0,2];
N39_MASTODON :: 2;
```

The identity and the proof come from a signed event that anyone could
have written, and they go into a URL that the client then fetches. So
`n39_proof_url` puts a part into the URL only when `n39_part` accepts it:
letters, digits, `.`, `_` and `-`, with exactly the slashes that the
platform's layout has, and no segment made of dots only. A GitHub identity
of `alice/../bob`, or a tweet id of `1?ref=x`, gives no URL.

<!-- source: coax/lib/nostr/nip39.coax url -->
```jai
// Which of NIP-39's platforms raw names, or -1.
n39_platform :: (raw:string)->int #effects(all) {
 for i:0..<N39_PLATFORMS.count {if raw==N39_PLATFORMS[i] {return i;}}
 return -1;
}
// Whether raw can go into a URL's path as it is: letters, digits, '.', '_'
// and '-', with exactly this many slashes, nothing empty between them and
// no segment of dots only (".." would climb the path). With at, the part
// after the first slash must start with one '@': a Mastodon identity,
// <instance>/@<username>.
n39_part :: (raw:string,slashes:int,at:bool)->bool #effects(all) {
 seen:=0;length:=0;dots:=0;marked:=false;
 for c:raw {
  if c=='/' {
   if dots==length {return false;}
   seen+=1;length=0;dots=0;continue;
  }
  if c=='@' && at && seen==1 && length==0 && !marked {marked=true;continue;}
  if !((c>='a' && c<='z') || (c>='A' && c<='Z') || (c>='0' && c<='9') || c=='.' || c=='_' || c=='-') {return false;}
  length+=1;if c=='.' {dots+=1;}
 }
 return dots<length && seen==slashes && marked==at;
}
// The URL where the claim's proof is published. The new length; 0 for a
// platform NIP-39 doesn't define, or an identity or proof that doesn't fit
// its layout; -1 when out is short.
n39_proof_url :: (e:NtEvent,c:N39Claim,out:[]u8)->int #effects(all) {
 p:=n39_platform(nt_raw(e,c.platform));identity:=nt_raw(e,c.identity);proof:=nt_raw(e,c.proof);
 if p<0 || !n39_part(proof,N39_SLASHES[p],false) {return 0;}
 n:=json_raw(out,0,N39_BEFORE[p]);
 if N39_NAMED[p] {
  mastodon:=p==N39_MASTODON;
  if !n39_part(identity,ifx mastodon then 1 else 0,mastodon) {return 0;}
  n=json_raw(out,n,identity);n=json_raw(out,n,N39_BETWEEN[p]);
 }
 return json_raw(out,n,proof);
}
```

A platform outside the table, such as `matrix`, also gives no URL. The
claim still reads; a client shows it as unverified.

### The proof

NIP-39's verification text is
`Verifying that I control the following Nostr public key: <npub>`, where
`<npub>` is chapter 16's Bech32 form of the key. Older versions of the NIP
used other words, so "Clients verifying a claim SHOULD accept any proof
published by `<identity>` that contains the user's npub".

<!-- source: coax/lib/nostr/nip39.coax proof -->
```jai
// Whether npub has the shape of one: "npub1" and 58 more characters. The
// Bech32 checksum is chapter 16's check, not made here.
n39_npub :: (npub:string)->bool #effects(all) {return npub.count==63 && npub[0..<5]=="npub1";}
// The text a proof should hold. The new length, or -1 when npub isn't one
// or out is short.
n39_text :: (npub:string,out:[]u8)->int #effects(all) {
 if !n39_npub(npub) {return -1;}
 n:=json_raw(out,0,N39_TEXT);return json_raw(out,n,npub);
}
// Whether a fetched proof names the key: it contains the npub, in any
// wording.
n39_proves :: (body:string,npub:string)->bool #effects(all) {
 if !n39_npub(npub) {return false;}
 for i:0..<body.count-62 {if body[i..<i+63]==npub {return true;}}
 return false;
}
```

`n39_proves` first asks that its `npub` argument has the shape of one. An
empty string is found in every text, so without that check a caller that
passed an empty key would see every proof pass.

### Worked example: Alice's three claims

Alice signs with BIP-340's test key 3. Her public key is `f9308a01...` and
her `npub` is
`npub1lycg5qvjtrp3qjf5f7zl382j9x6nrjz9sdhenvyxq8c3808qxmus6gq266`. Her
kind 10011 event has three `i` tags. Split each first value at its first
colon and fill in the table's URL by hand:

| First value | Platform | Identity | Proof | URL |
| --- | --- | --- | --- | --- |
| `github:alice-tea` | `github` | `alice-tea` | `9721ce4ee4fceb91c9711ca2a6c9a5ab` | `https://gist.github.com/alice-tea/9721ce4ee4fceb91c9711ca2a6c9a5ab` |
| `mastodon:tea.example/@alice` | `mastodon` | `tea.example/@alice` | `109775066355589974` | `https://tea.example/@alice/109775066355589974` |
| `matrix:@alice:tea.example` | `matrix` | `@alice:tea.example` | `$proof-event` | None: NIP-39 defines no `matrix` claim |

The third identity keeps its own colon, because the split is at the first
one.

Then four fetched texts, checked for Alice's `npub`:

1. The Gist with NIP-39's text and Alice's `npub`: it proves the claim.
2. A post in an older wording that still holds Alice's `npub`: it proves
   the claim.
3. A Gist with NIP-39's text and Bob's `npub`: it proves nothing about
   Alice.
4. A page that names no key: nothing.

## The example

The example is
[nostr/examples/profiles-follows.coax](../examples/profiles-follows.coax).
It reads NIP-05's example profile, splits its identifier and writes the
URL, checks the two example responses, signs and reads a follow list, admits
the three lists to a store, and shows fifteen refusals.

<!-- source: nostr/examples/profiles-follows.coax example -->
```jai
#load "../../coax/lib/nostr/metadata.coax";
#load "../../coax/lib/nostr/follows.coax";
#load "../../coax/lib/nostr/nip05.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#load "../../nostr/relay/store_core.coax";
#import "Basic";

META :: [8] string.["ok", "event", "kind", "json", "shape", "duplicate key", "field", "output"];
N5 :: [12] string.["ok", "identifier", "output", "redirect", "status", "json", "shape",
                   "duplicate", "unknown name", "hex", "mismatch", "absent"];
FOLLOW :: [6] string.["ok", "event", "kind", "index", "not p", "tag"];
STORE :: [12] string.["accept", "duplicate", "superseded", "bad", "", "unverified", "", "", "", "", "", "full"];

// Print a label and bytes as lowercase hex.
print_hex :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex that the program itself supplies; it must be valid.
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}

// NIP-05's example user and its example responses.
BOB_HEX :: "b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9";
BOB_PROFILE :: "{\"name\": \"bob\", \"nip05\": \"bob@example.com\", \"display_name\": \"Bob B.\", \"bot\": false}";
NAMES :: "{\"names\":{\"bob\":\"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\"}}";
NAMES_RELAYS :: "{\"names\":{\"bob\":\"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\"},\"relays\":{\"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\":[\"wss://relay.example.com\",\"wss://relay2.example.com\"]}}";
NAMES_NPUB :: "{\"names\":{\"bob\":\"npub1kp34665c28f6a5xdd3y4k2ppv7k0wctjjpudja0uxsdjyegtq7us853d4g\"}}";
// The BIP-340 vector 0 secret key, 3, signs the follow lists; vector 1's
// public key is one of the followed keys.
SECRET_HEX :: "0000000000000000000000000000000000000000000000000000000000000003";
VECTOR1_PUB :: "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";
OLD_TAGS :: "[[\"p\",\"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\",\"wss://relay.example.com\",\"bob\"]]";
NEW_TAGS :: "[[\"p\",\"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\",\"wss://relay.example.com\",\"bob\"],[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\",\"\",\"vector_1\"],[\"t\",\"x\"]]";
T0 :: 1761686400;

// Check one response to bob@example.com's URL.
check :: (label: string, identifier: string, status: int, body: string, pubkey: string) {
  key: [32] u8; decode_hex(pubkey, key[..]);
  print("%: %\n", label, N5[nip05_check(nip05_parse(identifier), status, body, key[..])]);
}
// Sign a kind 3 event with the fixed key and zero aux, so the output repeats.
sign_follows :: (created: int, tags: string, out: [] u8) -> string {
  key: [32] u8; decode_hex(SECRET_HEX, key[..]); aux: [32] u8;
  sw: [1] NostrSignWork; scratch: [4096] u8;
  k := EVENT_MORE;
  while k == EVENT_MORE { k = nostr_event_sign_step(key[..], aux[..], created, 3, tags, "", sw[..], scratch[..], out, false); }
  #assert k > 0;
  return cast(string) out[0..<k];
}
// Print every tag of a follow list as a follow.
show_follows :: (e: NostrEvent) {
  for i: 0..<e.tags {
    f := nostr_follow(e, i);
    print("tag %: %", i, FOLLOW[f.error]);
    if f.error != FL_OK { print("\n"); continue; }
    print(", petname \"%\" (%), relay \"%\"\n", cast(string) f.petname[0..<f.petname_n],
          ifx f.resolvable then "resolvable" else "not resolvable", cast(string) f.relay[0..<f.relay_n]);
    print_hex("  pubkey", f.pubkey[..]);
  }
}
// The store record for a verified event; tags relative to its text.
record :: (e: NostrEvent) -> RsdRec {
  c: RsdRec;
  c.id = e.id; c.author = e.public_key; c.kind = e.kind; c.created = e.created_at; c.expiry = -1;
  c.tags_from = e.tags_from; c.tags_to = e.tags_to;
  return c;
}
admit :: (s: [] RsdStore, label: string, text: string) {
  scratch: [4096] u8;
  e := nostr_event_verify_vartime(text, scratch[..], false);
  print("admit %: %\n", label, STORE[ns_store_admit(s, record(e), text, e.verified)]);
}
show_store :: (s: [] RsdStore) {
  for i: 0..<s[0].n {
    r := s[0].recs[i];
    print("record %: created_at %, %, ", i, r.created, ifx ns_store_visible(s, i, T0) then "live" else "replaced");
    print_hex("id begins", r.id[0..<4]);
  }
}

main :: () {
  print("a profile\n");
  m := nostr_metadata_parse(BOB_PROFILE);
  print("kind 0 content: %, name \"%\", about %\n", META[m.error], nmd_text(m, NMD_NAME),
        ifx m.has[NMD_ABOUT] then "present" else "absent");
  ident: [64] u8;
  n := nip05_from_metadata(BOB_PROFILE, ident[..]);
  identifier := cast(string) ident[0..<n];
  print("nip05: %\n", identifier);

  print("the identifier\n");
  id := nip05_parse(identifier);
  print("parse: %, local \"%\", domain \"%\", root %\n", N5[id.error], id.local, id.domain, id.root);
  url: [128] u8;
  u := nip05_url(id, url[..]);
  print("GET %\n", cast(string) url[0..<u]);
  root := nip05_parse("_@bob.com");
  u = nip05_url(root, url[..]);
  print("_@bob.com: root %, GET %\n", root.root, cast(string) url[0..<u]);

  print("responses for bob@example.com\n");
  check("200, names", "bob@example.com", 200, NAMES, BOB_HEX);
  check("200, names and relays", "bob@example.com", 200, NAMES_RELAYS, BOB_HEX);

  print("a follow list\n");
  old_buf: [1024] u8; new_buf: [1024] u8; scratch: [4096] u8;
  old := sign_follows(T0, OLD_TAGS, old_buf[..]);
  early_buf: [1024] u8;
  early := sign_follows(T0 - 60, OLD_TAGS, early_buf[..]);
  new := sign_follows(T0 + 60, NEW_TAGS, new_buf[..]);
  e := nostr_event_verify_vartime(new, scratch[..], false);
  print("kind %, created_at %, tags %, verified %\n", e.kind, e.created_at, e.tags, e.verified);
  print_hex("signer", e.public_key[..]);
  show_follows(e);

  print("a newer follow list replaces the older one\n");
  arena: [4096] u8; recs: [8] RsdRec; s: [1] RsdStore; s[0].arena = arena[..]; s[0].recs = recs[..];
  admit(s[..], "older list", old);
  admit(s[..], "newer list", new);
  admit(s[..], "older list again", old);
  admit(s[..], "a list from a minute before", early);
  admit(s[..], "newer list again", new);
  show_store(s[..]);

  print("refusals\n");
  check("301 redirect", "bob@example.com", 301, NAMES, BOB_HEX);
  check("404", "bob@example.com", 404, NAMES, BOB_HEX);
  check("another key", "bob@example.com", 200, NAMES, VECTOR1_PUB);
  check("Bob@example.com", "Bob@example.com", 200, NAMES, BOB_HEX);
  check("bob+x@example.com", "bob+x@example.com", 200, NAMES, BOB_HEX);
  check("bob@example.com:443", "bob@example.com:443", 200, NAMES, BOB_HEX);
  check("alice@example.com", "alice@example.com", 200, NAMES, BOB_HEX);
  check("npub value", "bob@example.com", 200, NAMES_NPUB, BOB_HEX);
  check("names an array", "bob@example.com", 200, "{\"names\":[]}", BOB_HEX);
  print("kind 0, name twice: %\n", META[nostr_metadata_parse("{\"name\":\"a\",\"name\":\"b\"}").error]);
  print("kind 0, not JSON: %\n", META[nostr_metadata_parse("{\"name\":").error]);
  print("kind 0, nip05 1: %\n", nip05_from_metadata("{\"nip05\":1}", ident[..]));
  sb: [1024] u8;
  wrong := nostr_event_verify_vartime(sign_follows(T0, OLD_TAGS, sb[..]), scratch[..], false);
  wrong.kind = 1;
  print("follow of a kind 1 event: %\n", FOLLOW[nostr_follow(wrong, 0).error]);
  bad := nostr_event_verify_vartime(sign_follows(T0, "[[\"p\",\"B0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\"]]", sb[..]), scratch[..], false);
  f := nostr_follow(bad, 0);
  print("p tag with uppercase hex: %, tag status %\n", FOLLOW[f.error], f.tag);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/profiles-follows.coax --run-steps 250000000
```

Signing needs more compile-time steps than the default, as in chapter 25.
The output matches the values worked out by hand: the 51-byte URL, `ok` for
both responses, the three IDs and one live record:

```text
a profile
kind 0 content: ok, name "bob", about absent
nip05: bob@example.com
the identifier
parse: ok, local "bob", domain "example.com", root false
GET https://example.com/.well-known/nostr.json?name=bob
_@bob.com: root true, GET https://bob.com/.well-known/nostr.json?name=_
responses for bob@example.com
200, names: ok
200, names and relays: ok
a follow list
kind 3, created_at 1761686460, tags 3, verified true
signer: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
tag 0: ok, petname "bob" (resolvable), relay "wss://relay.example.com"
  pubkey: b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9
tag 1: ok, petname "vector_1" (resolvable), relay ""
  pubkey: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
tag 2: not p
a newer follow list replaces the older one
admit older list: accept
admit newer list: accept
admit older list again: duplicate
admit a list from a minute before: superseded
admit newer list again: duplicate
record 0: created_at 1761686400, replaced, id begins: af17d275
record 1: created_at 1761686460, live, id begins: 2a0b4f55
refusals
301 redirect: redirect
404: status
another key: mismatch
Bob@example.com: identifier
bob+x@example.com: identifier
bob@example.com:443: identifier
alice@example.com: unknown name
npub value: hex
names an array: shape
kind 0, name twice: duplicate key
kind 0, not JSON: json
kind 0, nip05 1: -1
follow of a kind 1 event: kind
p tag with uppercase hex: tag, tag status 4
```

The refusals:

- **NIP-05 responses.** A 301 is `redirect` and a 404 is `status`. The
  BIP-340 vector 1 key in place of bob's is `mismatch`. `Bob@example.com`,
  `bob+x@example.com` and `bob@example.com:443` are `identifier`.
  `alice@example.com` is `unknown name`. An `npub` value is `hex`, and an
  array for `names` is `shape`.
- **Kind 0 content.** A repeated `name` is `duplicate key` and truncated
  JSON is `json` (chapter 22's reader). A `nip05` that is a number gives −1.
- **Follows.** A follow list with its kind changed to 1 is `kind`. A `p` tag
  with an uppercase hex digit is `tag`, with `nostr_tag`'s status 4
  (`NT_HEX`).

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/profiles-follows.out](../fixtures/profiles-follows.out).

### The external identities example

A second example,
[nostr/examples/external-identities.coax](../examples/external-identities.coax),
signs Alice's kind 10011 event, reads its claims and their proof URLs,
writes the verification text for her `npub`, checks the four fetched texts
(fixed strings: the example fetches nothing), signs a newer list with one
claim, and then shows the refusals.

<!-- source: nostr/examples/external-identities.coax example -->
```jai
#load "../../coax/lib/nostr/nip34-event.coax";
#load "../../coax/lib/nostr/nip39.coax";
#load "../../coax/lib/nostr/nip19-core.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// The statuses of nip34.coax, then nip39.coax's 6.
STATUS :: [7] string.["ok", "event", "kind", "missing", "malformed", "output", "bad platform name"];
// BIP-340 test vectors 0 and 1's published secret keys. Never sign with a
// real key in an example.
KEYS :: [2] string.[
  "0000000000000000000000000000000000000000000000000000000000000003",
  "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef"];
PEOPLE :: [2] string.["Alice", "Bob"];
ALICE :: 0; BOB :: 1;
T0 :: 1761686400;
// Alice's claims: two platforms NIP-39 defines and one it doesn't. A
// Matrix id has a colon of its own; the tag splits at the first one.
CLAIMS :: "[[\"i\",\"github:alice-tea\",\"9721ce4ee4fceb91c9711ca2a6c9a5ab\"],[\"i\",\"mastodon:tea.example/@alice\",\"109775066355589974\"],[\"i\",\"matrix:@alice:tea.example\",\"$proof-event\"]]";

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

// A key's npub (chapter 16).
npub_of :: (key: [32] u8, out: [] u8) -> string {
  k := key;
  r := nip19_encode_checked(N19_NPUB, cast(string) k[..], out, false);
  #assert r.error == NC_OK;
  return cast(string) out[0..<r.written];
}

show :: (label: string, event: NostrEvent) {
  e := nt_event_of(event); out: [4] N39Claim; r := n39_claims(e, out[..]);
  print("%: %", label, STATUS[r.status]);
  if r.status != N34_OK {
    if r.at >= 0 { print(", at i tag %", r.at); }
    print("\n"); return;
  }
  print(", % %\n", r.n, ifx r.n == 1 then "claim" else "claims");
  for k: 0..<r.n {
    url: [160] u8; n := n39_proof_url(e, out[k], url[..]);
    print("  platform %, identity %, proof %\n", nt_raw(e, out[k].platform), nt_raw(e, out[k].identity), nt_raw(e, out[k].proof));
    if n > 0 { print("    proof at %\n", cast(string) url[0..<n]); }
    else { print("    no proof URL: %\n", ifx n39_platform(nt_raw(e, out[k].platform)) < 0 then "NIP-39 doesn't define this platform" else "the identity or proof doesn't fit the platform's layout"); }
  }
}

main :: () {
  secret: [2][32] u8; keys: [2][32] u8;
  for k: 0..<2 { decode_hex(KEYS[k], secret[k][..]); }
  buf: [2][8192] u8;
  // Each public key comes from signing: sign an empty note first.
  for k: 0..<2 { keys[k] = sign(secret[k][..], T0, 1, "[]", "", buf[0][..]).public_key; }
  np: [2][64] u8; npub: [2] string;
  for k: 0..<2 { npub[k] = npub_of(keys[k], np[k][..]); print("%: %\n", PEOPLE[k], npub[k]); }

  print("Alice's identities (kind 10011)\n");
  i1 := sign(secret[ALICE][..], T0, 10011, CLAIMS, "", buf[0][..]);
  show("I1", i1);

  print("the proof Alice publishes on each platform\n");
  text: [160] u8; n := n39_text(npub[ALICE], text[..]);
  #assert n > 0;
  print("%\n", cast(string) text[0..<n]);

  print("fetched proofs, checked for Alice's npub\n");
  // What a client might download from the three URLs. Fetching is the
  // client's; here the bodies are fixed text.
  gist: [200] u8; g := json_raw(gist[..], 0, cast(string) text[0..<n]); g = json_raw(gist[..], g, "\n");
  print("the gist, with NIP-39's text: %\n", n39_proves(cast(string) gist[0..<g], npub[ALICE]));
  old: [200] u8; o := json_raw(old[..], 0, "Verifying my account on nostr My Public Key: \""); o = json_raw(old[..], o, npub[ALICE]); o = json_raw(old[..], o, "\"");
  print("a post in an older wording: %\n", n39_proves(cast(string) old[0..<o], npub[ALICE]));
  other: [200] u8; b := n39_text(npub[BOB], other[..]);
  print("a gist that names Bob's key: %\n", n39_proves(cast(string) other[0..<b], npub[ALICE]));
  print("a page that names no key: %\n", n39_proves("alice-tea has no public gists.", npub[ALICE]));

  print("a newer list: it is the whole list, so two claims are withdrawn\n");
  i2 := sign(secret[ALICE][..], T0 + 3600, 10011, "[[\"i\",\"github:alice-tea\",\"9721ce4ee4fceb91c9711ca2a6c9a5ab\"]]", "", buf[1][..]);
  show("I2, one hour later", i2);

  print("each event's id\n");
  h := n34_hex32(i1.id); print("I1: %, verified %\n", cast(string) h[..], i1.verified);
  h = n34_hex32(i2.id); print("I2: %, verified %\n", cast(string) h[..], i2.verified);

  print("refusals\n");
  bad: [8192] u8;
  show("a claim without a proof", unsigned(10011, "[[\"i\",\"github:alice-tea\",\"9721ce4e\"],[\"i\",\"twitter:alice_tea\"]]", bad[..]));
  show("a claim with no colon", unsigned(10011, "[[\"i\",\"github\",\"9721ce4e\"]]", bad[..]));
  show("a claim with no identity", unsigned(10011, "[[\"i\",\"github:\",\"9721ce4e\"]]", bad[..]));
  show("a platform in capitals", unsigned(10011, "[[\"i\",\"GitHub:alice-tea\",\"9721ce4e\"]]", bad[..]));
  show("five claims into room for four", unsigned(10011, "[[\"i\",\"a:1\",\"p\"],[\"i\",\"b:2\",\"p\"],[\"i\",\"c:3\",\"p\"],[\"i\",\"d:4\",\"p\"],[\"i\",\"e:5\",\"p\"]]", bad[..]));
  show("a kind 0 profile read as identities", unsigned(0, "[]", bad[..]));
  show("claims whose parts would change the URL", unsigned(10011, "[[\"i\",\"github:alice-tea/../bob\",\"9721ce4e\"],[\"i\",\"twitter:alice_tea\",\"1?ref=x\"],[\"i\",\"mastodon:tea.example\",\"1\"]]", bad[..]));
  print("verification text for an nsec: %\n", n39_text("nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqsl0kzwd", text[..]));
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/external-identities.coax --run-steps 250000000
```

The output matches the hand values:

```text
Alice: npub1lycg5qvjtrp3qjf5f7zl382j9x6nrjz9sdhenvyxq8c3808qxmus6gq266
Bob: npub1mlcawle2vuw97dscxundkg6phev0atsa5t0vakzrys8hk5pt5evssm7a0a
Alice's identities (kind 10011)
I1: ok, 3 claims
  platform github, identity alice-tea, proof 9721ce4ee4fceb91c9711ca2a6c9a5ab
    proof at https://gist.github.com/alice-tea/9721ce4ee4fceb91c9711ca2a6c9a5ab
  platform mastodon, identity tea.example/@alice, proof 109775066355589974
    proof at https://tea.example/@alice/109775066355589974
  platform matrix, identity @alice:tea.example, proof $proof-event
    no proof URL: NIP-39 doesn't define this platform
the proof Alice publishes on each platform
Verifying that I control the following Nostr public key: npub1lycg5qvjtrp3qjf5f7zl382j9x6nrjz9sdhenvyxq8c3808qxmus6gq266
fetched proofs, checked for Alice's npub
the gist, with NIP-39's text: true
a post in an older wording: true
a gist that names Bob's key: false
a page that names no key: false
a newer list: it is the whole list, so two claims are withdrawn
I2, one hour later: ok, 1 claim
  platform github, identity alice-tea, proof 9721ce4ee4fceb91c9711ca2a6c9a5ab
    proof at https://gist.github.com/alice-tea/9721ce4ee4fceb91c9711ca2a6c9a5ab
each event's id
I1: d4c4f78642b8c4c1405fda3c47e2d8ba36491e997164ed17cf009e18a7a81828, verified true
I2: e98d27f8f33f354af13f5dd27601478879510d388025fadc4689c122ddf38144, verified true
refusals
a claim without a proof: malformed, at i tag 1
a claim with no colon: malformed, at i tag 0
a claim with no identity: malformed, at i tag 0
a platform in capitals: bad platform name, at i tag 0
five claims into room for four: output
a kind 0 profile read as identities: kind
claims whose parts would change the URL: ok, 3 claims
  platform github, identity alice-tea/../bob, proof 9721ce4e
    no proof URL: the identity or proof doesn't fit the platform's layout
  platform twitter, identity alice_tea, proof 1?ref=x
    no proof URL: the identity or proof doesn't fit the platform's layout
  platform mastodon, identity tea.example, proof 1
    no proof URL: the identity or proof doesn't fit the platform's layout
verification text for an nsec: -1
```

The refusals:

- A claim without a proof is `malformed`, and `at` names the second `i`
  tag. A claim with no colon, or with nothing after it, is `malformed`.
- `GitHub` is a `bad platform name`.
- Five claims do not fit the example's array of four: `output`.
- A kind 0 profile is not a list of identities: `kind`.
- Three claims read, but give no URL: an identity that would climb the
  path, a proof with a query string, and a Mastodon identity with no
  `/@username`.
- `n39_text` writes no verification text for an `nsec`: it returns -1.

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/external-identities.out](../fixtures/external-identities.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/profiles-follows.coax](../examples/checks/profiles-follows.coax),
which loads the example. They compare the code with values from outside it:

- NIP-05's example as the NIP prints it: the content, the URL and both
  responses with their whitespace, and every status from 300 to 308 as a
  redirect;
- bob's key as an `npub`, from a Python BIP-173 encoder that reproduces
  NIP-19's own `npub` example, decoded by chapter 19's
  `nip19_decode_checked` and refused by `nip05_check` as `hex`;
- NIP-02's petname example tag, verbatim;
- the three follow-list IDs from Python's `hashlib` over
  `json.dumps([0, pubkey, created_at, 3, tags, ""])`, and the signer's key
  against BIP-340 vector 0;
- the hand-worked replacement rule in all six arrival orders of the three
  lists: the newer list is always the one live record, a list older than it
  is `superseded`, and an unverified list is never stored.

<!-- source: nostr/examples/checks/profiles-follows.coax check -->
```jai
// NIP-05's example, as the NIP prints it: the kind 0 content, the URL the
// client must GET, and both example responses, whitespace and all.
SPEC_CONTENT :: "{\"name\": \"bob\", \"nip05\": \"bob@example.com\"}";
SPEC_URL :: "https://example.com/.well-known/nostr.json?name=bob";
SPEC_BODY :: "{\n  \"names\": {\n    \"bob\": \"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\"\n  }\n}";
SPEC_BODY_RELAYS :: "{\n  \"names\": {\n    \"bob\": \"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\"\n  },\n  \"relays\": {\n    \"b0635d6a9851d3aed0cd6c495b282167acf761729078d975fc341b22650b07b9\": [ \"wss://relay.example.com\", \"wss://relay2.example.com\" ]\n  }\n}";

#test "NIP-05's example: content, URL and both responses" {
  ident: [64] u8; n := nip05_from_metadata(SPEC_CONTENT, ident[..]);
  id := nip05_parse(cast(string) ident[0..<n]);
  url: [128] u8; u := nip05_url(id, url[..]);
  #assert cast(string) url[0..<u] == SPEC_URL;
  key: [32] u8; decode_hex(BOB_HEX, key[..]);
  #assert nip05_check(id, 200, SPEC_BODY, key[..]) == N5_OK;
  #assert nip05_check(id, 200, SPEC_BODY_RELAYS, key[..]) == N5_OK;
  // NIP-05: "Fetchers MUST ignore any HTTP redirects".
  for status: 300..<309 { #assert nip05_check(id, status, SPEC_BODY, key[..]) == N5_REDIRECT; }
}
// The npub in NAMES_NPUB is bob's key: NIP-19's decoder (chapter 19), and a
// Python BIP-173 encoder that gives NIP-19's own npub example, agree. NIP-05
// requires hex, so the response refuses.
#test "an npub of the right key is still refused" {
  out: [64] u8;
  r := nip19_decode_checked("npub1kp34665c28f6a5xdd3y4k2ppv7k0wctjjpudja0uxsdjyegtq7us853d4g", out[..], false);
  key: [32] u8; decode_hex(BOB_HEX, key[..]);
  #assert r.error == NC_OK && r.type == N19_NPUB && r.written == 32;
  for i: 0..<32 { #assert out[i] == key[i]; }
  #assert nip05_check(nip05_parse("bob@example.com"), 200, NAMES_NPUB, key[..]) == N5_HEX;
}
// NIP-02's petname example tag, verbatim.
#test "NIP-02's petname example reads as erin" {
  e := fl_event(3, "[[\"p\", \"21df6d143fb96c2ec9d63726bf9edc7121df6d143fb96c2ec9d63726bf9edc71\", \"\", \"erin\"]]");
  f := nostr_follow(e, 0);
  #assert f.error == FL_OK && !f.has_relay && cast(string) f.petname[0..<f.petname_n] == "erin" && f.resolvable;
  #assert f.pubkey[0] == 0x21 && f.pubkey[1] == 0xdf && f.pubkey[31] == 0x71;
}
// The follow lists' ids from Python's hashlib over json.dumps([0, pubkey,
// created_at, 3, tags, ""], separators=(",", ":")), and the signer's key
// from BIP-340 vector 0 (secret key 3).
OLD_ID :: "af17d275dbf2dab414fefb86ada21f479067e19f9e1f3cc20fc34583d91ace68";
NEW_ID :: "2a0b4f5525259c0747d8e5834fcf7c0c983047166ace36dd3c32c9f47d630d21";
EARLY_ID :: "427487804bb316d3e5a937c06ea69ebd1eda5ca819134fb5808f6cb6fbb82776";
VECTOR0_PUB :: "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
same_hex :: (bytes: [32] u8, text: string) -> bool {
  want: [32] u8; decode_hex(text, want[..]);
  for i: 0..<32 { if bytes[i] != want[i] { return false; } }
  return true;
}
#test "same_hex sees a difference in the first byte" {
  a: [32] u8; decode_hex(NEW_ID, a[..]);
  #assert same_hex(a, NEW_ID);
  a[0] ^= 1; #assert !same_hex(a, NEW_ID);
}
#test "the signed follow lists have the independent ids and signer" #steps 1000000000 {
  b: [1024] u8; scratch: [4096] u8;
  times := [3] int.[T0, T0 + 60, T0 - 60];
  tags := [3] string.[OLD_TAGS, NEW_TAGS, OLD_TAGS];
  ids := [3] string.[OLD_ID, NEW_ID, EARLY_ID];
  for i: 0..<3 {
    e := nostr_event_verify_vartime(sign_follows(times[i], tags[i], b[..]), scratch[..], false);
    #assert e.verified && e.kind == 3 && e.created_at == times[i];
    #assert same_hex(e.id, ids[i]) && same_hex(e.public_key, VECTOR0_PUB);
  }
}
// By NIP-01 and NIP-02, worked out by hand: the newer list (T0 + 60) is the
// one live record whichever order the three lists arrive in; a list older
// than the live one is superseded; one already seen is a duplicate.
#test "the newest follow list is the only live one in every arrival order" #steps 1000000000 {
  bo: [1024] u8; bn: [1024] u8; be: [1024] u8; scratch: [4096] u8;
  texts := [3] string.[sign_follows(T0, OLD_TAGS, bo[..]), sign_follows(T0 + 60, NEW_TAGS, bn[..]), sign_follows(T0 - 60, OLD_TAGS, be[..])];
  orders := [18] int.[0, 1, 2, 0, 2, 1, 1, 0, 2, 1, 2, 0, 2, 0, 1, 2, 1, 0];
  recs3: [3] RsdRec; ok: [3] bool;
  for k: 0..<3 { e := nostr_event_verify_vartime(texts[k], scratch[..], false); recs3[k] = record(e); ok[k] = e.verified; }
  for o: 0..<6 {
    arena: [4096] u8; recs: [8] RsdRec; s: [1] RsdStore; s[0].arena = arena[..]; s[0].recs = recs[..];
    seen_new := false;
    for step: 0..<3 {
      k := orders[3 * o + step];
      got := ns_store_admit(s[..], recs3[k], texts[k], ok[k]);
      if k == 1 { #assert got == NSS_ACCEPT; seen_new = true; }
      else if seen_new { #assert got == NSS_SUPERSEDED; }
    }
    live := 0;
    for i: 0..<s[0].n { if ns_store_visible(s[..], i, T0) { live += 1; #assert same_hex(s[0].recs[i].id, NEW_ID); } }
    #assert live == 1 && s[0].live == 1;
    #assert ns_store_admit(s[..], recs3[0], texts[0], ok[0]) != NSS_ACCEPT;
  }
}
// A list that was not verified is never stored.
#test "an unverified follow list is refused" #steps 1000000000 {
  b: [1024] u8; scratch: [4096] u8;
  text := sign_follows(T0, OLD_TAGS, b[..]);
  e := nostr_event_verify_vartime(text, scratch[..], false);
  arena: [4096] u8; recs: [8] RsdRec; s: [1] RsdStore; s[0].arena = arena[..]; s[0].recs = recs[..];
  #assert ns_store_admit(s[..], record(e), text, false) == NSS_UNVERIFIED && s[0].n == 0;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/profiles-follows.coax --run-steps 250000000
```

`coax test` runs 419 checks, including the loaded files' own checks, in
about 55 seconds.

### The libraries' own checks

[nip05.coax](../../coax/lib/nostr/nip05.coax) checks each refusal of the
identifier (excluded characters next to each allowed range, the 64-byte and
253-byte bounds), the URL's exact length, the status boundaries (200, 201,
204, 299, 300, 399, 400), duplicates written with escapes, uppercase, short
and `npub` keys, a one-bit difference in the first and last key byte, and
the 65,536-byte body bound.
[follows.coax](../../coax/lib/nostr/follows.coax) checks NIP-02's example
list (its abbreviated keys padded to 64 digits), wrong kinds, forged spans,
each `nostr_tag` refusal and the petname characters.

On 2026-11-01, `coax mutate --root-only` killed 192 of 192 mutants in
`nip05.coax` and 67 of 67 in `follows.coax`, none marked equivalent.
In the checks file it killed 7 of 7.
The code this chapter reuses is covered by earlier records, such as the
[2026-10-30 record](../../docs/verification/2026-10-30-nostr-tags-metadata.md)
for `tags.coax` and `metadata.coax`; those runs were not repeated. No full
(not root-only) mutation run and no Wasm or iOS run of the two new files was
made.

### Checks of external identities

The checks are in
[nostr/examples/checks/external-identities.coax](../examples/checks/external-identities.coax),
which loads the example. They compare the code with values from outside
it:

- the two event ids from Python's `hashlib`, and Alice's and Bob's `npub`
  from a Bech32 encoder written from BIP-173 in the same script;
- the verification text, as the script writes it;
- the three claims and their URLs against the hand values above;
- NIP-39's own example event, verbatim, and the four URLs its claim types
  give for it.

<!-- source: nostr/examples/checks/external-identities.coax check -->
```jai
// The ids and tags from a Python script written from NIP-01, NIP-19 and NIP-39
// (it shares no code with Coax): the ids are hashlib.sha256 of
// json.dumps([0, pubkey, created_at, kind, tags, content],
// separators=(",", ":"), ensure_ascii=False).
PUB :: [2] string.[
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659"];
PY_ID :: [2] string.[
  "d4c4f78642b8c4c1405fda3c47e2d8ba36491e997164ed17cf009e18a7a81828",
  "e98d27f8f33f354af13f5dd27601478879510d388025fadc4689c122ddf38144"];
PY_I1 :: "[[\"i\",\"github:alice-tea\",\"9721ce4ee4fceb91c9711ca2a6c9a5ab\"],[\"i\",\"mastodon:tea.example/@alice\",\"109775066355589974\"],[\"i\",\"matrix:@alice:tea.example\",\"$proof-event\"]]";
PY_I2 :: "[[\"i\",\"github:alice-tea\",\"9721ce4ee4fceb91c9711ca2a6c9a5ab\"]]";
PY_AUTHOR :: [2] int.[0, 0];
PY_AT :: [2] int.[0, 3600];
PY_KIND :: [2] int.[10011, 10011];
PY_TAGS :: [2] string.[PY_I1, PY_I2];
PY_CONTENT :: [2] string.["", ""];

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
// The npubs are the script's own Bech32 (BIP-173) of the two keys.
PY_NPUB :: [2] string.[
  "npub1lycg5qvjtrp3qjf5f7zl382j9x6nrjz9sdhenvyxq8c3808qxmus6gq266",
  "npub1mlcawle2vuw97dscxundkg6phev0atsa5t0vakzrys8hk5pt5evssm7a0a"];
PY_TEXT :: "Verifying that I control the following Nostr public key: npub1lycg5qvjtrp3qjf5f7zl382j9x6nrjz9sdhenvyxq8c3808qxmus6gq266";

#test "the ids, the npubs and the verification text are Python's" {
  buf: [2][4096] u8; scratch: [4096] u8;
  for i: 0..<2 { e := py(i, buf[i][..], scratch[..]); #assert e.error == NE_OK && n34_same(e.computed_id, e.id); }
  #assert CLAIMS == PY_I1;
  np: [64] u8; key: [32] u8;
  for k: 0..<2 { decode_hex(PUB[k], key[..]); #assert npub_of(key, np[..]) == PY_NPUB[k]; }
  text: [160] u8; n := n39_text(PY_NPUB[ALICE], text[..]); #assert cast(string) text[0..<n] == PY_TEXT;
  #assert n39_proves(PY_TEXT, PY_NPUB[ALICE]) && !n39_proves(PY_TEXT, PY_NPUB[BOB]);
}

// By hand: I1 has three i tags. Each first value splits at its first colon:
// "github" and "alice-tea"; "mastodon" and "tea.example/@alice"; "matrix"
// and "@alice:tea.example" (the second colon stays in the identity). The
// proof URLs follow NIP-39's layouts: gist.github.com/<identity>/<proof>
// and https://<identity>/<proof>. NIP-39 defines no "matrix" claim type.
#test "Python's events read as worked by hand" {
  buf: [2][4096] u8; scratch: [4096] u8; out: [4] N39Claim; url: [160] u8;
  e := nt_event_of(py(0, buf[0][..], scratch[..])); r := n39_claims(e, out[..]);
  #assert r.status == N34_OK && r.n == 3;
  #assert nt_raw(e, out[0].platform) == "github" && nt_raw(e, out[0].identity) == "alice-tea" && nt_raw(e, out[0].proof) == "9721ce4ee4fceb91c9711ca2a6c9a5ab";
  #assert nt_raw(e, out[1].platform) == "mastodon" && nt_raw(e, out[1].identity) == "tea.example/@alice" && nt_raw(e, out[1].proof) == "109775066355589974";
  #assert nt_raw(e, out[2].platform) == "matrix" && nt_raw(e, out[2].identity) == "@alice:tea.example" && nt_raw(e, out[2].proof) == "$proof-event";
  n := n39_proof_url(e, out[0], url[..]); #assert cast(string) url[0..<n] == "https://gist.github.com/alice-tea/9721ce4ee4fceb91c9711ca2a6c9a5ab";
  n = n39_proof_url(e, out[1], url[..]); #assert cast(string) url[0..<n] == "https://tea.example/@alice/109775066355589974";
  #assert n39_proof_url(e, out[2], url[..]) == 0;
  e = nt_event_of(py(1, buf[1][..], scratch[..])); r = n39_claims(e, out[..]);
  #assert r.status == N34_OK && r.n == 1 && nt_raw(e, out[0].identity) == "alice-tea";
}

// NIP-39's own example event and the four URLs its claim types give.
#test "NIP-39's example and its proof URLs" {
  e := n34_event("[[\"i\",\"github:semisol\",\"9721ce4ee4fceb91c9711ca2a6c9a5ab\"],[\"i\",\"twitter:semisol_public\",\"1619358434134196225\"],[\"i\",\"mastodon:bitcoinhackers.org/@semisol\",\"109775066355589974\"],[\"i\",\"telegram:1087295469\",\"nostrdirectory/770\"]]", 10011);
  out: [4] N39Claim; r := n39_claims(e, out[..]); #assert r.status == N34_OK && r.n == 4;
  want := [4] string.["https://gist.github.com/semisol/9721ce4ee4fceb91c9711ca2a6c9a5ab", "https://twitter.com/semisol_public/status/1619358434134196225",
    "https://bitcoinhackers.org/@semisol/109775066355589974", "https://t.me/nostrdirectory/770"];
  for k: 0..<4 { url: [160] u8; n := n39_proof_url(e, out[k], url[..]); #assert cast(string) url[0..<n] == want[k]; }
}
```

[nip39.coax](../../coax/lib/nostr/nip39.coax) has `#test` checks written
with its code: NIP-39's example, the first colon, extra values, an empty
list, every refusal of a claim, the bytes just outside each allowed range
of a platform name, the six URLs, claims that give no URL, the
slashes and dots of a URL part, a Mastodon identity's `@`, and proofs with
the `npub` first, last, cut short by one character and absent.

```shell
coax/bin/coax test coax/lib/nostr/nip39.coax
coax/bin/coax test nostr/examples/checks/external-identities.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/nip39.coax --root-only
```

On 2026-11-12, `coax test` passed all 51 checks of `nip39.coax` and all
401 of the example's checks (each count includes the loaded files'
checks). `coax mutate --root-only` killed 183 of 183 mutants in
`nip39.coax`, none marked equivalent. The first run left seven survivors,
most of them in a separate scan for a Mastodon identity's `/@`. That scan
was folded into `n39_part`; new checks and one removed condition that
could not change a result dealt with the rest. No full (not
root-only) mutation run and no Wasm or iOS run of the file was made.

## Summary

- Kind 0 content is a JSON object in a string. `nip05_from_metadata` reads
  its `nip05` value.
- `nip05_parse` accepts `a-z0-9-_.` before the `@` and lowercase DNS labels
  after it. `nip05_url` puts the local part in the query of
  `/.well-known/nostr.json`.
- `nip05_check` accepts only status 200 and a `names` entry equal to the
  pubkey in lowercase hex. Redirects, other statuses, unknown names, `npub`
  values and other keys refuse.
- `nostr_follow` reads a kind 3 `p` tag as a pubkey, an optional relay and
  an optional petname.
- Kind 3 is replaceable: the store keeps only the newest list.
- A NIP-39 kind 10011 event claims accounts on other platforms.
  `n39_claims` reads each `i` tag, `n39_proof_url` gives the proof's URL
  for six platforms and refuses parts that would change the URL, and
  `n39_proves` accepts a fetched proof only when it holds the key's
  `npub`.

## Dependencies

- [22. Kinds and tags](ch022-kinds-and-tags.md)
- [28. Sockets, TLS and relay URLs](ch028-sockets-tls-and-relay-urls.md)

## Sources

- [NIP-01](../nips/official/01.md), kind 0 and replaceable events.
- [NIP-02](../nips/official/02.md), follow lists and petnames.
- [NIP-05](../nips/official/05.md), DNS-based identifiers.
- [NIP-24](../nips/official/24.md), the extra kind 0 fields (not read yet).
- [NIP-39](../nips/official/39.md), external identities.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
