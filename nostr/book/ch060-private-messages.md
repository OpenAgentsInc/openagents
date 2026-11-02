# 60. Private messages

NIP-17 is Nostr's private messaging. A message is an unsigned kind 14
rumor, sealed and gift wrapped with chapter 25's NIP-59 once for each
recipient and once for the sender, and sent only to the relays each
addressee lists in a kind 10050 event. This chapter builds the relay
plan, the send, the open and the room on top of chapter 25, works out
Alice's message to Bob and Carol by hand, and checks it against an
independent implementation and NIP-17's own example.

Part VI, Every NIP. Result: NIP-17 kind 14 chat messages sent as gift wraps
to every participant, kind 10050 DM relay lists that decide where each wrap
goes (and that nothing goes to a user without one), rooms named by their
member set and ordered by time, and kind 15 file messages checked for their
file tags. NIP-A4 public messages are described and not built; NIP-EE is
`unrecommended` and not implemented.

## What this chapter teaches

- Kind 14 rumor with one `p` tag per recipient; the chat room is the set of
  the rumor's `pubkey` and its `p` tags.
- One gift wrap per recipient and one for the sender: N recipients, N+1
  wraps.
- Kind 10050 lists the relays to deliver to; no list means do not send.
- The seal's key must be the rumor's `pubkey`, or anyone could write as
  anyone.
- Kind 15 file messages with `file-type`, `encryption-algorithm`,
  `decryption-key`, `decryption-nonce` and `x`.
- NIP-A4: kind 24 public messages, not encrypted. Not built here.
- NIP-EE is `unrecommended`, superseded by the Marmot protocol; not
  implemented.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip17.coax](../../coax/lib/nostr/nip17.coax) | `n17_relays(e, owner)` | Reads one participant's kind 10050 relays, or refuses (`#authority`) |
| | `nip17_plan(sender_x, recipients, lists)` | Checks the member set and reads every participant's list before any wrap exists |
| | `nip17_send(sender, plan, now, kind, tags, content, fresh, work, out, overlap)` | Writes the rumor's `p` tags and makes N+1 wraps with `nip59_wrap_checked` |
| | `nip17_open(recipient, recipient_x, wrap, work, out)` | Opens a wrap with `nip59_unwrap_checked`, then reads it as a NIP-17 message with `n17_read` |
| | `n17_room(keys, count)`, `nip17_order(m, room, order)`, `nip17_subject(m, order)` | The room id, a room's messages in time order, and its current subject |
| | `n17_extra`, `n17_file`, `n17_value`, `n17_rumor_tags` | The caller's tags, kind 15's file tags, one tag's first value, the rumor's tags |
| [nip59.coax](../../coax/lib/nostr/nip59.coax) | `nip59_wrap_checked`, `nip59_unwrap_checked` | Chapter 25's rumor, seal and wrap |
| [tags.coax](../../coax/lib/nostr/tags.coax), [relay_uri.coax](../../coax/lib/net/relay_uri.coax) | `nostr_tag`, `relay_uri` | Chapter 22's tag reader and chapter 28's relay URL check |

Nothing in `nip17.coax` draws randomness. The caller passes one chapter 25
`Nip59Fresh` record per wrap (N+1 of them), and every admitted return zeroes
all of them. Nothing allocates: the send needs chapter 25's `NIP59_WORK`
workspace and an output buffer that holds every wrap.

## Messages, rooms and copies

NIP-17 puts three ideas on top of NIP-59:

| Idea | NIP-17's rule | Here |
| --- | --- | --- |
| Message | An unsigned kind 14 (chat) or 15 (file) rumor; content is plain text | `nip17_send` refuses every other kind, kind 4 included |
| Room | The set of the rumor's `pubkey` and its `p` tags; a new member or a dropped one is a new room | `n17_room`: SHA-256 of the member keys sorted ascending |
| Copies | Sealed and wrapped "to each receiver and the sender individually" | Wrap i is for participant i; the last is the sender's own copy |

The sender's copy is how a user's other devices, or a client restored from
the key alone, see what the user sent: NIP-17 calls this "fully recoverable".

The room id is this chapter's own name for a room. It is computed locally
and never published. NIP-17 has no public group identifier on purpose: a
relay sees only wraps addressed to one key each, from keys used once.

An optional `subject` tag names the room; the newest message with one sets
it. An `e` tag names the message a reply answers. Neither has to repeat.

## Where each wrap goes

Kind 10050 is a replaceable list of `relay` tags. NIP-17 says clients MUST
publish only to the relays in the recipient's list, and that a missing
list means the user "is not ready to receive messages and clients
shouldn't try".

`nip17_plan` applies that before anything is encrypted:

1. Every recipient is a 32-byte key, there is at least one and at most ten
   (NIP-17: past ten participants, use another scheme), none repeats and
   none is the sender. Otherwise `N17_MEMBERS`, naming the participant.
2. For each participant, recipients first and then the sender, the list
   must be found (`N17_NO_LIST` when it isn't), be kind 10050 and read
   (`N17_LIST`), be signed by that participant (`N17_LIST_AUTHOR`), and
   hold at least one relay (`N17_NO_LIST` again: an empty list is the same
   as none).
3. The first participant that fails refuses the whole plan.

A group message with one unready member is refused whole, not sent to the
others: the other members would see a room that includes someone who can't
read it. A list signed by someone else is refused because whoever controls
a user's list controls where that user's mail goes.

<!-- source: coax/lib/nostr/nip17.coax plan -->
```jai
// Who gets a wrap and which relays it goes to. recipients is N 32-byte keys;
// lists holds N+1 kind 10050 events: the recipients' in order, then the
// sender's. Any participant without a usable list refuses the whole plan.
nip17_plan :: (sender_x:[]u8,recipients:[]u8,lists:[]NostrEvent)->N17Plan {
  p:N17Plan;n:=recipients.count/32;
  if sender_x.count!=32 || recipients.count%32!=0 || lists.count!=n+1 {return p;}
  p.error=N17_MEMBERS;
  if n==0 || n>N17_MOST {return p;}
  for i:0..<n {
    for c:0..<32 {p.pub[i*32+c]=recipients[i*32+c];}
    if n17_same(recipients[i*32..<i*32+32],sender_x) {p.who=i;return p;}
    for k:0..<i {if n17_same(recipients[i*32..<i*32+32],recipients[k*32..<k*32+32]) {p.who=i;return p;}}
  }
  for c:0..<32 {p.pub[n*32+c]=sender_x[c];}
  p.recipients=n;
  for i:0..<n+1 {
    p.relays[i]=n17_relays(lists[i],p.pub[i*32..<i*32+32]);
    if p.relays[i].error!=N17_OK {p.error=p.relays[i].error;p.who=i;return p;}
  }
  p.error=N17_OK;return p;
}
```

`n17_relays` reads the tags. A `relay` tag's first value is the URL; later
values are ignored. Each URL must pass chapter 28's `relay_uri`, is kept
once, and at most eight are kept (NIP-17 asks for one to three).

<!-- source: coax/lib/nostr/nip17.coax relays -->
```jai
// A participant's kind 10050 relays. e is the newest list found for owner,
// or an unparsed NostrEvent when none was found.
n17_relays :: (e:NostrEvent,owner:[]u8)->N17Relays {
  r:N17Relays;j:[1]Json;
  if !nt_start(j[..],e) {return r;}
  if e.kind!=N17_RELAY_LIST {return n17_list_fail(r,N17_LIST,0);}
  signer:=e.public_key;
  if !n17_same(signer[..],owner) {return n17_list_fail(r,N17_LIST_AUTHOR,0);}
  k:=0;t:=json_next(j[..]);
  while t!=JSON_ARRAY_END {
    if t!=JSON_ARRAY {return n17_list_fail(r,N17_LIST,k);}
    depth:=j[0].depth;t=json_next(j[..]);
    named:=t==JSON_STRING && json_is(json_text(j[..]),"relay");
    if named {
      t=json_next(j[..]);
      if t!=JSON_STRING {return n17_list_fail(r,N17_LIST,k);}
      url:[N17_URL_MOST]u8;n:=json_unescape(json_text(j[..]),url[..]);
      if n== -1 || !relay_uri(cast(string)url[0..<n]).ok {return n17_list_fail(r,N17_LIST,k);}
      at:=r.count;
      for i:0..<r.count {if n17_url(r,i,cast(string)url[0..<n]) {at=i;}}
      if at==N17_RELAYS_MOST {return n17_list_fail(r,N17_LIST,k);}
      if at==r.count {for i:0..<n {r.url[at*N17_URL_MOST+i]=url[i];}r.url_n[at]=n;r.count+=1;}
    }
    // The rest of the tag, whatever it holds.
    while j[0].depth>=depth {if json_next(j[..])==JSON_ERROR {return n17_list_fail(r,N17_LIST,k);}}
    k+=1;t=json_next(j[..]);
  }
  if r.count==0 {return n17_list_fail(r,N17_NO_LIST,0);}
  return n17_list_fail(r,N17_OK,0);
}
```

## Sending

`nip17_send` refuses, before it does any key work:

- a plan that was refused (with the plan's status and participant);
- a kind other than 14 or 15 (`N17_KIND`);
- caller tags that are not one JSON array that reads (an empty one written
  `[]`), or that hold a `p` tag (`N17_TAGS`): the plan, not the caller,
  names the room;
- a kind 15 without its file tags (`N17_FILE`).

It then checks that the plan's last participant is the signing key, so the
sender's copy can't be addressed to someone else, writes the rumor's tags
(`["p", recipient]` for each recipient in plan order, then the caller's),
and calls `nip59_wrap_checked` once per participant. Every call makes the
same rumor, so every wrap carries the same rumor id.

<!-- source: coax/lib/nostr/nip17.coax send -->
```jai
// Seals and wraps one kind 14 or 15 message from sender to every participant
// of plan: wrap i is for participant i, the last for the sender. fresh holds
// one record of host entropy per wrap; every admitted return zeroes all of
// it, and a refusal after the first wrap also zeroes out. tags are the
// caller's own (subject, e for a reply, kind 15's file tags); the p tags
// come from the plan.
nip17_send :: (sender:[]u8,given:N17Plan,now:int,kind:int,tags:string,content:string,fresh:[]Nip59Fresh,work:[]u8,out:[]u8,overlap:bool)->N17Sent {
  s:N17Sent;plan:=given;
  if sender.count!=32 || fresh.count!=plan.recipients+1 {return s;}
  if plan.error!=N17_OK {return n17_sent(s,plan.error,plan.who,fresh);}
  if kind!=N17_CHAT && kind!=N17_FILE_KIND {return n17_sent(s,N17_KIND,-1,fresh);}
  e:=n17_extra(tags,kind);
  if e!=N17_OK {return n17_sent(s,e,-1,fresh);}
  rtags:[N17_TAGS_MOST]u8;t:=n17_rumor_tags(plan,tags,rtags[..]);
  if t== -1 {return n17_sent(s,N17_TAGS,-1,fresh);}
  // The sender's copy must be addressed to the key that signs the seals.
  sender_index:=plan.recipients;
  me:[33]u8;
  if !bip32_public(sender,me[..]) {s.wrap_error=NIP59_KEY;return n17_sent(s,N17_WRAP,sender_index,fresh);}
  if !n17_same(me[1..<33],plan.pub[sender_index*32..<sender_index*32+32]) {return n17_sent(s,N17_MEMBERS,sender_index,fresh);}
  at:=0;
  for i:0..<plan.recipients+1 {
    w:=nip59_wrap_checked(sender,plan.pub[i*32..<i*32+32],now,kind,cast(string)rtags[0..<t],content,NIP59_WRAP_KIND,
                          fresh[i..<i+1],work,out[at..<out.count],overlap);
    if w.error!=NIP59_OK {
      n59_zero(out);s.wrap_error=w.error;
      return n17_sent(s,ifx w.error==NIP59_OUTPUT then N17_OUTPUT else N17_WRAP,i,fresh);
    }
    s.at[i]=at;s.n[i]=w.written;s.rumor_id=w.rumor_id;at+=w.written;
  }
  s.wraps=plan.recipients+1;s.error=N17_OK;return s;
}
```

## Opening

`nip59_unwrap_checked` already performs the two checks NIP-17 depends on:
a signed rumor is refused (`NIP59_RUMOR`), and a seal whose key is not the
rumor's `pubkey` is refused (`NIP59_IDENTITY`). Without the second, any
sender could seal a rumor that names Alice as its author. `nip17_open`
reports either as `N17_WRAP` with the NIP-59 status in `wrap_error`, then
reads the rumor:

<!-- source: coax/lib/nostr/nip17.coax read -->
```jai
// An opened rumor read as a NIP-17 message for the reader me. rumor is the
// JSON that nip59_unwrap_checked wrote, and o its result.
n17_read :: (o:Nip59Opened,rumor:string,me:[]u8)->N17Message {
  m:N17Message;
  if me.count!=32 {return m;}
  if o.kind!=N17_CHAT && o.kind!=N17_FILE_KIND {m.error=N17_KIND;return m;}
  e:NostrEvent;e.error=NE_OK;e.original=rumor;e.tags_from=o.tags_from;e.tags_to=o.tags_to;
  m.members=1;for c:0..<32 {m.member[c]=o.author[c];}
  i:=0;t:=nostr_tag(e,0);
  while t.error!=NT_INDEX {
    if t.error!=NT_OK {m.error=N17_TAGS;return m;}
    if t.type==NT_P {
      known:=false;
      for k:0..<m.members {if n17_same(m.member[k*32..<k*32+32],t.pubkey[..]) {known=true;}}
      if !known {
        if m.members==N17_MOST+1 {m.error=N17_MEMBERS;return m;}
        for c:0..<32 {m.member[m.members*32+c]=t.pubkey[c];}
        m.members+=1;
      }
    }
    if t.type==NT_E && !m.has_reply {m.has_reply=true;m.reply=t.id;}
    i+=1;t=nostr_tag(e,i);
  }
  m.subject_n=n17_value(e,"subject",m.subject[..]);
  if m.subject_n==N17_BAD {m.error=N17_TAGS;return m;}
  if o.kind==N17_FILE_KIND && !n17_file(e) {m.error=N17_FILE;return m;}
  member:=false;
  for k:0..<m.members {if n17_same(m.member[k*32..<k*32+32],me) {member=true;}}
  if !member {m.error=N17_NOT_MEMBER;return m;}
  m.room=n17_room(m.member[..],m.members);
  m.written=o.written;m.author=o.author;m.rumor_id=o.rumor_id;m.kind=o.kind;m.created_at=o.created_at;
  m.content_from=o.content_from;m.content_to=o.content_to;m.error=N17_OK;
  return m;
}
```

A `p` tag that repeats, or that names the author, adds no member. A rumor
whose room doesn't include the reader is refused as `N17_NOT_MEMBER`: the
wrap was addressed to the reader, but the message says it is for someone
else.

`nip17_order` lists one room's opened messages oldest first. Two messages
with the same `created_at` are ordered by rumor id, so every member sees the
same order.

<!-- source: coax/lib/nostr/nip17.coax order -->
```jai
// The indexes of the opened messages in room, oldest first; equal times by
// rumor id. Returns their count, or -1 when order is too small.
nip17_order :: (m:[]N17Message,room:[]u8,order:[]int)->int {
  n:=0;
  for i:0..<m.count {
    if m[i].error!=N17_OK || !n17_same(m[i].room[..],room) {continue;}
    if n==order.count {return -1;}
    k:=n;order[n]=i;n+=1;
    while k>0 && n17_before(m[order[k]],m[order[k-1]]) {x:=order[k];order[k]=order[k-1];order[k-1]=x;k-=1;}
  }
  return n;
}
```

## Worked example: Alice writes to Bob and Carol

Alice (secret key 3) sends "Are you going to the party tonight?" with the
subject "Party" to Bob (BIP-340 vector 1's key) and Carol (secret key 7).
The keys are chapter 25's:

| Who | x-only public key |
| --- | --- |
| Alice | `f9308a01…bce036f9` |
| Bob | `dff1d77f…502ba659` |
| Carol | `5cbdf064…cac4f9bc` |

### Step 1: count and address the wraps

There are N = 2 recipients, so N+1 = 3 wraps. Each wrap's only tag is
`["p", addressee]`, and each goes only to its addressee's kind 10050 relays:

| Wrap | `p` tag | Relays | Publishes |
| --- | --- | --- | --- |
| 0 | Bob | `wss://inbox.nostr.wine`, `wss://myrelay.nostr1.com` | 2 |
| 1 | Carol | `wss://carol.example` | 1 |
| 2 | Alice (her own copy) | `wss://alice.example` | 1 |

That is four relay publishes for one message. Bob's list is NIP-17's own
example list.

### Step 2: the rumor

The rumor has two `p` tags, Bob's then Carol's, and the subject:

```jai
{"id":"eb4521dd…2faac811","pubkey":"f9308a01…bce036f9","created_at":1793300000,"kind":14,
 "tags":[["p","dff1d77f…502ba659"],["p","5cbdf064…cac4f9bc"],["subject","Party"]],
 "content":"Are you going to the party tonight?"}
```

Written in full it is 406 bytes, and its id `eb4521dd…2faac811` is the
SHA-256 of the NIP-01 array `[0, pubkey, created_at, 14, tags, content]`.
All three wraps carry this one rumor.

### Step 3: the sizes

Chapter 20's padding rounds 406 bytes up to 448. The seal's content is the
base64 of 1 + 32 + 2 + 448 + 32 = 515 bytes: 4 × ⌈515 / 3⌉ = 688
characters. The seal is 1,031 bytes. That pads to 1,280, so the wrap's
content is the base64 of 1 + 32 + 2 + 1,280 + 32 = 1,347 bytes: 1,796
characters. The wrap's other fields, with its one `p` tag, take 417 bytes:
every wrap is 1,796 + 417 = 2,213 bytes. The three wraps have the same size
because they carry the same rumor.

### Step 4: the times

The example's stand-in draws for wrap i are `seal_back` = 3,600 (i + 1) and
`wrap_back` = 90,000 + 7,000 i:

| Wrap | Seal `created_at` | Wrap `created_at` |
| --- | --- | --- |
| 0 | 1793300000 − 3600 = 1793296400 | 1793300000 − 90000 = 1793210000 |
| 1 | 1793300000 − 7200 = 1793292800 | 1793300000 − 97000 = 1793203000 |
| 2 | 1793300000 − 10800 = 1793289200 | 1793300000 − 104000 = 1793196000 |

Only the rumor keeps the real time.

### Step 5: the room

The room's members are Alice and the two `p` tags. Sorted by their bytes,
`5cbd…` (Carol) < `dff1…` (Bob) < `f930…` (Alice), so the room id is
SHA-256(Carol ‖ Bob ‖ Alice) = `845f2643…96d1862f`. Bob's reply tags Alice
and Carol: the same set, the same room. Alice's later message to Bob alone
has members {Alice, Bob}: SHA-256(Bob ‖ Alice) = `21ef0f45…d1b71395`, a
different room.

## The example

[nostr/examples/private-messages.coax](../examples/private-messages.coax)
signs three kind 10050 lists (Dave has none), sends Alice's message, Bob's
reply and Alice's message to Bob alone, opens copies as their addressees,
lists Bob's two rooms, then shows the refusals and opens NIP-17's own
example.

<!-- source: nostr/examples/private-messages.coax example -->
```jai
#load "../../coax/lib/nostr/nip17.coax";
#load "../../coax/lib/nostr/nip04.coax";
#import "Basic";

print_hex :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
decode_hex :: (text: string, out: [] u8) {
  r := hex_decode_checked(text, out, false);
  #assert r.error == NC_OK && r.written == out.count;
}
STATUS :: [12] string.["ok", "input", "kind", "members", "tags", "file", "no list",
  "list", "list author", "wrap", "output", "not member"];
WRAP_STATUS :: [17] string.["ok", "input", "alias", "output", "capacity", "key", "fresh",
  "wrap", "wrap signature", "recipient", "wrap payload", "seal", "seal signature",
  "seal payload", "rumor", "identity", "internal"];

// Test keys only: Alice k=3, Bob is BIP-340 vector 1's secret key, Carol
// k=7, Dave k=9. Never put a real key in an example.
ALICE_HEX :: "0000000000000000000000000000000000000000000000000000000000000003";
BOB_HEX :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
CAROL_HEX :: "0000000000000000000000000000000000000000000000000000000000000007";
DAVE_HEX :: "0000000000000000000000000000000000000000000000000000000000000009";
BOB_TAG :: "[[\"p\",\"dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659\"]]";
NOW :: 1793300000;

test_key :: (text: string) -> [32] u8 { k: [32] u8; decode_hex(text, k[..]); return k; }
small_key :: (n: int) -> [32] u8 { k: [32] u8; k[31] = cast(u8) n; return k; }
x_only_key :: (secret: [] u8) -> [32] u8 {
  aux: [32] u8; sw: [1] NostrSignWork; scratch: [256] u8; out: [256] u8;
  #assert nostr_event_sign_step(secret, aux[..], 0, 1, "[]", "", sw[..], scratch[..], out[..], false) == EVENT_MORE;
  return sw[0].pub;
}
// What a host's random source would supply for wrap i of message m: a
// one-time key (k = 11 + 4m + i), two NIP-44 nonces, two BIP-340 aux values,
// and the draws that date the seal and the wrap back.
fixed_entropy :: (m: int, wraps: int) -> [N17_MOST + 1] Nip59Fresh {
  f: [N17_MOST + 1] Nip59Fresh;
  for i: 0..<wraps {
    b := (16 * (m * 4 + i + 1)) % 256;
    f[i].wrap_key = small_key(11 + m * 4 + i);
    for c: 0..<32 {
      f[i].seal_nonce[c] = cast(u8) (b + 1); f[i].wrap_nonce[c] = cast(u8) (b + 2);
      f[i].seal_aux[c] = cast(u8) (b + 3); f[i].wrap_aux[c] = cast(u8) (b + 4);
    }
    f[i].seal_back = cast(u64) (3600 * (i + 1) + 600 * m);
    f[i].wrap_back = cast(u64) (90000 + 7000 * i + 1000 * m);
  }
  return f;
}

// A kind 10050 list signed by its owner, verified and parsed. The event
// keeps pointing into text, so text must outlive it.
relay_list :: (owner: [] u8, tags: string, text: [] u8) -> NostrEvent {
  aux: [32] u8; sw: [1] NostrSignWork; scratch: [4096] u8; pre: [4096] u8;
  n := n59_sign(owner, aux[..], NOW - 86400, N17_RELAY_LIST, tags, "", sw[..], scratch[..], text);
  e := nostr_event_verify_vartime(cast(string) text[0..<n], pre[..], false);
  #assert e.verified;
  return e;
}
print_plan :: (p: N17Plan, names: [] string) {
  print("plan: %", STATUS[p.error]);
  if p.who >= 0 { print(" (%)", names[p.who]); }
  print("\n");
  if p.error != N17_OK { return; }
  for i: 0..<p.recipients + 1 {
    print("  wrap % to %:", i, names[i]);
    r := p.relays[i]; url := r.url;
    for k: 0..<r.count { print(" %", cast(string) url[k * N17_URL_MOST..<k * N17_URL_MOST + r.url_n[k]]); }
    print("\n");
  }
}
// Sends one message and prints each wrap's size, time, id and signer.
send :: (sender: [] u8, p: N17Plan, m: int, at: int, tags: string, content: string, out: [] u8) -> N17Sent {
  fresh := fixed_entropy(m, p.recipients + 1); work: [NIP59_WORK] u8;
  s := nip17_send(sender, p, at, 14, tags, content, fresh[0..<p.recipients + 1], work[..], out, false);
  print("send: %", STATUS[s.error]);
  if s.error == N17_WRAP { print(" (%)", WRAP_STATUS[s.wrap_error]); }
  if s.who >= 0 { print(" at participant %", s.who); }
  print(", % wraps\n", s.wraps);
  if s.error != N17_OK { return s; }
  print_hex("  rumor id", s.rumor_id[..]);
  for i: 0..<s.wraps {
    scratch: [NIP59_REGION] u8;
    e := nostr_event_read(cast(string) out[s.at[i]..<s.at[i] + s.n[i]], scratch[..], false);
    print("  wrap %: % bytes, created_at %\n", i, s.n[i], e.created_at);
    print_hex("    id", e.id[..]); print_hex("    signed by", e.public_key[..]);
  }
  return s;
}
wrap_of :: (s: N17Sent, out: [] u8, i: int) -> string { return cast(string) out[s.at[i]..<s.at[i] + s.n[i]]; }

// Opens text as (secret, its public key) and prints the message.
open_as :: (label: string, secret: [] u8, me: [] u8, text: string) { kept := open_kept(label, secret, me, text); }
open_kept :: (label: string, secret: [] u8, me: [] u8, text: string) -> N17Message {
  work: [NIP59_WORK] u8; out: [NIP59_REGION] u8;
  m := nip17_open(secret, me, text, work[..], out[..]);
  print("%: %", label, STATUS[m.error]);
  if m.error == N17_WRAP { print(" (%)", WRAP_STATUS[m.wrap_error]); }
  print("\n");
  if m.error == N17_OK {
    print_hex("  author", m.author[..]);
    print("  kind %, created_at %, members %, content %\n", m.kind, m.created_at, m.members,
          cast(string) out[m.content_from..<m.content_to]);
    print_hex("  room", m.room[..]);
    if m.subject_n >= 0 { print("  subject %\n", cast(string) m.subject[0..<m.subject_n]); }
    if m.has_reply { print_hex("  reply to", m.reply[..]); }
  }
  return m;
}

// Seals any rumor bytes with seal_key and wraps the seal for `to` with a
// one-time key, skipping every rule nip59_wrap_checked enforces.
forge :: (seal_key: [] u8, to: [] u8, rumor: [] u8, out: [] u8) -> string {
  one := small_key(99); f := fixed_entropy(5, 1);
  ck: [32] u8; work: [2 * NIP59_REGION] u8; pay: [NIP59_REGION] u8;
  scratch: [NIP59_REGION] u8; seal: [NIP59_REGION] u8; tag: [80] u8;
  #assert n59_ck(seal_key, to, ck[..]) == NIP44_OK;
  p := nip44_encrypt_checked(NIP44_COMPAT, ck[..], f[0].seal_nonce[..], rumor, work[..], pay[..], false);
  sw: [1] NostrSignWork;
  s := n59_sign(seal_key, f[0].seal_aux[..], NOW - 3600, NIP59_SEAL_KIND, "[]",
                cast(string) pay[0..<p.written], sw[..], scratch[..], seal[..]);
  #assert s > 0 && n59_ck(one[..], to, ck[..]) == NIP44_OK;
  q := nip44_encrypt_checked(NIP44_COMPAT, ck[..], f[0].wrap_nonce[..], seal[0..<s], work[..], pay[..], false);
  hex: [64] u8; h := hex_encode_checked(cast(string) to, hex[..], false);
  t := json_raw(tag[..], 0, "[[\"p\",\""); t = json_raw(tag[..], t, cast(string) hex[0..<h.written]); t = json_raw(tag[..], t, "\"]]");
  ww: [1] NostrSignWork;
  n := n59_sign(one[..], f[0].wrap_aux[..], NOW - 27200, NIP59_WRAP_KIND, cast(string) tag[0..<t],
                cast(string) pay[0..<q.written], ww[..], scratch[..], out);
  return cast(string) out[0..<n];
}

main :: () {
  alice := test_key(ALICE_HEX); bob := test_key(BOB_HEX); carol := test_key(CAROL_HEX); dave := test_key(DAVE_HEX);
  alice_pub := x_only_key(alice[..]); bob_pub := x_only_key(bob[..]);
  carol_pub := x_only_key(carol[..]); dave_pub := x_only_key(dave[..]);
  print("keys\n");
  print_hex("alice", alice_pub[..]); print_hex("bob", bob_pub[..]);
  print_hex("carol", carol_pub[..]); print_hex("dave", dave_pub[..]);

  // Each list is signed by its owner. Dave has published none.
  bob_text: [1024] u8; carol_text: [1024] u8; alice_text: [1024] u8; forged_text: [1024] u8;
  bob_list := relay_list(bob[..], "[[\"relay\",\"wss://inbox.nostr.wine\"],[\"relay\",\"wss://myrelay.nostr1.com\"]]", bob_text[..]);
  carol_list := relay_list(carol[..], "[[\"relay\",\"wss://carol.example\"]]", carol_text[..]);
  alice_list := relay_list(alice[..], "[[\"relay\",\"wss://alice.example\"]]", alice_text[..]);

  print("alice to bob and carol\n");
  to: [64] u8; for i: 0..<32 { to[i] = bob_pub[i]; to[32 + i] = carol_pub[i]; }
  lists: [3] NostrEvent; lists[0] = bob_list; lists[1] = carol_list; lists[2] = alice_list;
  plan := nip17_plan(alice_pub[..], to[..], lists[..]);
  names1 := [3] string.["bob", "carol", "alice"]; print_plan(plan, names1[..]);
  first: [16384] u8;
  s0 := send(alice[..], plan, 0, NOW, "[[\"subject\",\"Party\"]]", "Are you going to the party tonight?", first[..]);
  print("wrap 0: %\n", wrap_of(s0, first[..], 0));

  print("bob replies to the room\n");
  rid := s0.rumor_id; hex: [64] u8; h := hex_encode_checked(cast(string) rid[..], hex[..], false);
  reply_tags: [128] u8; t := json_raw(reply_tags[..], 0, "[[\"e\",\"");
  t = json_raw(reply_tags[..], t, cast(string) hex[0..<h.written]); t = json_raw(reply_tags[..], t, "\"]]");
  for i: 0..<32 { to[i] = alice_pub[i]; to[32 + i] = carol_pub[i]; }
  lists[0] = alice_list; lists[1] = carol_list; lists[2] = bob_list;
  plan = nip17_plan(bob_pub[..], to[..], lists[..]);
  names2 := [3] string.["alice", "carol", "bob"]; print_plan(plan, names2[..]);
  second: [16384] u8;
  s1 := send(bob[..], plan, 1, NOW + 60, cast(string) reply_tags[0..<t], "Yes, at nine.", second[..]);

  print("alice to bob alone\n");
  lists[0] = bob_list; lists[1] = alice_list;
  plan = nip17_plan(alice_pub[..], bob_pub[..], lists[0..<2]);
  names3 := [2] string.["bob", "alice"]; print_plan(plan, names3[..]);
  third: [16384] u8;
  s2 := send(alice[..], plan, 2, NOW + 120, "[]", "Bring the speaker.", third[..]);

  print("each addressee opens their copy\n");
  open_as("carol opens wrap 1 of the first", carol[..], carol_pub[..], wrap_of(s0, first[..], 1));
  open_as("alice opens her own copy of the first", alice[..], alice_pub[..], wrap_of(s0, first[..], 2));
  open_as("alice opens wrap 0 of the reply", alice[..], alice_pub[..], wrap_of(s1, second[..], 0));
  // Bob receives his three out of order: the direct message, his own reply, the first.
  inbox: [3] N17Message;
  inbox[0] = open_kept("bob opens wrap 0 of the direct message", bob[..], bob_pub[..], wrap_of(s2, third[..], 0));
  inbox[1] = open_kept("bob opens his own copy of the reply", bob[..], bob_pub[..], wrap_of(s1, second[..], 2));
  inbox[2] = open_kept("bob opens wrap 0 of the first", bob[..], bob_pub[..], wrap_of(s0, first[..], 0));

  print("bob's rooms\n");
  rooms := [2] int.[2, 0];
  for r: 0..<2 {
    order: [3] int; room := inbox[rooms[r]].room;
    n := nip17_order(inbox[..], room[..], order[..]);
    print_hex("room", room[..]);
    for k: 0..<n { print("  inbox[%], created_at %\n", order[k], inbox[order[k]].created_at); }
    sub := nip17_subject(inbox[..], order[0..<n]);
    if sub >= 0 { subject := inbox[sub].subject; print("  subject: %\n", cast(string) subject[0..<inbox[sub].subject_n]); }
    else { print("  no subject\n"); }
  }

  print("refusals\n");
  for i: 0..<32 { to[i] = bob_pub[i]; to[32 + i] = dave_pub[i]; }
  none: NostrEvent; lists[0] = bob_list; lists[1] = none; lists[2] = alice_list;
  plan = nip17_plan(alice_pub[..], to[..], lists[..]);
  print("alice to bob and dave, ");
  names4 := [3] string.["bob", "dave", "alice"]; print_plan(plan, names4[..]);
  scrap: [16384] u8;
  refused := send(alice[..], plan, 3, NOW, "[]", "Party?", scrap[..]);
  lists[1] = relay_list(bob[..], "[[\"relay\",\"wss://bob.example\"]]", forged_text[..]);
  print("dave's list signed by bob, ");
  names5 := [3] string.["bob", "dave", "alice"]; print_plan(nip17_plan(alice_pub[..], to[..], lists[..]), names5[..]);
  lists[1] = carol_list;
  for i: 0..<32 { to[32 + i] = carol_pub[i]; }
  plan = nip17_plan(alice_pub[..], to[..], lists[..]);
  fresh := fixed_entropy(4, 3); work: [NIP59_WORK] u8;
  k4 := nip17_send(alice[..], plan, NOW, 4, "[]", "x", fresh[0..<3], work[..], scrap[..], false);
  print("a kind 4 message, send: %\n", STATUS[k4.error]);
  shared: [32] u8; legacy: [64] u8;
  print("nip04_encrypt: %\n", ifx nip04_encrypt(shared[..], "x", legacy[..]).error == NIP04_RETIRED then "retired" else "made");

  pre: [NIP59_REGION] u8; body: [NIP59_REGION] u8; id: [32] u8; forged: [NIP59_REGION] u8;
  n := n59_rumor(alice_pub[..], NOW, 14, BOB_TAG, "I am Alice.", pre[..], body[..], id[..]);
  open_as("alice's rumor sealed by carol", bob[..], bob_pub[..], forge(carol[..], bob_pub[..], body[0..<n], forged[..]));
  aux: [32] u8; sw: [1] NostrSignWork;
  n = n59_sign(alice[..], aux[..], NOW, 14, BOB_TAG, "signed", sw[..], pre[..], body[..]);
  open_as("a signed rumor", bob[..], bob_pub[..], forge(alice[..], bob_pub[..], body[0..<n], forged[..]));
  fresh = fixed_entropy(6, 1);
  w := nip59_wrap_checked(alice[..], dave_pub[..], NOW, 14, BOB_TAG, "for bob only", NIP59_WRAP_KIND,
                          fresh[0..<1], work[..], forged[..], false);
  open_as("a wrap to dave of a rumor naming only bob", dave[..], dave_pub[..], cast(string) forged[0..<w.written]);
  open_as("bob opens carol's wrap", bob[..], bob_pub[..], wrap_of(s0, first[..], 1));

  print("NIP-17's example\n");
  sender := test_key(NIP17_SENDER_HEX); receiver := test_key(NIP17_RECEIVER_HEX);
  sender_pub := x_only_key(sender[..]); receiver_pub := x_only_key(receiver[..]);
  open_as("the receiver opens the first wrap", receiver[..], receiver_pub[..], NIP17_EXAMPLE_1);
  open_as("the sender opens the second wrap", sender[..], sender_pub[..], NIP17_EXAMPLE_2);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/private-messages.coax --run-steps 250000000
```

Output (the wrap's middle is shortened here; the fixture has it in full):

```text
keys
alice: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
bob: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
carol: 5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc
dave: acd484e2f0c7f65309ad178a9f559abde09796974c57e714c35f110dfc27ccbe
alice to bob and carol
plan: ok
  wrap 0 to bob: wss://inbox.nostr.wine wss://myrelay.nostr1.com
  wrap 1 to carol: wss://carol.example
  wrap 2 to alice: wss://alice.example
send: ok, 3 wraps
  rumor id: eb4521dd9b5c8e84b55ec8aa59f910613772fb0c7d584ae07cacd9aa2faac811
  wrap 0: 2213 bytes, created_at 1793210000
    id: 96a2756d46a71cdeffe0ae578f22978543543dd97c83587e7e97c7775479edef
    signed by: 774ae7f858a9411e5ef4246b70c65aac5649980be5c17891bbec17895da008cb
  wrap 1: 2213 bytes, created_at 1793203000
    id: fac32add0ca415b9e02809f5211880ecd145d6b0b496af1c185a61098d964ce7
    signed by: d01115d548e7561b15c38f004d734633687cf4419620095bc5b0f47070afe85a
  wrap 2: 2213 bytes, created_at 1793196000
    id: 5f5b46637d1e022b24eee5d861c830e6e410ba77fa5beeb6a6b4b36217ddcb62
    signed by: f28773c2d975288bc7d1d205c3748651b075fbc6610e58cddeeddf8f19405aa8
wrap 0: {"id":"96a2756d46a71cdeffe0ae578f22978543543dd97c83587e7e97c7775479edef","pubkey":"774ae7f858a9411e5ef4246b70c65aac5649980be5c17891bbec17895da008cb","created_at":1793210000…6a1ca581da95ac078dbd47eb64dd79f645fb8e"}
bob replies to the room
plan: ok
  wrap 0 to alice: wss://alice.example
  wrap 1 to carol: wss://carol.example
  wrap 2 to bob: wss://inbox.nostr.wine wss://myrelay.nostr1.com
send: ok, 3 wraps
  rumor id: 385eb8c9c4a1d90c4cc63c8e7870467a1d8e440b2e848a9d6a2ba49b120965ea
  wrap 0: 2213 bytes, created_at 1793209060
    id: 00a06c6461d57c49014977230fb083054baef77240db3eaec0f6d97ae683a203
    signed by: d7924d4f7d43ea965a465ae3095ff41131e5946f3c85f79e44adbcf8e27e080e
  wrap 1: 2213 bytes, created_at 1793202060
    id: df2bbc88f975ee8c4369b17b8b2033976fcf75aa0bbda4c652ba5a2d71b3bf95
    signed by: e60fce93b59e9ec53011aabc21c23e97b2a31369b87a5ae9c44ee89e2a6dec0a
  wrap 2: 2213 bytes, created_at 1793195060
    id: 258c3ac91109800c4f1bb8892eba264c4addadadee071c92f02028d486ef281f
    signed by: defdea4cdb677750a420fee807eacf21eb9898ae79b9768766e4faa04a2d4a34
alice to bob alone
plan: ok
  wrap 0 to bob: wss://inbox.nostr.wine wss://myrelay.nostr1.com
  wrap 1 to alice: wss://alice.example
send: ok, 2 wraps
  rumor id: e8a203774e2d8cb13eba01dd4d2cc4e4132f36e2bcc736fe5aa6b9107199f490
  wrap 0: 1701 bytes, created_at 1793208120
    id: 4add4ac97201710b1b25554633c30733faeeddc2146351ca80a956608ccae9b2
    signed by: 2b4ea0a797a443d293ef5cff444f4979f06acfebd7e86d277475656138385b6c
  wrap 1: 1701 bytes, created_at 1793201120
    id: 1d91f94cb62d7d8da69757b0c3a26ea66a837db2059c79f53d520ed1d2e4bcac
    signed by: 4ce119c96e2fa357200b559b2f7dd5a5f02d5290aff74b03f3e471b273211c97
each addressee opens their copy
carol opens wrap 1 of the first: ok
  author: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
  kind 14, created_at 1793300000, members 3, content Are you going to the party tonight?
  room: 845f26439ac6d8a4a04cf5432f3bbb5e10e3e70ded1a5a85f0dc745396d1862f
  subject Party
alice opens her own copy of the first: ok
  author: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
  kind 14, created_at 1793300000, members 3, content Are you going to the party tonight?
  room: 845f26439ac6d8a4a04cf5432f3bbb5e10e3e70ded1a5a85f0dc745396d1862f
  subject Party
alice opens wrap 0 of the reply: ok
  author: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
  kind 14, created_at 1793300060, members 3, content Yes, at nine.
  room: 845f26439ac6d8a4a04cf5432f3bbb5e10e3e70ded1a5a85f0dc745396d1862f
  reply to: eb4521dd9b5c8e84b55ec8aa59f910613772fb0c7d584ae07cacd9aa2faac811
bob opens wrap 0 of the direct message: ok
  author: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
  kind 14, created_at 1793300120, members 2, content Bring the speaker.
  room: 21ef0f45560edb87008736f810a6a9ad4da6c8cf08afb70f98084559d1b71395
bob opens his own copy of the reply: ok
  author: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
  kind 14, created_at 1793300060, members 3, content Yes, at nine.
  room: 845f26439ac6d8a4a04cf5432f3bbb5e10e3e70ded1a5a85f0dc745396d1862f
  reply to: eb4521dd9b5c8e84b55ec8aa59f910613772fb0c7d584ae07cacd9aa2faac811
bob opens wrap 0 of the first: ok
  author: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
  kind 14, created_at 1793300000, members 3, content Are you going to the party tonight?
  room: 845f26439ac6d8a4a04cf5432f3bbb5e10e3e70ded1a5a85f0dc745396d1862f
  subject Party
bob's rooms
room: 845f26439ac6d8a4a04cf5432f3bbb5e10e3e70ded1a5a85f0dc745396d1862f
  inbox[2], created_at 1793300000
  inbox[1], created_at 1793300060
  subject: Party
room: 21ef0f45560edb87008736f810a6a9ad4da6c8cf08afb70f98084559d1b71395
  inbox[0], created_at 1793300120
  no subject
refusals
alice to bob and dave, plan: no list (dave)
send: no list at participant 1, 0 wraps
dave's list signed by bob, plan: list author (dave)
a kind 4 message, send: kind
nip04_encrypt: retired
alice's rumor sealed by carol: wrap (identity)
a signed rumor: wrap (rumor)
a wrap to dave of a rumor naming only bob: not member
bob opens carol's wrap: wrap (recipient)
NIP-17's example
the receiver opens the first wrap: ok
  author: 44900586091b284416a0c001f677f9c49f7639a55c3f1e2ec130a8e1a7998e1b
  kind 14, created_at 1703172058, members 2, content Hola, que tal?
  room: f883ba14095342f975f3e08f63cabcec3ee8beccc764958910873860a98d1008
the sender opens the second wrap: ok
  author: 44900586091b284416a0c001f677f9c49f7639a55c3f1e2ec130a8e1a7998e1b
  kind 14, created_at 1703172058, members 2, content Hola, que tal?
  room: f883ba14095342f975f3e08f63cabcec3ee8beccc764958910873860a98d1008
```

What the output shows:

- Three wraps for two recipients, each to its addressee's relays, each
  2,213 bytes, with the times worked out above. Each is signed by a
  different one-time key.
- Every copy opens to the same rumor id and the room `845f2643…96d1862f`;
  the direct message is in room `21ef0f45…d1b71395`.
- Bob opens his three copies out of order. `nip17_order` puts the first
  message before his reply, and the room's subject is "Party".
- Dave has no kind 10050 list, so the plan to Bob and Dave is refused
  (`no list (dave)`) and the send writes no wrap for anyone, Bob included.
- A list for Dave signed by Bob is refused (`list author`).
- A kind 4 message is refused (`kind`); NIP-04 is read-only, and
  `nip04_encrypt` returns its retired status.
- Alice's rumor sealed by Carol is refused (`identity`), a signed rumor is
  refused (`rumor`), a wrap to Dave whose rumor names only Bob is refused
  (`not member`), and Bob can't open Carol's wrap (`recipient`).
- NIP-17's two example wraps open with its published keys: the receiver's
  copy and the sender's copy hold the same rumor, "Hola, que tal?".

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/private-messages.out](../fixtures/private-messages.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/private-messages.coax](../examples/checks/private-messages.coax),
which loads the example. The independent values come from
[nip17_reference.py](../../coax/tests/nostr/nip17_reference.py), a
pure-Python NIP-17 written for this chapter on top of chapter 25's
[nip59_reference.py](../../coax/tests/nostr/nip59_reference.py). It shares
no code with Coax. Given the same keys and stand-in entropy, it builds the
three messages, opens every wrap as its addressee with all of NIP-59's
checks plus NIP-17's membership check, and prints the rumor ids, wrap ids
and room ids. It also decodes NIP-17's two published `nsec` keys with its
own bech32 and opens NIP-17's two example wraps.

The checks compare:

- all eight wraps from `nip17_send` with Python's wrap ids, and the first
  wrap byte for byte, so a Coax wrap opens in the Python client and the
  Python wrap opens in Coax;
- Python's first wrap opened by `nip17_open` with Python's rumor id, room
  and subject;
- NIP-17's two example wraps opened by Coax with the author, rumor id and
  room Python reads from them;
- Dave without a list: the plan and the send refused, no byte written and
  every entropy record cleared;
- Alice's rumor sealed by Carol refused as `NIP59_IDENTITY`, with `out`
  cleared.

<!-- source: nostr/examples/checks/private-messages.coax check -->
```jai
// What coax/tests/nostr/nip17_reference.py prints: a pure-Python NIP-17 on
// top of chapter 25's nip59_reference.py (integers, hashlib, hmac and
// base64; it shares no code with Coax), given the same test keys, nonces,
// aux bytes and time draws. Per message: the rumor id, then the wrap ids in
// plan order (recipients, then the sender's own copy).
PY_RUMOR :: [3] string.[
  "eb4521dd9b5c8e84b55ec8aa59f910613772fb0c7d584ae07cacd9aa2faac811",
  "385eb8c9c4a1d90c4cc63c8e7870467a1d8e440b2e848a9d6a2ba49b120965ea",
  "e8a203774e2d8cb13eba01dd4d2cc4e4132f36e2bcc736fe5aa6b9107199f490"];
PY_WRAP_IDS :: [8] string.[
  "96a2756d46a71cdeffe0ae578f22978543543dd97c83587e7e97c7775479edef",
  "fac32add0ca415b9e02809f5211880ecd145d6b0b496af1c185a61098d964ce7",
  "5f5b46637d1e022b24eee5d861c830e6e410ba77fa5beeb6a6b4b36217ddcb62",
  "00a06c6461d57c49014977230fb083054baef77240db3eaec0f6d97ae683a203",
  "df2bbc88f975ee8c4369b17b8b2033976fcf75aa0bbda4c652ba5a2d71b3bf95",
  "258c3ac91109800c4f1bb8892eba264c4addadadee071c92f02028d486ef281f",
  "4add4ac97201710b1b25554633c30733faeeddc2146351ca80a956608ccae9b2",
  "1d91f94cb62d7d8da69757b0c3a26ea66a837db2059c79f53d520ed1d2e4bcac"];
PY_ROOM_PARTY :: "845f26439ac6d8a4a04cf5432f3bbb5e10e3e70ded1a5a85f0dc745396d1862f";
PY_ROOM_DIRECT :: "21ef0f45560edb87008736f810a6a9ad4da6c8cf08afb70f98084559d1b71395";
// What the same script reads from NIP-17's two example wraps, with the
// keys decoded from NIP-17's nsec strings by its own bech32.
PY_NIP17_AUTHOR :: "44900586091b284416a0c001f677f9c49f7639a55c3f1e2ec130a8e1a7998e1b";
PY_NIP17_RUMOR_ID :: "cf4d60706f9681a31c1cd5850779bcabe1578c1ae293296be20748c2e0771749";
PY_NIP17_ROOM :: "f883ba14095342f975f3e08f63cabcec3ee8beccc764958910873860a98d1008";

same_hex :: (bytes: [32] u8, text: string) -> bool {
  want: [32] u8; decode_hex(text, want[..]);
  for i: 0..<32 { if bytes[i] != want[i] { return false; } }
  return true;
}
// A relay list record as a relay would hand it over after checking its
// signature; the plan reads only the kind, the signer and the tags.
list_of :: (owner: [32] u8, tags: string) -> NostrEvent { e := nt_event(tags); e.kind = N17_RELAY_LIST; e.public_key = owner; return e; }
// Sends message m and checks every wrap id against Python's, from index first.
sends_as_python :: (sender: [] u8, to: [] u8, lists: [] NostrEvent, m: int, at: int, tags: string, content: string, first: int) -> bool {
  me := x_only_key(sender); p := nip17_plan(me[..], to, lists);
  if p.error != N17_OK { return false; }
  fresh := fixed_entropy(m, p.recipients + 1); work: [NIP59_WORK] u8; out: [16384] u8;
  s := nip17_send(sender, p, at, 14, tags, content, fresh[0..<p.recipients + 1], work[..], out[..], false);
  if s.error != N17_OK || s.wraps != p.recipients + 1 || !same_hex(s.rumor_id, PY_RUMOR[m]) { return false; }
  for i: 0..<s.wraps {
    scratch: [NIP59_REGION] u8;
    e := nostr_event_read(wrap_of(s, out[..], i), scratch[..], false);
    if e.error != NE_OK || !same_hex(e.id, PY_WRAP_IDS[first + i]) { return false; }
  }
  return m != 0 || wrap_of(s, out[..], 0) == PY_WRAP_0;
}

#test "all eight wraps of the three messages are the Python reference's" #steps 4000000000 {
  alice := test_key(ALICE_HEX); bob := test_key(BOB_HEX); carol := test_key(CAROL_HEX);
  a := x_only_key(alice[..]); b := x_only_key(bob[..]); c := x_only_key(carol[..]);
  la := list_of(a, "[[\"relay\",\"wss://alice.example\"]]"); lb := list_of(b, "[[\"relay\",\"wss://inbox.nostr.wine\"]]");
  lc := list_of(c, "[[\"relay\",\"wss://carol.example\"]]");
  to: [64] u8; lists: [3] NostrEvent;
  for i: 0..<32 { to[i] = b[i]; to[32 + i] = c[i]; } lists[0] = lb; lists[1] = lc; lists[2] = la;
  #assert sends_as_python(alice[..], to[..], lists[..], 0, NOW, "[[\"subject\",\"Party\"]]", "Are you going to the party tonight?", 0);
  for i: 0..<32 { to[i] = a[i]; } lists[0] = la; lists[2] = lb;
  #assert sends_as_python(bob[..], to[..], lists[..], 1, NOW + 60, "[[\"e\",\"eb4521dd9b5c8e84b55ec8aa59f910613772fb0c7d584ae07cacd9aa2faac811\"]]", "Yes, at nine.", 3);
  lists[0] = lb; lists[1] = la;
  #assert sends_as_python(alice[..], b[..], lists[0..<2], 2, NOW + 120, "[]", "Bring the speaker.", 6);
}
#test "Python's first wrap opens for Bob in Python's room" #steps 1000000000 {
  bob := test_key(BOB_HEX); me := x_only_key(bob[..]); work: [NIP59_WORK] u8; out: [NIP59_REGION] u8;
  m := nip17_open(bob[..], me[..], PY_WRAP_0, work[..], out[..]);
  #assert m.error == N17_OK && m.members == 3 && same_hex(m.room, PY_ROOM_PARTY) && same_hex(m.rumor_id, PY_RUMOR[0]);
  #assert m.subject_n == 5 && cast(string) m.subject[0..<5] == "Party";
}
#test "NIP-17's two example wraps open with its keys to Python's rumor and room" #steps 2000000000 {
  sender := test_key(NIP17_SENDER_HEX); receiver := test_key(NIP17_RECEIVER_HEX);
  s := x_only_key(sender[..]); r := x_only_key(receiver[..]); work: [NIP59_WORK] u8; out: [NIP59_REGION] u8;
  m := nip17_open(receiver[..], r[..], NIP17_EXAMPLE_1, work[..], out[..]);
  #assert m.error == N17_OK && same_hex(m.author, PY_NIP17_AUTHOR) && same_hex(m.rumor_id, PY_NIP17_RUMOR_ID) && same_hex(m.room, PY_NIP17_ROOM);
  #assert cast(string) out[m.content_from..<m.content_to] == "Hola, que tal?" && m.created_at == 1703172058;
  m = nip17_open(sender[..], s[..], NIP17_EXAMPLE_2, work[..], out[..]);
  #assert m.error == N17_OK && same_hex(m.author, PY_NIP17_AUTHOR) && same_hex(m.rumor_id, PY_NIP17_RUMOR_ID) && same_hex(m.room, PY_NIP17_ROOM);
  // Each wrap is for one key only.
  #assert nip17_open(sender[..], s[..], NIP17_EXAMPLE_1, work[..], out[..]).wrap_error == NIP59_RECIPIENT;
}
#test "Dave, with no relay list, makes the send write nothing for anyone" #steps 1000000000 {
  alice := test_key(ALICE_HEX); a := x_only_key(alice[..]); bob := test_key(BOB_HEX); b := x_only_key(bob[..]);
  dave := test_key(DAVE_HEX); d := x_only_key(dave[..]);
  to: [64] u8; for i: 0..<32 { to[i] = b[i]; to[32 + i] = d[i]; }
  lists: [3] NostrEvent; lists[0] = list_of(b, "[[\"relay\",\"wss://inbox.nostr.wine\"]]"); lists[2] = list_of(a, "[[\"relay\",\"wss://alice.example\"]]");
  p := nip17_plan(a[..], to[..], lists[..]);
  #assert p.error == N17_NO_LIST && p.who == 1;
  fresh := fixed_entropy(3, 3); work: [NIP59_WORK] u8; out: [4096] u8;
  s := nip17_send(alice[..], p, NOW, 14, "[]", "Party?", fresh[0..<3], work[..], out[..], false);
  #assert s.error == N17_NO_LIST && s.who == 1 && s.wraps == 0;
  for x: out { #assert x == 0; }
  for i: 0..<3 { #assert fresh[i].wrap_key[31] == 0 && fresh[i].seal_back == 0; }
  // An empty list is the same as none.
  lists[1] = list_of(d, "[]"); #assert nip17_plan(a[..], to[..], lists[..]).error == N17_NO_LIST;
}
#test "a rumor by Alice sealed by Carol is refused as identity" #steps 1000000000 {
  alice := test_key(ALICE_HEX); carol := test_key(CAROL_HEX); bob := test_key(BOB_HEX);
  a := x_only_key(alice[..]); b := x_only_key(bob[..]);
  pre: [NIP59_REGION] u8; body: [NIP59_REGION] u8; id: [32] u8; forged: [NIP59_REGION] u8;
  work: [NIP59_WORK] u8; out: [NIP59_REGION] u8;
  n := n59_rumor(a[..], NOW, 14, BOB_TAG, "I am Alice.", pre[..], body[..], id[..]);
  m := nip17_open(bob[..], b[..], forge(carol[..], b[..], body[0..<n], forged[..]), work[..], out[..]);
  #assert m.error == N17_WRAP && m.wrap_error == NIP59_IDENTITY;
  for x: out { #assert x == 0; }
}
#test "same_hex notices a change in the first or last byte" {
  b: [32] u8; zero := "0000000000000000000000000000000000000000000000000000000000000000";
  #assert same_hex(b, zero);
  b[0] = 1; #assert !same_hex(b, zero);
  b[0] = 0; b[31] = 1; #assert !same_hex(b, zero);
}
```

### NIP-17's example

Every id in NIP-17's example is correct. Both wraps' ids and signatures
verify, both seals' ids and signatures verify, and the rumor's id
`cf4d6070…e0771749` is the hash of its fields. The published keys decode to
sender `44900586…a7998e1b` and receiver `918e2da9…755f0788`, matching the
wraps' `p` tags. One thing differs from NIP-17's advice: both seals carry
`created_at` 1703172058, the rumor's own time, where NIP-17 says the seal's
time SHOULD be randomized. The reader accepts it; a SHOULD is not a
refusal.

### The library's own checks

`coax test coax/lib/nostr/nip17.coax` runs 24 checks of its own (and the
checks of every file it loads): the relay list reader on NIP-17's example
list, missing, empty, foreign and malformed lists, the 8-relay and 256-byte
limits; the plan's member and list rules; the caller's tags and kind 15's
file tags; the rumor's tags; the room id for every order of the same
members; reading an opened rumor, with repeated `p` tags, eleven members
and twelve; the room order with ties; and the send's refusals before key
work. Four checks do curve work: a signing key that doesn't match the
plan's sender; a send from Alice to Bob that makes two wraps of one rumor,
the second addressed to Alice, and that refuses as `N17_OUTPUT` with `out`
cleared when only the first fits; nostr-effect's wrap from chapter 25
opened as a message of Alice and Bob's room; and a wrapped kind 1 rumor
refused with `out` cleared.

`nip17.coax` is `#authority`: it decides who receives a message and where
it goes. `coax mutate coax/lib/nostr/nip17.coax --root-only`, run on
2026-11-01 in two shards of about five minutes each, made 628 mutants and
killed all 628. The first runs left survivors in the send loop, which then
had no successful-send check, and in comparisons whose mutants couldn't
change a result (a 33-byte slice compared over 32 bytes, `n < 0` where
`n` is never 0); the checks and the code were changed until none
survived, without marking any mutant equivalent.

### Interoperability

"A message from Coax opens in an independent NIP-17 client and the reverse"
holds for the Python client above: its wraps are byte for byte the Coax
wraps, each opens in the other, and NIP-17's published wraps, made by
another client, open in Coax. No deployed client (Amethyst, 0xchat) was
run for this chapter, and no wrap was published to a live relay.

## What is not built

- Kind 7 reactions and kind 5 deletions inside a room, edits, and
  disappearing messages (an `expiration` tag on each wrap and seal, or no
  copy for the sender).
- The relay side: serving kind 1059 only to the `p`-tagged user after
  NIP-42 AUTH.
- The file itself. Kind 15's tags are checked, but this stack has no
  AES-GCM, so the file's bytes are neither encrypted nor decrypted here.
- NIP-A4 kind 24 public messages: plaintext, `p` tags for the receivers,
  sent to their NIP-65 inbox relays and the sender's outbox (chapter 32).
- NIP-EE, MLS groups: `unrecommended`, superseded by the Marmot protocol,
  and listed as retired in chapter 70's table.

## Summary

- `nip17_plan` reads every participant's kind 10050 list first; one
  missing, empty or foreign list refuses the whole message.
- `nip17_send` makes N+1 wraps of one kind 14 or 15 rumor, the last for the
  sender, and refuses every other kind, kind 4 included.
- `nip17_open` relies on NIP-59's identity and unsigned-rumor checks, then
  reads the room and refuses a message whose room leaves the reader out.
- `n17_room` and `nip17_order` name a room by its member set and order its
  messages by time and rumor id.
- The wraps match an independent Python NIP-17 byte for byte, and NIP-17's
  own example opens.

## Dependencies

- [25. Gift wrap](ch025-gift-wrap.md)
- [32. Many relays and the outbox
  model](ch032-many-relays-and-the-outbox-model.md)

## Sources

- [NIP-17](../nips/official/17.md)
- [NIP-59](../nips/official/59.md)
- [NIP-A4](../nips/official/A4.md)
- [NIP-EE](../nips/official/EE.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
