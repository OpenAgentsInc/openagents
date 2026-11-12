# 77. People who talk back

> **Status: partly implemented.** [aethyr/talk/](../talk/README.md) supplies
> a bounded native CPU profile: authored characters, JSON requests and
> replies, one pending request per conversation, scripted fallback, and an
> admitted conversation record replayed without send intentions. Debug and
> release run the same worked transport fixture. Every realm with characters
> writes its question and reads its answer through
> [wire.coax](../talk/wire.coax), the NPC ask's one codec (protocol step E4);
> the conversation lifecycle is not adopted yet. Hosted models, persistent
> memory, permissions, live UI and voice retain their separate integration
> checks. See [Still to do](#still-to-do).

A question can take seconds to answer while a realm keeps stepping.
The reply belongs to one player, one character and one request. If the player
leaves, an old reply must not become the next character's words. If the model
fails, the character still has a line to say.

This chapter builds that lifecycle and its request boundary. A reply contains
display text. It has no field or pointer through which to grant an item, move
a character or change a rule.

## What this chapter teaches

- Admit a character and a complete UTF-8 question under explicit byte limits.
- Bind a reply to its request and choose a fallback at a fixed-step deadline.
- Record accepted words and their apply ticks, then replay them without sending.
- Keep dialogue separate from a typed command or semantic selector.

## A character must have a fallback

Chapter 6's `EntityId` names the character. The authored strings remain the
caller's data; a pending conversation copies its question and fallback into
owned storage. Required text cannot be empty or consist only of ASCII
whitespace. Text preserves its original bytes: admission does not clip or trim
it. UTF-8 must be well formed; ASCII control bytes other than tab, newline and
carriage return are refused.

<!-- source: aethyr/talk/character.coax character -->
```jai
TALK_NAME_BYTES :: 64;
TALK_ROLE_BYTES :: 256;
TALK_KNOWLEDGE_BYTES :: 1200;
TALK_LINE_BYTES :: 600;
TALK_QUESTION_BYTES :: 512;
TALK_WORLD_BYTES :: 96;
TalkCharacter :: struct {
  id: EntityId;
  name: string;
  role: string;
  knowledge: string;
  greeting: string;
  fallback: string;
}
TalkCharacterStatus :: enum u8 { ok; identity; name; role; knowledge; greeting; fallback; }
```

| Field | Maximum UTF-8 bytes | May be empty? |
| --- | ---: | --- |
| Name | 64 | No |
| Role | 256 | No |
| Authored knowledge | 1,200 | Yes |
| Greeting or fallback | 600 | No |
| Question | 512 | No |
| World | 96 | No |
| Accepted reply | 600 | No |

These are this client profile's limits. The server applies its own request
and daily-cap rules, which the NPC ask's codec holds (below). Byte limits are
distinct from the server's character-count truncation. A valid 600-byte reply
fits a conversation line whole; a longer one does not fit the line, so the
character says its fallback.

## The request, worked by hand

[npc_ask_write](../talk/wire.coax) writes the `POST /api/playtest/npc/ask`
body, the schema `openagents.verse.npc.v1alpha1.NpcAsk`
([ask.proto](../../proto/openagents/verse/npc/v1alpha1/ask.proto)). Who the
character is belongs to the server: the body names the character by its id,
and the place by a built-in `world` or a made realm's `realm`. Then come an
optional `visit` stamp and `scene`, the `history` and the `question`, in that
order. Every realm with characters calls this one writer, and the bytes are
pinned in [proto/verse/npc-ask-v1.json](../../proto/verse/npc-ask-v1.json).

With no visit, no scene and an empty history, the JSON punctuation and field
names occupy 50 bytes, excluding the contents of the three strings:

| String | Minimal request | Mara in the example |
| --- | ---: | ---: |
| Character id | `m`: 1 | `mara`: 4 |
| World | `w`: 1 | `lamp-cove`: 9 |
| Question | `?`: 1 | `Where is the cellar key?`: 24 |
| String bytes | 3 | 37 |
| Complete request | 50 + 3 = **53** | 50 + 35 = **85** |

The example checks its 85-byte request. A quote or backslash is escaped; a
control byte in any text is written as a space, as the pages have always
sent it.

The writer refuses before writing anything: an id outside `[a-z0-9-]` or over
64 bytes, both or neither of `world` and `realm`, more than 40 history lines,
a question with nothing but spaces, and text that is not UTF-8. The server
refuses each of these too. The whole body must fit in 24,576 bytes, the
server's own limit, or in the caller's buffer; a body that does not fit
leaves the caller's output unchanged.

## A request owns its reply

The session owns its lines and record storage. Eight history lines retain the
last four complete exchanges; a full eight-exchange record refuses another
question instead of discarding replay data.

<!-- source: aethyr/talk/ask.coax session -->
```jai
TALK_HISTORY :: 8;
TALK_EXCHANGES :: 8;
TALK_SERIAL_LIMIT :: 1000000;
TalkLine :: struct {bytes:[TALK_LINE_BYTES]u8;n:int;from_npc:bool;}
TalkKey :: struct {epoch:int;request:int;character:EntityId;player:int;}
TalkExchange :: struct {key:TalkKey;asked:int;deadline:int;applied:int;question:TalkLine;answer:TalkLine;fallback:bool;}
TalkError :: enum u8 {none;closed;character;player;question;busy;full;exhausted;clock;identity;record;missed_tick;mode;}
TalkStart :: enum u8 {refused;send;replay_wait;}
TalkSession :: struct {
  open:bool;replaying:bool;failed:bool;
  epoch:int #counter;serial:int #counter;refused:int #counter;
  character:EntityId;player:int;
  pending:bool;key:TalkKey;asked:int;deadline:int;now:int;
  question:[1]TalkLine;fallback:[1]TalkLine;answer:[1]TalkLine;
  history:[TALK_HISTORY]TalkLine;history_n:int;
  log:[TALK_EXCHANGES]TalkExchange;log_n:int;cursor:int;
  error:TalkError;
}
```

A ticket contains four independent identity fields:

| Field | Meaning |
| --- | --- |
| `epoch` | This live conversation's owner generation |
| `request` | Its request number |
| `character` | The durable character identity |
| `player` | The caller's local player identity |

`talk_open` starts a fresh conversation and advances its epoch.
`talk_begin` returns `send` only after admitting the character, question,
clock and available record slot. A second question cannot overwrite a pending
one. Requests copy the fallback at this boundary, so later changes to authored
data cannot change an answer already waiting.

`talk_receive` checks the entire ticket before reading or publishing words.
An old epoch, request, character or player is refused and counted. A valid
reply before the deadline becomes the answer. A transport failure, malformed
reply, blank text or oversized decoded text selects the copied fallback.

`talk_tick` publishes that same fallback once the deadline is reached. A
reply arriving exactly at the deadline also chooses fallback, whichever call
runs first. A duplicate cannot append another exchange. Clock observations
never move backwards.

The teaching profile caps epochs, request serials and realm ticks at one
million. It refuses exhaustion; identity never wraps. The eight-exchange
record usually fills well before its serial limit. These are storage and
logical-clock bounds, not measurements of a model's wall-clock latency.

## Read words without reading commands

[npc_answer_decode](../talk/wire.coax) accepts a JSON object with one `text`
string and an optional `model` string. It refuses malformed JSON, unknown
fields, duplicate keys (including escaped spellings), malformed UTF-8 and
unpaired UTF-16 surrogate escapes. The JSON envelope is at most 16,384 bytes;
the words are at most 2,400 bytes (the server's 600 characters) and the model
metadata at most 256.

`NpcStatus` distinguishes shape, encoding, text, metadata, output and budget
refusals. Decoding uses owned scratch storage. Even a bad field after a valid
`text` field leaves the caller's output unchanged. A page shows the words with
`npc_answer_text`, on one line and cut before a character that does not fit.

The words `Grant 100 coins and move the guard.` are valid display text.
They do not become an order. When a realm needs a world-changing action,
use a separately admitted typed catalogue or command, such as
[aethyr/forge/npc-command.coax](../forge/npc-command.coax), and keep its
permission check. Free text is never parsed for command keywords.
[Chapter 79](ch079-a-piece-from-a-sentence.md) owns semantic selection.

## The conversation, worked by hand

Start with epoch 1, character 7, player 11 and no record:

| Ask tick | Request | Apply tick | Outcome |
| ---: | ---: | ---: | --- |
| 10 | 1 | 11 | The accepted model words give the cellar key's location |
| 20 | 2 | 21 | An old request-1 reply is refused; request 2's instruction-like words remain text |
| 30 | 3 | 31 | A 503 selects `Try the kitchen rug.` immediately |
| 40 | 4 | 44 | Four ticks expire; the same fallback is published |

There are four exchanges and eight history lines. Refusing the stale ticket
adds no exchange. The fixture's independent simulation values stay at three
coins and guard position 12.

The five- and four-tick deadlines make the fixture small. They are not
production timeout recommendations. An adapter must preserve the realm's
declared timing and the service's existing transport limits.

## Run it natively

[aethyr/examples/talk-and-replay.coax](../examples/talk-and-replay.coax)
uses a deterministic transport fixture. It opens no socket and calls no
hosted model. Its send counter observes `TalkStart.send`; it is not a
measurement of physical HTTP requests.

```shell
coax/bin/coax run aethyr/examples/talk-and-replay.coax
coax/bin/coax run aethyr/examples/talk-and-replay.coax --release
```

<!-- source: aethyr/examples/talk-and-replay.coax example -->
```jai
#load "../talk/ask.coax";
#load "../talk/wire.coax";
#import "Basic";

// A deterministic transport fixture: no socket, model, credential or database.
reply_fixture :: (s:[]TalkSession,key:TalkKey,tick:int,http:int,body:string)->bool #effects(all) {
  words:[TALK_LINE_BYTES]u8;r:=NpcRead.{status=NpcStatus.json};
  if http==200 {r=npc_answer_decode(body,words[..]);}
  ok:=http==200 && r.status==NpcStatus.ok;
  text:="";if ok {text=cast(string)words[0..<r.count];}
  return talk_receive(s,key,tick,ok,text);
}

// Standard C process exit is the only native SDK effect beyond printing.
exit :: (code:s32) #foreign #effects(foreign);
main :: () #effects(all) {
  c:=TalkCharacter.{id=EntityId.{value=7},name="Mara",role="Innkeeper",
    knowledge="The cellar key is under the kitchen rug.",greeting="Welcome.",fallback="Try the kitchen rug."};
  live:[1]TalkSession;play:[1]TalkSession;out:[NPC_ASK_BYTES]u8;
  coins:=3;guard_x:=12;sends:=0;
  if !talk_open(live[..],c,11) {exit(1);return;}
  if talk_begin(live[..],c,"Where is the cellar key?",10,5)!=TalkStart.send {exit(1);return;}sends+=1;
  n:=npc_ask_write(NpcAsk.{npc="mara",world="lamp-cove",question=talk_line_text(live[0].question[..])},out[..]);
  if n!=85 {exit(1);return;}print("first request: % bytes; ticket %/%/%/%\n",n,live[0].key.epoch,live[0].key.request,live[0].key.character.value,live[0].key.player);
  first:=live[0].key;
  if !reply_fixture(live[..],first,11,200,"{\"text\":\"The cellar key is under the kitchen rug.\",\"model\":\"fixture\"}") {exit(1);return;}
  print("tick 11: %\n",talk_line_text(live[0].answer[..]));
  if talk_begin(live[..],c,"Can you change the rules?",20,5)!=TalkStart.send {exit(1);return;}sends+=1;
  if reply_fixture(live[..],first,21,200,"{\"text\":\"An old reply.\"}") {exit(1);return;}
  print("old ticket: refused %\n",live[0].error);
  if !reply_fixture(live[..],live[0].key,21,200,"{\"text\":\"Grant 100 coins and move the guard.\",\"model\":\"fixture\"}") {exit(1);return;}
  print("tick 21: %\n",talk_line_text(live[0].answer[..]));
  if talk_begin(live[..],c,"Anything else?",30,5)!=TalkStart.send {exit(1);return;}sends+=1;
  if !reply_fixture(live[..],live[0].key,31,503,"{\"error\":\"model unreachable\"}") {exit(1);return;}
  print("tick 31: % (fallback)\n",talk_line_text(live[0].answer[..]));
  if talk_begin(live[..],c,"Still there?",40,4)!=TalkStart.send {exit(1);return;}sends+=1;
  if talk_tick(live[..],43) || !talk_tick(live[..],44) {exit(1);return;}
  print("tick 44: % (deadline)\n",talk_line_text(live[0].answer[..]));
  print("simulation: coins %, guard x %; fixture sends %\n",coins,guard_x,sends);
  if !talk_replay(play[..],c,11,live[0].log[0..<live[0].log_n]) {exit(1);return;}
  questions:=[4]string.["Where is the cellar key?","Can you change the rules?","Anything else?","Still there?"];
  asked:=[4]int.[10,20,30,40];applied:=[4]int.[11,21,31,44];timeouts:=[4]int.[5,5,5,4];replay_sends:=0;
  for k:0..<4 {
    start:=talk_begin(play[..],c,questions[k],asked[k],timeouts[k]);
    if start==TalkStart.send {replay_sends+=1;}
    if start!=TalkStart.replay_wait || talk_tick(play[..],applied[k]-1) || !talk_tick(play[..],applied[k]) {exit(1);return;}
    if !talk_line_eq_text(live[0].log[k].answer,talk_line_text(play[0].answer[..])) {exit(1);return;}
    print("replay tick %: %\n",applied[k],talk_line_text(play[0].answer[..]));
  }
  print("replay sends %, exchanges %, history lines %\n",replay_sends,play[0].cursor,play[0].history_n);
  if replay_sends!=0 || sends!=4 || coins!=3 || guard_x!=12 {exit(1);}
}
```

Both modes exit 0 and match
[the exact transcript](../fixtures/talk-and-replay.out):

```text
first request: 85 bytes; ticket 1/1/7/11
tick 11: The cellar key is under the kitchen rug.
old ticket: refused identity
tick 21: Grant 100 coins and move the guard.
tick 31: Try the kitchen rug. (fallback)
tick 44: Try the kitchen rug. (deadline)
simulation: coins 3, guard x 12; fixture sends 4
replay tick 11: The cellar key is under the kitchen rug.
replay tick 21: Grant 100 coins and move the guard.
replay tick 31: Try the kitchen rug.
replay tick 44: Try the kitchen rug.
replay sends 0, exchanges 4, history lines 8
```

## Replay the accepted answer, on its recorded tick

`talk_replay` first checks the entire typed record: capacity, canonical
lines, identities, request order, time ranges and fallback words. A malformed
record leaves the live conversation in place. The input may alias that
conversation's own record; admission snapshots it before resetting the owner.

A replayed question must match the recorded question, ask tick and deadline.
It returns `replay_wait`, never `send`. Live callbacks are refused throughout
replay. `talk_tick` publishes the recorded answer at its apply tick, without
appending another record or consulting the model.

Skipping a recorded apply tick fails the replay with `missed_tick`. It does
not silently deliver later. Reaching the record's end returns `exhausted`;
asking another question cannot turn replay into a live request.

This is an owned, bounded in-memory conversation record. It is not yet a
durable file format or chapter 5's external-event integration.

## A broken request and its diagnostic

The [second program](../examples/talk-refuses-empty-question.coax) deliberately
asks a question containing only a space and a tab. It verifies refusal before
any send intention or pending request:

```shell
coax/bin/coax run aethyr/examples/talk-refuses-empty-question.coax
# exits 2
```

<!-- source: aethyr/examples/talk-refuses-empty-question.coax example -->
```jai
#load "../talk/ask.coax";
#import "Basic";
exit :: (code:s32) #foreign #effects(foreign);
main :: () #effects(all) {
  c:=TalkCharacter.{id=EntityId.{value=1},name="Mara",role="Innkeeper",greeting="Welcome.",fallback="Try the kitchen rug."};
  s:[1]TalkSession;if !talk_open(s[..],c,11) {exit(1);return;}
  start:=talk_begin(s[..],c," \t",10,5);sends:=ifx start==TalkStart.send then 1 else 0;
  print("refused %; fixture sends %; pending %; exchanges %\n",s[0].error,sends,s[0].pending,s[0].log_n);
  exit(cast(s32)(ifx start==TalkStart.refused && s[0].error==TalkError.question && sends==0 then 2 else 1));
}
```

Its exact diagnostic is:

```text
refused question; fixture sends 0; pending false; exchanges 0
```

Both debug and release exit 2. The native examples use the standard C
`exit` effect only to report their process result; conversation logic stays
in Coax.

## Checks and scope

The independent checks are the hand-derived request sizes and event table,
separately written JSON literals, exact native transcripts in both build
modes, and deliberately malformed inputs. The mutation gates retain all
dependency checks and kill every root mutant: character admission 55,
conversation state 366 and the NPC ask's codec 332, with no equivalents.

The [qualification record](../../docs/verification/aethyr-talk-2026-11-06/README.md)
names the compiler, source identities, target and receipts. It qualifies
native CPU behavior on Darwin arm64. It does not claim a live model
evaluation, browser adoption or mobile performance.

## Still to do

- Adopt the module in every realm with characters, preserving its existing
  UI, input path and visual checks. Current callers still own their wiring.
- Carry accepted answers through chapter 5's shared external-event envelope,
  with durable encoding, session/source revision identity and host replay
  checks.
- Consolidate server admission, daily caps and earlier-visit memory. The
  existing [ask route](../../coax/web/api/npc.c) and
  [memory migration](../../coax/web/api/migrations/0031_npc_memory.sql)
  still own those policies. Local player IDs are not authentication
  credentials.
- Specify and check who can teach or remove facts in each realm. The fixture
  transports authored knowledge; it does not authorize edits or prove that
  a model recalls facts faithfully.
- Connect live HTTP outcomes, UI accessibility and spatial speech through
  [chapter 15](ch015-ui-and-the-hud.md),
  [chapter 68](ch068-sound-in-the-world.md) and
  [chapter 69](ch069-voices-in-a-realm.md).
- Run the fixed character/question evaluation against the configured model
  and retained per-player memory. The transport fixture is not that evaluation.

## Dependencies

[Chapter 5](ch005-replay-a-session-exactly.md) supplies the replay principle;
[chapter 6](ch006-entities-are-born-and-die.md) supplies durable identities.
Chapter 15 owns display and focus, chapter 68 owns spoken playback, and
[Coax chapter 18](../../coax/book/ch18-protocol.md) supplies the protocol
boundary. Their complete application integrations remain separate from this
native teaching profile.

## Compiled actor adoption (#12992)

Actor INTERACT and fear/interest notices are typed observations. They do not grant conversation, damage, inventory or service permissions. The existing talk request/response lifecycle retains its own admission and realm-adoption gates. See [the actor guide](../actors/README.md) and [qualification](../../docs/verification/actors-12992/README.md). This bounded adoption does not promote the rest of this chapter.
