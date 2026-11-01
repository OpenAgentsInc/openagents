# 79. Agents, skills and automation

An agent is saved, carried privately to the host that runs it, and started
on a schedule or when something changes. Four OpenAgents NIPs describe
those steps. NIP-AGENT defines a saved agent: a name, instructions, a
runtime ID and the access it asks for, carried privately in a kind 3188
artifact or published as kind 30187. NIP-SKILL publishes reviewed
`SKILL.md` listings as kind 30188. NIP-SOV gives an agent a durable
identity with custody, guardians and a treasury. NIP-AUTO turns an
owner-signed plan into occurrences: once, on fixed intervals, or for each
new revision a source observes. This chapter builds the saved agent's
reader, its private carriage both ways, the host's pinned selection, and
the automation controller's decision about what fires, and runs them
against artifacts sealed by an independent implementation.

Part X, Every NIP. Result: NIP-AGENT definitions (every field's contract),
private carriage (seal and open of the 3188 artifact) and pinned
selection, and NIP-AUTO scheduling (once, interval and source triggers,
missed and occupied slots recorded once, one active occurrence, authority
per occurrence, pause, resume and cancel). Public agents, NIP-SKILL
listings, NIP-SOV and AUTO's continuation, budgets and journals are not
built yet; see [What is not built](#what-is-not-built).

## What this chapter teaches

- A saved agent is inert configuration: a definition grants nothing, and
  an unknown field, schema or access value refuses.
- Private carriage: the artifact envelope's digest covers the payload's
  JCS bytes, NIP-44 v2 encrypts it to one recipient, and the 3188 names
  that recipient and a mailbox.
- Pinned selection: the host chooses a signer, definition id, revision and
  digest explicitly, so a newer revision never replaces the one it
  assigned.
- Automation slots: slot n of an interval plan is due at
  `first_at + n × every`, checked for overflow, and only the latest due
  slot can run. The slots before it are recorded as one skipped range.
- Why "recorded once, not run twice" follows from one counter that only
  grows: a duplicate timer delivery or a second tick finds nothing new.
- One occurrence per source revision: a redelivered webhook is the same
  revision, so it is the same occurrence.
- Every occurrence needs current authority, here chapter 77's
  `cap_dispatch` against the agent's exact definition digest. A refusal
  blocks the plan, and only an explicit resume unblocks it.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [definition.coax](../../coax/lib/nostr/agent/definition.coax) | `agent_read(payload, a)` | Reads a NIP-AGENT definition payload and checks every field's contract |
| [definition.coax](../../coax/lib/nostr/agent/definition.coax) | `agent_select(pin, signer, digest, d)` | The host's pinned selection: signer, id, revision, digest, not deleted |
| [carriage.coax](../../coax/lib/nostr/agent/carriage.coax) | `agent_seal(payload, sender, recipient_x, mailbox, nonce, aux, ...)` | Seals a payload into a signed kind 3188 private artifact |
| [carriage.coax](../../coax/lib/nostr/agent/carriage.coax) | `agent_open(text, recipient, recipient_x, pin, w, a)` | Opens a 3188 and applies the pin, every check in order |
| [schedule.coax](../../coax/lib/nostr/auto/schedule.coax) | `auto_tick(p, s, skips, occ, now, authority)` | A timer tick for a once or interval plan: what runs, what is skipped |
| [schedule.coax](../../coax/lib/nostr/auto/schedule.coax) | `auto_observe(p, s, occ, obs, now, authority)` | A source observation: one occurrence per revision |
| [schedule.coax](../../coax/lib/nostr/auto/schedule.coax) | `auto_settle`, `auto_control` | The active occurrence settled; pause, resume and cancel |

All three files are new with this chapter, and each is an `#authority`
file: the definition reader and selection decide which configuration a
host admits, the carriage decides whether an artifact is accepted, and the
scheduler decides what runs. The checks came first, and a root-only
mutation run kills every mutant (see
[How the code is checked](#how-the-code-is-checked)). The scheduler loads
no crypto, so its checks run in milliseconds; the carriage reuses chapter
21's event reader, chapter 11's BIP-340, chapter 20's NIP-44 v2 and the
shared artifact envelope (`envelope.coax`).

## Saved agents

A NIP-AGENT payload is a JSON object with exactly these fields:

```text
{"schema": "openagents.agent.definition.v1",
 "definition": {"id": "<64 hex>", "revision": 3,
                "spec": {"name": "Triage", "description": "Sorts new issues",
                         "instructions": "Label each new issue: bug, feature or question.",
                         "runtime": "codex", "model": null, "access": "read_only"},
                "deleted": false}}
```

A fork adds `forked_from: {published, revision}`. The spec's contract:

| Field | Rule `agent_read` enforces |
| --- | --- |
| `name` | Not blank, at most 80 bytes, no control characters |
| `description` | At most 512 bytes, no control characters |
| `instructions` | Not blank, at most 65,536 bytes, no NUL |
| `runtime` | 1 to 128 ASCII letters, digits, `.`, `_`, `:` or `-`; never a command or URL |
| `model` | `null`, or 1 to 256 bytes without control characters; `null` when runtime is `any` |
| `access` | `host_default`, `read_only` or `workspace_write` |
| `fallbacks` | Optional, at most 4 `{runtime, model?}`, each distinct, not the primary, not `sdk` or `any` |
| `autonomy` | Optional object; its known numbers in range (`every_minutes` 5 to 1440, `stall_minutes` 5 to 240, `shell_timeout_secs` 10 to 3600, `coalesce_secs` 0 to 1800, at most 16 `wakes`); unknown fields and wakes ignored |

Everything else refuses: an unknown field anywhere outside `autonomy`, a
repeated key, another schema, another access value. A definition cannot
add executable paths, credentials, spending or approval authority, and a
field such as `grants` is simply unknown. Grants belong to the account
that grants them, not to the agent.

The reader returns one record, with its strings unescaped into the
caller's arena:

<!-- source: coax/lib/nostr/agent/definition.coax record -->
```jai
AgText :: struct {buf:[]u8;at:int;}
AgentDefinition :: struct {
  error:u32=AG_SHAPE;
  id:string; revision:int; deleted:bool;
  name:string; description:string; instructions:string;
  runtime:string; model:string; has_model:bool; access:int;
  fallbacks:int; forked:bool; forked_from:string; forked_revision:int;
}
AgentPin :: struct {signer:string;id:string;revision:int;digest:string;}
```

<!-- source: coax/lib/nostr/agent/definition.coax codes -->
```jai
AG_OK :: cast(u32)0;
AG_SHAPE :: cast(u32)1;     // not the payload's shape: a missing, repeated, unknown or mistyped field
AG_SCHEMA :: cast(u32)2;    // not openagents.agent.definition.v1
AG_FIELD :: cast(u32)3;     // a field breaks its contract (bounds, grammar, enum)
AG_SIGNER :: cast(u32)4;    // selection: not the chosen signer
AG_ID :: cast(u32)5;        // selection: another definition
AG_REVISION :: cast(u32)6;  // selection: another revision, newer or older
AG_DIGEST :: cast(u32)7;    // selection: other bytes under the pinned revision
AG_DELETED :: cast(u32)8;   // selection: the pinned revision is a deletion

AG_SCHEMA_ID :: "openagents.agent.definition.v1";
AG_HOST_DEFAULT :: 0; AG_READ_ONLY :: 1; AG_WORKSPACE_WRITE :: 2;
```

The spec is read field by field. Each key is matched against the list of
eight names, a repeated or unknown key refuses as `shape`, and each value
is checked against its row of the table above:

<!-- source: coax/lib/nostr/agent/definition.coax spec -->
```jai
AG_SPEC :: [8]string.["name","description","instructions","runtime","model","access","fallbacks","autonomy"];
ag_spec :: (j:[]Json,d:[]AgentDefinition,a:[]AgText)->u32 {
  seen:=0;fb:[4]string;
  t:=json_next(j);
  while t==JSON_STRING {
    f:=-1;k:=json_text(j);for i:0..<8 {if k==AG_SPEC[i] {f=i;}}
    if f<0 || (seen&(1<<f))!=0 {return AG_SHAPE;}
    seen|=1<<f;v:=json_next(j);
    if f==6 {if v!=JSON_ARRAY {return AG_SHAPE;}e:=ag_fallbacks(j,d,fb[..],a);if e!=AG_OK {return e;}}
    else if f==7 {if v!=JSON_OBJECT {return AG_SHAPE;}e:=ag_autonomy(j);if e!=AG_OK {return e;}}
    else if !(f==4 && v==JSON_NULL) {
      if v!=JSON_STRING {return AG_SHAPE;}
      s:=ag_string(j,a);if !s.ok {return AG_SHAPE;}
      if f==0 {if !ag_nonblank(s.s) || s.s.count>80 || !ag_clean(s.s,false) {return AG_FIELD;}d[0].name=s.s;}
      if f==1 {if s.s.count>512 || !ag_clean(s.s,false) {return AG_FIELD;}d[0].description=s.s;}
      // At most 65,536 bytes: the JSON reader refuses a longer raw string.
      if f==2 {if !ag_nonblank(s.s) || !ag_clean(s.s,true) {return AG_FIELD;}d[0].instructions=s.s;}
      if f==3 {if !ag_runtime_ok(s.s) {return AG_FIELD;}d[0].runtime=s.s;}
      if f==4 {if !ag_model_ok(s.s) {return AG_FIELD;}d[0].model=s.s;d[0].has_model=true;}
      if f==5 {d[0].access=ag_access(s.s);if d[0].access<0 {return AG_FIELD;}}
    }
    t=json_next(j);
  }
  if t!=JSON_OBJECT_END || (seen&63)!=63 {return AG_SHAPE;}
  // runtime any leaves the choice to the host: its model is null.
  if d[0].runtime=="any" && d[0].has_model {return AG_FIELD;}
  for i:0..<d[0].fallbacks {if fb[i]==d[0].runtime {return AG_FIELD;}}
  return AG_OK;
}
```

The instructions limit needs no comparison of its own: the JSON reader
refuses a string longer than 65,536 raw bytes, and unescaping only shrinks
a string. The cost is that instructions near the limit that need escapes
refuse rather than being admitted. The spec raised the limit from 16,384
bytes on 2026-10-18; a reader from before then refuses a longer definition.

### Pinned selection

Reading a definition is not choosing it. NIP-AGENT has the consumer verify
"an explicitly selected signer, definition ID, revision, and digest". The
signer is the artifact's verified signer: an account ID is never proof that
a Nostr key controls that account, so the host names the key itself. A
newer revision cannot silently replace the assigned one, and a deletion
prevents new assignments:

<!-- source: coax/lib/nostr/agent/definition.coax select -->
```jai
// The consumer's pinned selection. signer is the verified 3188 signer and
// digest the artifact digest the envelope checked.
agent_select :: (pin:AgentPin,signer:string,digest:string,d:AgentDefinition)->u32 {
  if d.error!=AG_OK {return d.error;}
  if signer!=pin.signer {return AG_SIGNER;}
  if d.id!=pin.id {return AG_ID;}
  if d.revision!=pin.revision {return AG_REVISION;}
  if digest!=pin.digest {return AG_DIGEST;}
  if d.deleted {return AG_DELETED;}
  return AG_OK;
}
```

`AG_OK` from `agent_select` is still not permission to run. The host checks
the current account, project and access before dispatch.

## Private carriage

A private agent travels in the shared private artifact envelope (kind
3188). The event has exactly one `p` tag (the recipient), one `h` tag (a
random 64-hex mailbox) and `t: oa:artifact:v1`. Its content is NIP-44 v2,
encrypted from the signer's key to the recipient's. The plaintext is the
artifact envelope:

```text
{"v": "openagents.artifact-envelope.v1", "requires": [],
 "artifact": {"digest": "sha256:<64 hex>", "size": 334,
              "media_type": "application/json",
              "schema": "openagents.agent.definition.v1"},
 "inline": { ...the payload... },
 "issued_at": 1790000000, "retain_until": 1790086400}
```

The digest is SHA-256 over the payload's RFC 8785 JCS bytes (keys sorted,
no whitespace), and the size is their length. The inline object may keep
its own key order: the digest is over the canonical form, not the bytes as
written.

<!-- source: coax/lib/nostr/agent/carriage.coax record -->
```jai
// The private workspace: every plaintext byte and the conversation key live
// here, and both seal and open leave it all zero.
AgentWork :: struct {pre:[AC_REGION]u8;body:[AC_REGION]u8;cipher:[2*AC_REGION]u8;scratch:[AC_REGION]u8;ck:[32]u8;}
AgentOpened :: struct {
  error:u32=AC_INPUT; selection:u32;
  definition:AgentDefinition;
  signer:string; digest:string; mailbox:string; id:string;
}
```

<!-- source: coax/lib/nostr/agent/carriage.coax codes -->
```jai
AC_OK :: cast(u32)0;
AC_INPUT :: cast(u32)1;      // a key or the work view is not admitted
AC_EVENT :: cast(u32)2;      // not a NIP-01 event with its own id
AC_SIGNATURE :: cast(u32)3;
AC_ARTIFACT :: cast(u32)4;   // not a 3188 private artifact for exactly this recipient
AC_KEY :: cast(u32)5;        // the recipient secret is refused
AC_DECRYPT :: cast(u32)6;    // the content does not decrypt for this recipient
AC_ENVELOPE :: cast(u32)7;   // not an artifact envelope of an agent definition, or its digest or size differ
AC_DEFINITION :: cast(u32)8; // the payload is refused: definition.error says why
AC_SELECTION :: cast(u32)9;  // not the pinned signer, id, revision or digest: selection says which

AC_REGION :: 131072;
AC_TOPIC :: "oa:artifact:v1";
```

Sealing builds the envelope with `envelope_build_checked`, derives the
conversation key from the sender's secret and the recipient's x-only key,
encrypts with a caller-supplied nonce, and signs the 3188 with a
caller-supplied aux. Every plaintext byte and the conversation key live in
the `AgentWork` the caller passes in, and it is cleared before the
function returns:

<!-- source: coax/lib/nostr/agent/carriage.coax seal -->
```jai
// Seals payload from sender to recipient_x under mailbox (64 hex). nonce
// and aux are 32 fresh random bytes each from the host. The signed event
// JSON goes to out; returns its length, or -1.
agent_seal :: (payload:string,sender:[]u8,recipient_x:[]u8,mailbox:string,nonce:[]u8,aux:[]u8,created_at:int,issued_at:int,retain_until:int,w:[]AgentWork,out:[]u8)->int {
  if sender.count!=32 || recipient_x.count!=32 || nonce.count!=32 || aux.count!=32 || w.count!=1 || !ag_hex64(mailbox) {return -1;}
  n:=ac_seal(payload,sender,recipient_x,mailbox,nonce,aux,created_at,issued_at,retain_until,w,out);
  ac_clear(w);return n;
}
ac_seal :: (payload:string,sender:[]u8,recipient_x:[]u8,mailbox:string,nonce:[]u8,aux:[]u8,created_at:int,issued_at:int,retain_until:int,w:[]AgentWork,out:[]u8)->int {
  b:=envelope_build_checked(payload,AG_SCHEMA_ID,issued_at,retain_until,w[0].scratch[..],w[0].body[..],false);
  if b.error!=AE_OK {return -1;}
  ew:[1]EcdhScratch;
  if nip44_conversation_key_from_keys_checked(sender,recipient_x,ew[..],w[0].ck[..],false)!=NIP44_OK {return -1;}
  e:=nip44_encrypt_checked(NIP44_COMPAT,w[0].ck[..],nonce,w[0].body[0..<b.written],w[0].cipher[..],w[0].pre[..],false);
  if e.error!=NIP44_OK {return -1;}
  rhex:[64]u8;r:=hex_encode_checked(cast(string)recipient_x,rhex[..],false);
  tags:[256]u8;
  n:=json_raw(tags[..],0,"[[\"p\",\"");n=json_raw(tags[..],n,cast(string)rhex[..]);n=json_raw(tags[..],n,"\"],[\"h\",\"");
  n=json_raw(tags[..],n,mailbox);n=json_raw(tags[..],n,"\"],[\"t\",\"oa:artifact:v1\"]]");
  sw:[1]NostrSignWork;k:=EVENT_MORE;
  while k==EVENT_MORE {k=nostr_event_sign_step(sender,aux,created_at,3188,cast(string)tags[0..<n],cast(string)w[0].pre[0..<e.written],sw[..],w[0].scratch[..],out,false);}
  return k;
}
```

Opening does what NIP-AGENT asks of the consumer, in order: the original
signature, the recipient, decryption, the artifact digest, the schema, the
payload's bounds, and last the explicit selection. A failure at any step
returns its own code, and nothing later runs:

<!-- source: coax/lib/nostr/agent/carriage.coax open -->
```jai
// Opens a 3188 for the recipient whose secret and x-only key are given,
// then applies the pin. Strings of the result live in the arena.
agent_open :: (text:string,recipient:[]u8,recipient_x:[]u8,pin:AgentPin,w:[]AgentWork,a:[]AgText)->AgentOpened {
  r:AgentOpened;
  if recipient.count!=32 || recipient_x.count!=32 || w.count!=1 {return r;}
  e:=nostr_event_read(text,w[0].pre[..],false);
  if e.error!=NE_OK {return ac_done(r,AC_EVENT,w);}
  if !nostr_fast_verify_vartime(e.id[..],e.public_key[..],e.signature[..]) {return ac_done(r,AC_SIGNATURE,w);}
  ew:[1]EcdhScratch;peer:=e.public_key;
  c:=nip44_conversation_key_from_keys_checked(recipient,peer[..],ew[..],w[0].ck[..],false);
  if c!=NIP44_OK {return ac_done(r,ifx c==NIP44_SECRET then AC_KEY else AC_DECRYPT,w);}
  return ac_open(text,e,recipient_x,pin,w,a);
}
// After the signature and the conversation key: recipient, decryption,
// envelope, payload and pin.
ac_open :: (text:string,e:NostrEvent,recipient_x:[]u8,pin:AgentPin,w:[]AgentWork,a:[]AgText)->AgentOpened {
  r:AgentOpened;
  rhex:[64]u8;mail:[64]u8;peer:=e.public_key;eid:=e.id;
  hr:=hex_encode_checked(cast(string)recipient_x,rhex[..],false);
  if e.kind!=3188 || !ac_tags(text[e.tags_from..<e.tags_to],cast(string)rhex[..],mail[..]) {return ac_done(r,AC_ARTIFACT,w);}
  n:=json_unescape(text[e.content_from..<e.content_to],w[0].scratch[..]);
  d:=nip44_decrypt_checked(NIP44_COMPAT,w[0].ck[..],cast(string)w[0].scratch[0..<n],w[0].cipher[..],w[0].body[..],false);
  if d.error!=NIP44_OK {return ac_done(r,AC_DECRYPT,w);}
  envelope:=cast(string)w[0].body[0..<d.written];
  info:=envelope_check_body_checked(envelope,AG_SCHEMA_ID,w[0].scratch[..],false);
  if info.error!=AE_OK {return ac_done(r,AC_ENVELOPE,w);}
  r.definition=agent_read(envelope[info.inline_from..<info.inline_to],a);
  if r.definition.error!=AG_OK {return ac_done(r,AC_DEFINITION,w);}
  r.signer=ac_hex(a,peer[..]);r.id=ac_hex(a,eid[..]);r.mailbox=ac_keep(a,cast(string)mail[..]);
  r.digest=ac_keep(a,ac_digest(envelope));
  r.selection=agent_select(pin,r.signer,r.digest,r.definition);
  if r.selection!=AG_OK {return ac_done(r,AC_SELECTION,w);}
  return ac_done(r,AC_OK,w);
}
// The artifact's digest: the string after the first "digest" two levels
// down, inside "artifact". The envelope check has already compared it with
// the inline payload's JCS bytes, and no other string at that depth can be
// "digest" in an admitted agent envelope. It is ASCII: the raw token is it.
ac_digest :: (envelope:string)->string {
  j:[1]Json;json_start(j[..],envelope);t:=json_next(j[..]);
  while t!=JSON_END && t!=JSON_ERROR {
    if t==JSON_STRING && j[0].depth==2 && json_text(j[..])=="digest" {t=json_next(j[..]);return json_text(j[..]);}
    t=json_next(j[..]);
  }
  return "";
}
```

The tag check counts: exactly one `p`, which must be this recipient;
exactly one `h`, which must be 64 lowercase hex; and the topic. A tag
named `px` or a `p` in a value position is not a `p` tag.

<!-- source: coax/lib/nostr/agent/carriage.coax tags -->
```jai
// Exactly one ["p", recipient], one ["h", <64-hex>] (copied to mailbox) and
// a ["t", "oa:artifact:v1"].
ac_tags :: (tags:string,recipient_hex:string,mailbox:[]u8)->bool {
  decoded:[256]u8;j:[1]Json;json_start(j[..],tags);
  if json_next(j[..])!=JSON_ARRAY {return false;}
  ps:=0;hs:=0;topic:=false;
  t:=json_next(j[..]);
  while t==JSON_ARRAY {
    k:=0;name:="";v:=json_next(j[..]);
    while v==JSON_STRING {
      n:=json_unescape(json_text(j[..]),decoded[..]);if n<0 {return false;}
      value:=cast(string)decoded[0..<n];
      if k==0 {name=ifx value=="p" then "p" else ifx value=="h" then "h" else ifx value=="t" then "t" else "-";}
      if k==1 {
        if name=="p" {ps+=1;if value!=recipient_hex {return false;}}
        if name=="h" {hs+=1;if !ag_hex64(value) {return false;}for i:0..<64 {mailbox[i]=decoded[i];}}
        if name=="t" && value==AC_TOPIC {topic=true;}
      }
      k+=1;v=json_next(j[..]);
    }
    if v!=JSON_ARRAY_END {return false;}
    t=json_next(j[..]);
  }
  return t==JSON_ARRAY_END && ps==1 && hs==1 && topic;
}
```

Sharing the encrypted envelope grants no execution rights, and `AC_OK`
means only that the artifact is exactly what the pinned signer sealed for
this host. Plaintext must fit NIP-44's compatible 65,535 bytes after the
envelope's own fields; a definition with instructions near the 65,536-byte
limit needs a separately admitted artifact reference, which is not built.

## Automation

A NIP-AUTO plan is owner-signed and immutable. Among its fields are a
trigger, an active interval (`starts_at < expires_at`) and a positive
`max_occurrences`. The trigger is one of four closed variants:

| Trigger | When an occurrence is due |
| --- | --- |
| `once` | One slot at `at`, inside the active interval |
| `interval` | Slot n at `first_at + n × every_seconds`; `missed` is `skip` or `latest` |
| `source` | When a read-only source observes a new revision of one resource |
| `continuation` | After a checker says `continue` (not built here) |

The controller decides with its own trusted clock; signed event times
never extend anything. Only one occurrence may be active per plan.

<!-- source: coax/lib/nostr/auto/schedule.coax record -->
```jai
AutoPlan :: struct {
  trigger:int; at:int; first_at:int; every:int; missed:int;
  starts_at:int; expires_at:int; max_occurrences:int;
  source:string; resource:string; max_age:int;
}
AutoState :: struct {status:int;revision:int;next_slot:int;occurrences:int;skips:int;active:int;}
AutoSkip :: struct {first_slot:int;last_slot:int;first_due:int;last_due:int;reason:int;}
AutoOccurrence :: struct {ordinal:int;slot:int;due_at:int;revision:string;settled:bool;}
AutoObservation :: struct {source:string;resource:string;revision:string;available:bool;observed_at:int;}
AutoResult :: struct {code:u32;ordinal:int;skipped:int;}
```

<!-- source: coax/lib/nostr/auto/schedule.coax codes -->
```jai
AU_OK :: cast(u32)0;          // an occurrence admitted (a missed range may be recorded first)
AU_IDLE :: cast(u32)1;        // nothing new is due
AU_BUSY :: cast(u32)2;        // an occurrence is active: latest waits, a source revision waits
AU_SKIPPED :: cast(u32)3;     // due slots recorded as occupied or paused, nothing admitted
AU_BLOCKED :: cast(u32)4;     // authority refused: the plan is blocked, no slot consumed
AU_EXPIRED :: cast(u32)5;     // the plan expired: no new dispatch
AU_EXHAUSTED :: cast(u32)6;   // max_occurrences reached
AU_CLOSED :: cast(u32)7;      // the plan is not active
AU_DUPLICATE :: cast(u32)8;   // this source revision already has its occurrence
AU_STALE :: cast(u32)9;       // the observation is older than max_age, or from the future
AU_UNAVAILABLE :: cast(u32)10;// the source reported no revision
AU_MALFORMED :: cast(u32)11;  // a bad plan, the wrong call for the trigger, another source or resource
AU_LIMIT :: cast(u32)12;      // no room left in the record
AU_CONFLICT :: cast(u32)13;   // a control's expected revision is not current

AU_ONCE :: 0; AU_INTERVAL :: 1; AU_SOURCE :: 2;               // trigger
AU_SKIP :: 0; AU_LATEST :: 1;                                  // missed-slot handling
AUS_ACTIVE :: 0; AUS_PAUSED :: 1; AUS_BLOCKED :: 2; AUS_EXHAUSTED :: 3;
AUS_COMPLETE :: 4; AUS_CANCELLED :: 5; AUS_EXPIRED :: 6;       // status
AUR_MISSED :: 0; AUR_OCCUPIED :: 1; AUR_PAUSED :: 2; AUR_EXPIRED :: 3; AUR_BOUND :: 4; // skip reason
AU_PAUSE :: 0; AU_RESUME :: 1; AU_CANCEL :: 2;                 // control
AU_MAX_SAFE :: 9007199254740991;
```

### Slots and their due times

Slot n's due time is checked for overflow before it is computed. A plan is
admitted only with sane bounds:

<!-- source: coax/lib/nostr/auto/schedule.coax due -->
```jai
// Slot n's due time, or -1 when there is no such slot or it overflows.
auto_due :: (p:AutoPlan,slot:int)->int {
  if slot<0 {return -1;}
  if p.trigger==AU_ONCE {return ifx slot==0 then p.at else -1;}
  if p.every<=0 || p.first_at<0 {return -1;}
  if slot>(AU_MAX_SAFE-p.first_at)/p.every {return -1;}
  return p.first_at+slot*p.every;
}
// The latest slot due at t (t at or after the first slot), and the last
// slot before expiry.
auto_slot_at :: (p:AutoPlan,t:int)->int {
  if p.trigger==AU_ONCE {return 0;}
  return (t-p.first_at)/p.every;
}
auto_plan_ok :: (p:AutoPlan)->bool {
  if p.starts_at<0 || p.starts_at>=p.expires_at || p.expires_at>AU_MAX_SAFE || p.max_occurrences<=0 {return false;}
  if p.trigger==AU_ONCE {return p.at>=p.starts_at && p.at<p.expires_at;}
  if p.trigger==AU_INTERVAL {return p.every>0 && p.first_at>=p.starts_at && p.first_at<p.expires_at && (p.missed==AU_SKIP || p.missed==AU_LATEST);}
  if p.trigger==AU_SOURCE {return p.source!="" && p.resource!="" && p.max_age>0;}
  return false;
}
```

### What a tick decides

NIP-AUTO is explicit that a suspended machine does not acquire a right to
replay a backlog. When the controller ticks, it finds the latest due slot,
`(now - first_at) / every`, and the first slot it has not handled,
`next_slot`. Then:

- Nothing new is due (`next_slot` is past the latest due slot): idle. This
  is why a duplicate tick, or a timer delivered twice, does nothing.
- The plan has expired: the slots still unhandled are recorded once as
  `expired`, and the plan stops.
- The plan is paused, or a `skip` plan has an occurrence running: the due
  slots are recorded as `paused` or `occupied`, and nothing runs.
- A `latest` plan has an occurrence running: it waits (busy). After the
  occurrence settles, the next tick runs only the latest due slot.
- Authority is refused: the plan is blocked, and no slot is consumed.
- `max_occurrences` is reached: the rest is recorded as `bound`, and the
  plan is exhausted.
- Otherwise the slots before the latest due one are recorded as one
  `missed` range, and the latest due slot becomes the next occurrence.

Every record is checked for room before anything changes, and every
accepted change moves the state's revision once.

<!-- source: coax/lib/nostr/auto/schedule.coax tick -->
```jai
// A timer tick at now for a once or interval plan.
auto_tick :: (p:AutoPlan,s:[]AutoState,skips:[]AutoSkip,occ:[]AutoOccurrence,now:int,authority:u32)->AutoResult {
  if !auto_plan_ok(p) || p.trigger==AU_SOURCE {return au_code(AU_MALFORMED);}
  status:=s[0].status;
  if au_terminal(status) || status==AUS_BLOCKED {return au_code(AU_CLOSED);}
  next:=s[0].next_slot;
  if now<auto_due(p,0) {return au_code(AU_IDLE);}
  last:=auto_slot_at(p,p.expires_at-1);
  if now>=p.expires_at {
    // No new dispatch after expiry: the slots still unhandled are recorded once.
    room:=next>last || s[0].skips<skips.count;
    if !room {return au_code(AU_LIMIT);}
    n:=0;if next<=last {au_skip(p,s,skips,next,last,AUR_EXPIRED);s[0].next_slot=last+1;n=1;}
    s[0].status=AUS_EXPIRED;s[0].revision+=1;r:=au_code(AU_EXPIRED);r.skipped=n;return r;
  }
  due:=auto_slot_at(p,now);
  if next>due {return au_code(AU_IDLE);}
  if status==AUS_PAUSED || (s[0].active!=0 && p.missed==AU_SKIP) {
    if s[0].skips>=skips.count {return au_code(AU_LIMIT);}
    au_skip(p,s,skips,next,due,ifx status==AUS_PAUSED then AUR_PAUSED else AUR_OCCUPIED);
    s[0].next_slot=due+1;s[0].revision+=1;r:=au_code(AU_SKIPPED);r.skipped=1;return r;
  }
  if s[0].active!=0 {r:=au_code(AU_BUSY);r.ordinal=s[0].active;return r;}
  if authority!=AU_OK {s[0].status=AUS_BLOCKED;s[0].revision+=1;return au_code(AU_BLOCKED);}
  if s[0].occurrences>=p.max_occurrences {
    if s[0].skips>=skips.count {return au_code(AU_LIMIT);}
    au_skip(p,s,skips,next,last,AUR_BOUND);s[0].next_slot=last+1;
    s[0].status=AUS_EXHAUSTED;s[0].revision+=1;r:=au_code(AU_EXHAUSTED);r.skipped=1;return r;
  }
  missed:=ifx next<due then 1 else 0;
  if s[0].skips+missed>skips.count || s[0].occurrences>=occ.count {return au_code(AU_LIMIT);}
  if missed==1 {au_skip(p,s,skips,next,due-1,AUR_MISSED);}
  s[0].next_slot=due+1;
  r:=au_admit(s,occ,due,auto_due(p,due),"");r.skipped=missed;return r;
}
```

### Source observations

A source plan admits work for an available, fresh observation of exactly
its configured source and resource. The key is the revision: a webhook
delivered twice, or a poll that sees the same version again, finds the
occurrence it already created. Revisions are opaque and never ordered.

<!-- source: coax/lib/nostr/auto/schedule.coax observe -->
```jai
// An observation event for a source plan.
auto_observe :: (p:AutoPlan,s:[]AutoState,occ:[]AutoOccurrence,obs:AutoObservation,now:int,authority:u32)->AutoResult {
  if !auto_plan_ok(p) || p.trigger!=AU_SOURCE {return au_code(AU_MALFORMED);}
  if s[0].status!=AUS_ACTIVE {return au_code(AU_CLOSED);}
  if obs.source!=p.source || obs.resource!=p.resource {return au_code(AU_MALFORMED);}
  if !obs.available || obs.revision=="" {return au_code(AU_UNAVAILABLE);}
  if obs.observed_at>now || now-obs.observed_at>p.max_age {return au_code(AU_STALE);}
  // One occurrence per (source, resource, revision): a redelivery is the same one.
  for i:0..<s[0].occurrences {if occ[i].revision==obs.revision {r:=au_code(AU_DUPLICATE);r.ordinal=occ[i].ordinal;return r;}}
  if now>=p.expires_at {s[0].status=AUS_EXPIRED;s[0].revision+=1;return au_code(AU_EXPIRED);}
  if now<p.starts_at {return au_code(AU_IDLE);}
  if s[0].active!=0 {r:=au_code(AU_BUSY);r.ordinal=s[0].active;return r;}
  if authority!=AU_OK {s[0].status=AUS_BLOCKED;s[0].revision+=1;return au_code(AU_BLOCKED);}
  if s[0].occurrences>=p.max_occurrences {s[0].status=AUS_EXHAUSTED;s[0].revision+=1;return au_code(AU_EXHAUSTED);}
  if s[0].occurrences>=occ.count {return au_code(AU_LIMIT);}
  return au_admit(s,occ,-1,now,obs.revision);
}
```

### Settling and controls

<!-- source: coax/lib/nostr/auto/schedule.coax settle -->
```jai
// The active occurrence settled (its RUN terminal record is the caller's).
// A once plan, or an interval plan with no slot left, is then complete.
auto_settle :: (p:AutoPlan,s:[]AutoState,occ:[]AutoOccurrence,ordinal:int)->u32 {
  if ordinal<=0 || s[0].active!=ordinal {return AU_MALFORMED;}
  occ[ordinal-1].settled=true;s[0].active=0;s[0].revision+=1;
  if s[0].status==AUS_ACTIVE && p.trigger!=AU_SOURCE && s[0].next_slot>auto_slot_at(p,p.expires_at-1) {s[0].status=AUS_COMPLETE;}
  return AU_OK;
}
// Pause, resume or cancel, compared against the current revision.
auto_control :: (s:[]AutoState,action:int,expected_revision:int,authority:u32)->u32 {
  if expected_revision!=s[0].revision {return AU_CONFLICT;}
  status:=s[0].status;
  if action==AU_PAUSE {
    if status!=AUS_ACTIVE {return AU_CLOSED;}
    s[0].status=AUS_PAUSED;
  } else if action==AU_RESUME {
    if status!=AUS_PAUSED && status!=AUS_BLOCKED {return AU_CLOSED;}
    if authority!=AU_OK {return AU_BLOCKED;}
    if s[0].active!=0 {return AU_BUSY;} // a prior occurrence is not reconciled
    s[0].status=AUS_ACTIVE;
  } else if action==AU_CANCEL {
    if au_terminal(status) {return AU_CLOSED;}
    s[0].status=AUS_CANCELLED;
  } else {return AU_MALFORMED;}
  s[0].revision+=1;return AU_OK;
}
```

A `once` plan, or an interval plan with no slot left before expiry, is
complete when its last occurrence settles. Scheduled completion means only
that the scheduled work ran; it does not say the goal was achieved. A
resume needs explicit authority again and a reconciled prior occurrence;
no later timer resumes a plan by itself.

### Worked example: two events and one tick, by hand

The host holds the owner's saved agent Triage at revision 3, digest
`sha256:eb39…3a72`, and three plans that run it. All three were admitted
at T0 = 1,790,000,000 and expire at T0 + 24 h, except the report, which
expires at T0 + 5 h:

| Plan | Trigger | Record so far |
| --- | --- | --- |
| hourly | interval, `first_at` T0, every 3600 s, `skip` | slot 0 ran at T0 and settled; `next_slot` 1 |
| watch | source `sha256:5017`, resource `repo:issues`, `max_age` 300 s | nothing yet |
| report | once at T0 + 14,400 | nothing yet |

The authority each occurrence needs is chapter 77's `cap_dispatch` for
`agent:triage` (definition digest `sha256:eb39…3a72`, one binding to that
exact digest, the controller's grant `g-auto` for reads and 600 s). It
dispatches, so the authority code is 0.

The machine sleeps and wakes at now = T0 + 14,500. Two events arrive, then
one tick:

1. **Event 1**, `repo:issues` revision `r-118` observed at T0 + 14,450.
   It is the configured source and resource, available, and
   14,500 − 14,450 = 50 s old, within 300. No occurrence has that revision
   and none is active, so watch admits occurrence #1 for `r-118`.
2. **Event 2**, the same revision redelivered, observed at T0 + 14,490.
   Occurrence #1 already has `r-118`: duplicate, and nothing new runs.
3. **The tick, hourly.** The latest due slot is
   ⌊14,500 / 3600⌋ = 4 (due T0 + 14,400). `next_slot` is 1, so slots 1, 2
   and 3, due T0 + 3600, T0 + 7200 and T0 + 10,800, are recorded as one
   `missed` range, and slot 4 becomes occurrence #2. `next_slot` becomes 5.
4. **The tick, report.** Its one slot was due at T0 + 14,400 ≤ now, before
   expiry: occurrence #1 for slot 0.

The same tick delivered again finds `next_slot` 5 > 4 for hourly and
`next_slot` 1 > 0 for report: both idle. The missed slots 1 to 3 are
recorded once and never run, and slot 4 runs once.

## What is not built

- **Public agents.** No kind 0 agent profile, no 30187 public definition,
  no NIP-OA owner attestation (`auth` tag), no `status: revoked`, no
  runtime evidence (`att-release`, `att-endpoint`) and no 3204 gateway
  receipts.
- **Assignments and sessions.** No assignment book, runtime fallback
  order, SESS command identities or coder-sync session.
- **Large definitions.** A payload that does not fit NIP-44's 65,535 bytes
  needs a separately admitted artifact reference (`inline: null`); the
  carriage refuses it.
- **NIP-SKILL.** No reader for 30188 listings (`d`, `version`, `x`, `r`,
  `t`, `status: withdrawn`), no newest-per-directory selection with the
  withdrawn tie-break, and no document digest check at install. A listing
  grants nothing either way.
- **NIP-SOV.** No immutable profile, bounded activation, checkpoints,
  NIP-46 custody, guardian quorum, treasury limits or migration. Custody of
  an agent's on-chain coins is NIP-KEYS, [chapter
  104](ch104-agent-treasuries-and-a-network-of-keys.md).
- **AUTO beyond scheduling.** The plan, control, state and skipped-range
  wire records and their signatures are not read; `schedule.coax` takes
  their fields. No continuation trigger or checker, no budgets or
  reservations, no COORD proposals and claims per occurrence, no CJ
  dispatch, no source polls on interval slots or revalidation before
  dispatch, no RUN journal, controller takeover or recovery. Cron,
  time zones and calendars are outside AUTO v1 too.

## The example

[nostr/examples/agents-automation.coax](../examples/agents-automation.coax)
runs the chapter at a fixed clock. The host opens the owner's 3188
(sealed in Python), seals the same payload itself and opens that, checks
its authority, and then runs the worked example and more refusals.

The host's side of the carriage, and the plans:

<!-- source: nostr/examples/agents-automation.coax open -->
```jai
// The host's side: its own secret, and the pin it chose when the owner
// assigned the agent: signer, definition id, revision 3 and that digest.
host_pin :: ()->AgentPin {
  p:AgentPin;p.signer=ACV_SENDER_X;p.id=ACV_AGENT_ID;p.revision=3;p.digest=ACV_DIGEST;return p;
}
show_open :: (label:string,text:string,work:[]AgentWork,store:[]u8) {
  a:=[1]AgText.[AgText.{buf=store}];
  sk:=key_of(ACV_RECIPIENT_SECRET);x:=key_of(ACV_RECIPIENT_X);
  r:=agent_open(text,sk[..],x[..],host_pin(),work,a[..]);
  if r.error==AC_SELECTION {print("%: selection: %\n",label,AG_WORDS[r.selection]);}
  else if r.error==AC_DEFINITION {print("%: definition: %\n",label,AG_WORDS[r.definition.error]);}
  else {print("%: %\n",label,AC_WORDS[r.error]);}
}
```

<!-- source: nostr/examples/agents-automation.coax plans -->
```jai
// Three automations of the saved agent, all admitted at T0.
hourly :: ()->AutoPlan {
  p:AutoPlan;p.trigger=AU_INTERVAL;p.first_at=T0;p.every=HOUR;p.missed=AU_SKIP;
  p.starts_at=T0;p.expires_at=T0+24*HOUR;p.max_occurrences=24;return p;
}
watch :: ()->AutoPlan {
  p:AutoPlan;p.trigger=AU_SOURCE;p.source="sha256:5017";p.resource="repo:issues";p.max_age=300;
  p.starts_at=T0;p.expires_at=T0+24*HOUR;p.max_occurrences=10;return p;
}
report :: ()->AutoPlan {
  p:AutoPlan;p.trigger=AU_ONCE;p.at=T0+4*HOUR;p.starts_at=T0;p.expires_at=T0+5*HOUR;p.max_occurrences=1;return p;
}
observation :: (revision:string,at:int)->AutoObservation {
  o:AutoObservation;o.source="sha256:5017";o.resource="repo:issues";o.revision=revision;o.available=true;o.observed_at=at;return o;
}
// The host's capability record (chapter 77): the agent's definition by its
// exact digest, one binding, and the controller's grant.
host_authority :: (revoked:bool,now:int)->u32 {
  d:=[1]CapDefinition.[CapDefinition.{id="agent:triage",digest=ACV_DIGEST,profile=CAP_EXECUTOR,effects=CAP_READS,minimum_wall_ms=1000}];
  b:=[1]CapBinding.[CapBinding.{binding="b-triage",definition=ACV_DIGEST,profile=CAP_EXECUTOR,invoke=true,presence=CAP_PRESENT,host=CAP_WALL_MS}];
  g:=[1]CapGrant.[CapGrant.{grant="g-auto",issuer=CONTROLLER,principal=CONTROLLER,binding="b-triage",effects=CAP_READS,wall_ms=600000,expires_at=T0+24*HOUR,revoked=revoked}];
  deny:[1]string;p:CapPolicy;p.owner=CONTROLLER;p.deny=deny[0..<0];p.assurance=CAP_HOST_ENFORCED;p.wall_ms=600000;
  q:CapRequest;q.principal=CONTROLLER;q.definition="agent:triage";q.effects=CAP_READS;q.required=CAP_WALL_MS;q.wall_ms=300000;
  return cap_dispatch(d[..],b[..],g[..],p,q,now).error;
}
```

<!-- source: nostr/examples/agents-automation.coax main -->
```jai
main :: () {
  work:[1]AgentWork;store:[8192]u8;
  print("AGENT: the host opens the owner's 3188\n");
  a:=[1]AgText.[AgText.{buf=store[..]}];
  sk:=key_of(ACV_RECIPIENT_SECRET);x:=key_of(ACV_RECIPIENT_X);
  r:=agent_open(ACV_SEALED,sk[..],x[..],host_pin(),work[..],a[..]);
  d:=r.definition;
  print("status: %, event %…, signer %…, mailbox %…\n",AC_WORDS[r.error],r.id[0..<16],r.signer[0..<16],r.mailbox[0..<16]);
  print("definition %…, revision %, deleted %\n",d.id[0..<16],d.revision,d.deleted);
  print("name \"%\", runtime %, model %, access %\n",d.name,d.runtime,ifx d.has_model then d.model else "null",ACCESS_WORDS[d.access]);
  print("instructions \"%\"\n",d.instructions);
  print("digest %\n",r.digest);

  print("AGENT: Coax seals the same payload and opens it\n");
  sender:=key_of(ACV_SENDER_SECRET);nonce:[32]u8;aux:[32]u8;for i:0..<32 {nonce[i]=0x5a;aux[i]=0xa5;}
  sealed:[8192]u8;
  n:=agent_seal(ACV_PAYLOAD,sender[..],x[..],ACV_MAILBOX,nonce[..],aux[..],T0,T0,T0+86400,work[..],sealed[..]);
  store2:[8192]u8;b:=[1]AgText.[AgText.{buf=store2[..]}];
  back:=agent_open(cast(string)sealed[0..<n],sk[..],x[..],host_pin(),work[..],b[..]);
  print("sealed % bytes; opened: %, same digest: %, same instructions: %\n",n,AC_WORDS[back.error],back.digest==r.digest,back.definition.instructions==d.instructions);

  print("AUTO: authority for the agent's definition\n");
  print("grant g-auto: %; the same grant revoked: %\n",CAP_WORDS[host_authority(false,T0+4*HOUR+100)],CAP_WORDS[host_authority(true,T0+4*HOUR+100)]);

  print("AUTO: the hourly check ran slot 0 at T0 and settled\n");
  ph:=hourly();sh:[1]AutoState;kh:[8]AutoSkip;oh:[8]AutoOccurrence;
  show("hourly at T0",auto_tick(ph,sh[..],kh[..],oh[..],T0,host_authority(false,T0)),kh[..],sh[0],oh[..]);
  settled:=auto_settle(ph,sh[..],oh[..],1);
  print("#1 settled: %; the machine sleeps until T0 + 4 h + 100 s\n",settled==AU_OK);

  print("AUTO: two events and one tick, now = T0 + 14500\n");
  now:=T0+4*HOUR+100;cap:=host_authority(false,now);
  pw:=watch();sw:[1]AutoState;ow:[8]AutoOccurrence;
  pr:=report();sr:[1]AutoState;kr:[2]AutoSkip;orr:[2]AutoOccurrence;
  show("event 1, issues revision r-118 observed at T0 + 14450",auto_observe(pw,sw[..],ow[..],observation("r-118",T0+14450),now,cap),kh[0..<0],sw[0],ow[..]);
  show("event 2, the same revision redelivered at T0 + 14490",auto_observe(pw,sw[..],ow[..],observation("r-118",T0+14490),now,cap),kh[0..<0],sw[0],ow[..]);
  show("tick, hourly",auto_tick(ph,sh[..],kh[..],oh[..],now,cap),kh[..],sh[0],oh[..]);
  show("tick, report",auto_tick(pr,sr[..],kr[..],orr[..],now,cap),kr[..],sr[0],orr[..]);
  print("AUTO: the same tick delivered again\n");
  show("tick, hourly",auto_tick(ph,sh[..],kh[..],oh[..],now,cap),kh[..],sh[0],oh[..]);
  show("tick, report",auto_tick(pr,sr[..],kr[..],orr[..],now,cap),kr[..],sr[0],orr[..]);
  print("hourly: % occurrences, % skipped range, next slot %; watch: %; report: %\n",sh[0].occurrences,sh[0].skips,sh[0].next_slot,sw[0].occurrences,sr[0].occurrences);
  print("AUTO: a new revision while #1 runs, then after it settles\n");
  show("event 3, r-119 at T0 + 14520",auto_observe(pw,sw[..],ow[..],observation("r-119",T0+14520),T0+14530,cap),kh[0..<0],sw[0],ow[..]);
  print("#1 settles: %\n",auto_settle(pw,sw[..],ow[..],1)==AU_OK);
  show("event 3 again after #1 settles",auto_observe(pw,sw[..],ow[..],observation("r-119",T0+14520),T0+14540,cap),kh[0..<0],sw[0],ow[..]);

  print("broken inputs\n");
  t:[8192]u8;s:=ACV_SEALED;for i:0..<s.count {t[i]=s[i];}
  at:=-1;for i:0..<s.count-12 {if at<0 && s[i..<i+12]=="\"created_at\"" {at=i+13;}}
  t[at]='2';
  show_open("3188 with its created_at changed",cast(string)t[0..<s.count],work[..],store2[..]);
  show_open("3188 for another host",ACV_FOR_OTHER,work[..],store2[..]);
  show_open("3188 encrypted to another key",ACV_WRONG_KEY,work[..],store2[..]);
  show_open("3188 whose digest names revision 4's bytes",ACV_BAD_DIGEST,work[..],store2[..]);
  show_open("3188 whose payload adds grants",ACV_GRANTS,work[..],store2[..]);
  show_open("revision 4 under the revision 3 pin",ACV_SEALED_R4,work[..],store2[..]);
  stale:[1]AutoState;so:[2]AutoOccurrence;
  show("issues observed at T0 + 14100, 400 s old",auto_observe(pw,stale[..],so[..],observation("r-120",T0+14100),now,cap),kh[0..<0],stale[0],so[..]);
  other:=observation("r-120",now);other.resource="repo:pulls";
  show("an observation of repo:pulls",auto_observe(pw,stale[..],so[..],other,now,cap),kh[0..<0],stale[0],so[..]);
  blocked:[1]AutoState;bk:[2]AutoSkip;bo:[2]AutoOccurrence;
  show("report with the grant revoked",auto_tick(pr,blocked[..],bk[..],bo[..],now,host_authority(true,now)),bk[..],blocked[0],bo[..]);
  print("then: % occurrences, status blocked: %\n",blocked[0].occurrences,blocked[0].status==AUS_BLOCKED);
  late:[1]AutoState;lk:[2]AutoSkip;lo:[2]AutoOccurrence;
  show("report first ticked at T0 + 5 h",auto_tick(pr,late[..],lk[..],lo[..],T0+5*HOUR,cap),lk[..],late[0],lo[..]);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/agents-automation.coax --run-steps 250000000
```

Its output, in debug and release builds alike
([nostr/fixtures/agents-automation.out](../fixtures/agents-automation.out)):

```text
AGENT: the host opens the owner's 3188
status: ok, event c1700e5f0f810af2…, signer 79be667ef9dcbbac…, mailbox 108ab669f200b0ce…
definition 68b32644f8a9097e…, revision 3, deleted false
name "Triage", runtime codex, model null, access read_only
instructions "Label each new issue: bug, feature or question."
digest sha256:eb391bd808c5b1feed374f3bf5594a858e7e9bf206227108405a6ddde50f3a72
AGENT: Coax seals the same payload and opens it
sealed 1457 bytes; opened: ok, same digest: true, same instructions: true
AUTO: authority for the agent's definition
grant g-auto: dispatch; the same grant revoked: revoked
AUTO: the hourly check ran slot 0 at T0 and settled
hourly at T0: admit, occurrence #1 for slot 0 due T0 + 0
#1 settled: true; the machine sleeps until T0 + 4 h + 100 s
AUTO: two events and one tick, now = T0 + 14500
event 1, issues revision r-118 observed at T0 + 14450: admit, occurrence #1 for revision r-118
event 2, the same revision redelivered at T0 + 14490: duplicate, occurrence #1
tick, hourly: admit, skipped slots 1..3 (due T0 + 3600 .. T0 + 10800, missed), occurrence #2 for slot 4 due T0 + 14400
tick, report: admit, occurrence #1 for slot 0 due T0 + 14400
AUTO: the same tick delivered again
tick, hourly: idle
tick, report: idle
hourly: 2 occurrences, 1 skipped range, next slot 5; watch: 1; report: 1
AUTO: a new revision while #1 runs, then after it settles
event 3, r-119 at T0 + 14520: busy, occurrence #1
#1 settles: true
event 3 again after #1 settles: admit, occurrence #2 for revision r-119
broken inputs
3188 with its created_at changed: event id
3188 for another host: not for this recipient
3188 encrypted to another key: decrypt
3188 whose digest names revision 4's bytes: envelope
3188 whose payload adds grants: definition: shape
revision 4 under the revision 3 pin: selection: revision
issues observed at T0 + 14100, 400 s old: stale
an observation of repo:pulls: malformed
report with the grant revoked: blocked
then: 0 occurrences, status blocked: true
report first ticked at T0 + 5 h: expired, skipped slots 0..0 (due T0 + 14400 .. T0 + 14400, expired)
```

The first part is the round trip of "Done means": the Python-sealed 3188
opens to revision 3 under the pin, and the 3188 Coax seals from the same
payload opens to the same digest and instructions. The two events and the
tick match the worked example, and the same tick again does nothing.

The broken inputs are the deliberately wrong cases. Changing the first
digit of `created_at` changes the bytes the id covers, so the reader
refuses it as `event id` before any signature or decryption. The 3188 for
another host fails the tag check; the one encrypted to another key fails
NIP-44's MAC. The envelope whose digest names revision 4's bytes fails the
digest comparison. The payload that adds `grants` reads as an unknown
field (`shape`). Revision 4, valid in every other way, is refused by the
pin as `revision`. The observation 400 s old is stale; one of `repo:pulls`
is another resource; the revoked grant blocks the report without
consuming its slot; and the report first ticked at T0 + 5 h has expired,
so its slot is recorded as `expired`, never run.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/agents-automation.coax](../examples/checks/agents-automation.coax),
which loads the example:

- The 3188 ids and the artifact digest from
  [agent_carriage_vectors.py](../../coax/tests/nostr/agent_carriage_vectors.py),
  which seals with
  [nip59_reference.py](../../coax/tests/nostr/nip59_reference.py)'s
  integer secp256k1, BIP-340 and NIP-44 v2 and hashes with `hashlib` over
  `json.dumps(sort_keys=True)` (JCS for this payload). The Coax reader must
  open the Python artifact, recompute its id and find the same digest.
- The digest Coax's envelope builder writes for the payload equals the
  Python digest.
- The worked example's arithmetic by hand.

NIP-AGENT and NIP-AUTO publish no event examples or vectors, so there are
no published ids to compare with (and none to record as wrong); the Python
artifacts are the independent check of the wire.

<!-- source: nostr/examples/checks/agents-automation.coax check -->
```jai
// From coax/tests/nostr/agent_carriage_vectors.py, which seals with
// nip59_reference.py's integer secp256k1, BIP-340 and NIP-44 v2 (no code
// shared with Coax): the 3188's id as hashlib.sha256 of json.dumps([0,
// pubkey, created_at, kind, tags, content], separators=(",", ":")), and the
// artifact digest as hashlib.sha256 of json.dumps(payload, sort_keys=True,
// separators=(",", ":")), RFC 8785 JCS for this payload.
PY_SEALED_ID :: "c1700e5f0f810af232d5a5ea81a6ccb3ed40ffdeae28a418bfa6e212f9afb309";
PY_DIGEST :: "sha256:eb391bd808c5b1feed374f3bf5594a858e7e9bf206227108405a6ddde50f3a72";
PY_R4_ID :: "a60a0df3982dade89941117468d9c858568202d31286bf28704a01330882dc0d";

#test "the 3188's id and the artifact digest are the Python reference's" #steps 400000000 {
  work:[1]AgentWork;store:[8192]u8;a:=[1]AgText.[AgText.{buf=store[..]}];
  sk:=key_of(ACV_RECIPIENT_SECRET);x:=key_of(ACV_RECIPIENT_X);
  r:=agent_open(ACV_SEALED,sk[..],x[..],host_pin(),work[..],a[..]);
  #assert r.error==AC_OK && r.id==PY_SEALED_ID && r.digest==PY_DIGEST && ACV_DIGEST==PY_DIGEST;
  // Revision 4 opens as far as the pin and is its own event.
  b:=[1]AgText.[AgText.{buf=store[..]}];
  q:=agent_open(ACV_SEALED_R4,sk[..],x[..],host_pin(),work[..],b[..]);
  #assert q.error==AC_SELECTION && q.selection==AG_REVISION && q.id==PY_R4_ID && q.definition.revision==4;
}
#test "the digest is SHA-256 over the payload's canonical bytes" {
  // The payload's keys sorted (JCS) and hashed here with Coax's SHA-256:
  // the same digest the envelope builder writes.
  scratch:[4096]u8;out:[8192]u8;
  e:=envelope_build_checked(ACV_PAYLOAD,AG_SCHEMA_ID,T0,T0+86400,scratch[..],out[..],false);
  #assert e.error==AE_OK && ac_digest(cast(string)out[0..<e.written])==PY_DIGEST;
}

// By hand: slot n of the hourly plan is due at T0 + 3600 n. At T0 + 14500
// the latest due slot is floor(14500 / 3600) = 4; slot 0 ran, so 1, 2 and 3
// (due T0 + 3600, 7200, 10800) are one missed range and slot 4 runs.
#test "the worked example's values, by hand" {
  ph:=hourly();sh:[1]AutoState;kh:[4]AutoSkip;oh:[4]AutoOccurrence;
  #assert auto_tick(ph,sh[..],kh[..],oh[..],T0,AU_OK).code==AU_OK && auto_settle(ph,sh[..],oh[..],1)==AU_OK;
  r:=auto_tick(ph,sh[..],kh[..],oh[..],T0+14500,AU_OK);
  #assert r.code==AU_OK && kh[0].first_slot==1 && kh[0].last_slot==3 && kh[0].first_due==T0+3600 && kh[0].last_due==T0+10800;
  #assert oh[1].slot==4 && oh[1].due_at==T0+14400;
  #assert auto_tick(ph,sh[..],kh[..],oh[..],T0+14500,AU_OK).code==AU_IDLE && sh[0].skips==1 && sh[0].occurrences==2;
  // 14500 - 14450 = 50 s old, within 300; 14500 - 14100 = 400 s, not.
  pw:=watch();sw:[1]AutoState;ow:[4]AutoOccurrence;
  #assert auto_observe(pw,sw[..],ow[..],observation("r-118",T0+14450),T0+14500,AU_OK).code==AU_OK;
  #assert auto_observe(pw,sw[..],ow[..],observation("r-118",T0+14490),T0+14500,AU_OK).code==AU_DUPLICATE;
  #assert auto_observe(pw,sw[..],ow[..],observation("r-120",T0+14100),T0+14500,AU_OK).code==AU_STALE;
  #assert host_authority(false,T0+14500)==CAP_OK && host_authority(true,T0+14500)==CAP_REVOKED;
}
```

```shell
coax/bin/coax test nostr/examples/checks/agents-automation.coax --run-steps 250000000
```

`coax test` runs 498 checks, including the loaded files' own checks; all pass.

### The library's own checks

`definition.coax` checks every spec field at its boundary (80 and 81
bytes, 512 and 513, 128 and 129, 256 and 257, the autonomy ranges' edges),
control characters, blank text, every unknown or repeated key at each
level, the fork fields, the revision's ceiling (2^53 − 1 and 2^53), the
fallbacks rules, the arena running out, and selection by each pinned field.
`carriage.coax` opens the Python artifacts: the valid one, another
recipient, another kind, a wrong digest, an unknown payload field, the
wrong key, a changed id and a changed signature; it checks the tag rules,
refused views and inputs on both sides, an oversized payload and a short
output, the Coax round trip, and that the workspace is cleared on every
return. `schedule.coax` checks due times and overflow, plan bounds, the
missed range recorded once, occupied and coalesced slots, blocked and
resumed plans, pause, expiry and the bound recording the rest once, once
plans, settling, record room before any change, and source duplicates,
staleness, resource and expiry.

```shell
coax/bin/coax test coax/lib/nostr/agent/definition.coax --library
coax/bin/coax mutate coax/lib/nostr/agent/definition.coax --root-only --library
coax/bin/coax test coax/lib/nostr/agent/carriage.coax --library --run-steps 250000000
coax/bin/coax mutate coax/lib/nostr/agent/carriage.coax --root-only --library --run-steps 250000000 --shard 1/2
coax/bin/coax mutate coax/lib/nostr/agent/carriage.coax --root-only --library --run-steps 250000000 --shard 2/2
coax/bin/coax test coax/lib/nostr/auto/schedule.coax --library
coax/bin/coax mutate coax/lib/nostr/auto/schedule.coax --root-only --library
```

On 2026-11-01, `definition.coax` passed its 13 checks and root-only `coax mutate` killed all 528 mutants; `carriage.coax` passed its 14 checks and killed all 217 (two shards of 109 and 108); `schedule.coax` passed its 18 checks and killed all 379. None survived and none is marked equivalent. The test builders live in `coax/tests/nostr/agent_definition_fixtures.coax`, `agent_carriage_fixtures.coax` and `auto_schedule_fixtures.coax`, so root-only mutation mutates the library code. The example matches its transcript in debug and release. The full mutation run (`COAX_MUTATION_FULL=1`) and the whole-book example run were not run.

## Summary

- A saved agent is inert: `agent_read` admits exactly NIP-AGENT's fields
  and bounds, and anything unknown refuses.
- `agent_seal` and `agent_open` carry it privately in a 3188: one
  recipient, one mailbox, NIP-44 v2, and an envelope whose digest covers
  the payload's JCS bytes. The round trip returns the same definition and
  digest.
- `agent_select` pins signer, id, revision and digest, so a newer revision
  never silently replaces the assigned one.
- `auto_tick` runs only the latest due slot and records the rest as one
  skipped range; `next_slot` only grows, so a missed occurrence is recorded
  once and never run twice.
- `auto_observe` keys source work by revision, so a redelivery is the same
  occurrence; every occurrence needs current authority, and a refusal
  blocks the plan until an explicit resume.

## Dependencies

- [76. Work, coordination and control](ch076-work-coordination-and-control.md)
- [77. Capabilities, policy and
  extensions](ch077-capabilities-policy-and-extensions.md)

## Sources

- [OpenAgents NIP-AGENT](../nips/openagents/NIP-AGENT.md)
- [OpenAgents NIP-SKILL](../nips/openagents/NIP-SKILL.md)
- [OpenAgents NIP-SOV](../nips/openagents/NIP-SOV.md)
- [OpenAgents NIP-AUTO](../nips/openagents/NIP-AUTO.md)
- [OpenAgents shared contracts](../nips/openagents/contracts.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
