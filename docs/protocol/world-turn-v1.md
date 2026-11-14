# What a turn knows of a world v1 (`world.turn.v1`, `world.plan.v1`, `world.card.v1`)

The editor that `?edit` opens over a world has the `/chat` conversation in its foot (GitHub
#13418). Three shapes carry the world between the editor, the conversation and the server.

- Codec: `coax/lib/agent/world/turn.coax`.
- Pinned bytes: `proto/world/turn-v1.json` (the turn and the plan) and `proto/world/card-v1.json`.
- Check: `coax/web/tool.sh coax/lib/agent/world/turn-profile.coax proto/world/turn-v1.json`, and
  the same with `card-v1.json`.

Every number is whole. Places are centimetres and headings are degrees, -179 to 180. A reader
refuses any bytes but the ones its writer writes.

## The turn: `world.turn.v1`

The editor shell writes it from the page's model whenever the world, the selection or the point
last tapped changes, once the foot was opened. The browser host carries it to the chat module,
which sends it with every turn after. It never leaves the page as these bytes.

| Line | What it says |
| --- | --- |
| `world.turn.v1 WORLD LANE REV SELECTED` | The world's ID, its lane (`dev` or `live`), the lane's revision, and the selected thing's ID (0 for none). |
| `here X Z` | The point last tapped. Left out until one is tapped. |
| `k KIND NAME` | A kind a thing can be: its content ID and its name. At most 64. |
| `a ID KIND BYTES NAME` | An attachment of the world or its project (#13468): its ID, its kind (`image`, `document` or `link`), its size and its name. At most 24. Never its bytes. |
| `an NOTE` | The attachment above's note. Left out for none. |
| `at TAGS` | Its tags, joined by commas. Left out for none. |
| `l TABLE TITLE` | A list of switches of a realm (#13528): its table and its title. At most 4. |
| `w ID ON NAME` | A row of the list above: its ID, `1` on or `0` off, and its name. At most 96 in all. |
| `asks` | The world is a realm that keeps a list of asks: what its owner asked for. Left out for a world that keeps none. |
| `q ID STATE WORDS` | An ask: its ID, its state (`applied`, `waiting` or `removing`) and its words. At most 24. |
| `e ID KIND X Z YAW` | A thing: its ID, its kind, where it stands and its heading. At most 128. |
| `more N` | Things the world holds that are not listed. Left out for none. |
| `card ID X Z THING` | A visitor's proposal waiting for the owner of a realm's world (#13527): its ID on the realm's server, where the thing would stand, and what it is. At most 16. |
| `from WHO` | Who proposed the card on the line above. |
| `cards N` | How many proposals wait in all, when fewer are listed. Left out when all are. |

The conversation shows each card as a turn of its own: who proposed what and where, with Accept and
Decline (`apps/openagents-web/chat/chat-ideas.coax`). Accept posts
`/api/playtest/realms/WORLD/proposals/ID/accept` as the owner, and the server places the thing as
the owner's own world edit under its rules. Decline posts `.../decline`. Neither goes through a
model, and the cards are not in the turn in words.

How a turn carries it depends on who answers:

- **An agent that works by itself** (Claude Code, Codex, Antigravity or Microcoder on your
  computer, or the hosted agent on a Claude account): the turn in words follows your message
  (`wt_words`): the world, the selected thing and the point last tapped. The transcript shows
  what you typed. The agents on your computer change the world with the `realm_world` tools,
  and their edits show in the open editor as any outside edit does.
- **A model that only answers** (the Gemini models): the turn is a hosted world turn, below.

The turn in words lists the attachments too, with the command that reads one
(`openagents realm file-read WORLD ID`), so an agent on your computer reads them itself.

A realm's turn in words lists its lists of switches, each row on or off, and its asks with their
states. It names the tools that change them, the same realm tools an agent on your computer
already has: `realm_world_switch` for a row, `realm_ask`, `realm_revise`, `realm_ask_delete` and
`realm_row_delete` for the asks and the board, and `realm_asks` to read them again. So the asks
reach an agent on your computer and a hosted turn the same way: as the turn's own lines, with a
tool for each change.

## The hosted world tools and the plan: `world.plan.v1`

A hosted world turn is one model call under a schema (`wp_schema`). The model answers a route,
what to say and steps:

- `talk`: nothing changes. What it says is the answer.
- `edit`: the steps are the changes, in order.
- `open`: the model wants to see attachments before it answers. `open` lists the IDs of at most
  3 of them, from the turn's `a` lines; the steps and what to say are empty.

When the model takes `open`, the conversation shows a line "Open NAME" for each and asks again:
the same messages under the same schema, with words that say what was opened, and the request
carries `"attachments": {"world": WORLD, "ids": [...]}`. The server (`chat.c`, then `worlds.c`
`world_files_turn`) checks that whoever asks may see the world's attachments, removes that key
and joins each attachment to the last message: a line that names it, then a text file's text,
or an image or a PDF as the file itself, which the model reads. A link is its address; its page
is not opened. A turn reads 8 MB of attachments at most. A file over what is left is not read,
and the line in its place says how large it is and that it was not read, so the answer can say
so. The second answer must be `talk` or `edit`; a second `open` is refused in words. Nothing
about a file is logged.

On the dev server the local API reads the dev server's own store (`serve.sh` names it in
`OA_WORLD_FILES_DIR`); deployed, it reads the private bucket.

Each step names one of the hosted world tools. They are the rows of `WT_TOOL`, and the schema
and the instructions are written from those rows.

| Tool | What it does |
| --- | --- |
| `place` | A new thing of a kind at a place, with a heading. |
| `move` | A thing to a new place. |
| `turn` | A thing to a heading. |
| `size` | A thing to a size in percent. |
| `remove` | A thing taken out of the world. |
| `undo`, `redo` | The last change taken back, or made again. It must be the only step. |
| `switch` | A row of a list switched on or off: the list's place among the turn's lists, the row's ID and `on`. |
| `show` | One of the editor's own panes opened for you: `files`, `scripts`, `code`, `revisions` or `assets`. |
| `ask` | Words put on the realm's list of asks: one feature or change, as its owner would ask. |
| `revise` | The realm's board revised from words, with nothing kept on the list. |
| `unask` | An ask deleted, by its ID. What it changed goes back. |
| `unrow` | A row of the realm's board deleted (only the list of table rows), named as for `switch`. |
| `look` | A question about what is on the screen now, answered from a picture of it. It must be the only step. |

`switch`, `show` and `unrow` need a turn with lists; `ask`, `revise`, `unask`, `unrow` and `look`
need a turn whose world keeps asks. The instructions offer a tool only where the turn can use it,
and a plan that names a tool the turn cannot use, a row that is not in its lists or an ask that is
not on its list is refused in words.

**Show** means the editor's own panes, not a part of the world. The world's things are already in
the turn, and the selection and the point last tapped say what "this" and "here" are, so a turn
that wants you to see something in the world places, moves or looks at it instead.

**A design document**: to work from an attachment (a design document for the realm), the model
opens it first (route `open`, above), then answers with an `ask` step for each feature or change
it asks for.

Which route a turn takes and which tools it uses is the model's typed answer. No code matches
words in the message. An answer that is not under the schema, or that names a kind or a thing
the turn does not list, is refused in words and nothing changes.

The chat module writes the steps as a plan and the browser host carries it to the editor shell:

| Line | What it says |
| --- | --- |
| `world.plan.v1 WORLD REV` | The world and the revision the plan was made from. |
| `place KIND X Z YAW` | A new thing. |
| `move ID X Z`, `turn ID YAW`, `size ID PERCENT`, `remove ID` | A change to a thing. |
| `undo`, `redo` | A plan of only that. |
| `switch LIST ID ON` | A row of the turn's list `LIST` (0 for the first) to `1` or `0`. |
| `show PANE` | A pane opened: `files`, `scripts`, `code`, `revisions` or `assets`. |
| `ask WORDS`, `revise WORDS`, `look WORDS` | Words on one line, at most 200 bytes. |
| `unask ID`, `unrow LIST ID` | An ask, or a row of the board, deleted. |

The shell makes the plan's world steps and its switches one `world.edit.v1` batch from that
revision, and the page sends it as its own. A switch is a `cell` edit of the list's table: the
row's `on` column to `1` or `0`, the same edit the switch on the rows table makes (#13514). A
plan of only realm steps needs no matching revision. The shell opens each pane a `show` names, and
says so when the world has no such pane.

The realm steps go to the page as the editor's message 530, a step a line: `ask WORDS`,
`revise WORDS`, `unask ID`, `unrow TABLE ROW` (the list's table, not its place) and `look WORDS`.
The page's edit layer (`metaverse/edit/edit.coax`) sends each, in order, as its owner's own request:
`POST /api/playtest/realms/SLUG/asks`, `/revise`, `/asks/ID/delete`, `/rows/KEY/delete` and
`POST /api/playtest/realm/SLUG/look` with the frame drawn. The server's rules answer them as they
answer a person: only the realm's owner and admins change it, one request at a time. What came
of each goes back to the conversation in words (the page's action 34), as the agent's answer under
the turn: added and applied, added and waiting for code, revised, deleted, or the look's answer. So the server's rules answer a hosted turn as they answer a person: a stale revision,
a kept-clear zone or a write by someone who is not an admin is refused in words, and one undo
takes the whole turn back. A plan made from an older revision than the editor's is not sent;
the shell says to ask again.

A plan holds at most 16 steps. The conversation shows each step as a line above the answer.

## The card: `world.card.v1`

`GET /api/worlds/ID/card` answers the world as a conversation shows it. Anyone may read it.

| Line | What it says |
| --- | --- |
| `world.card.v1 WORLD REV THINGS` | The world's ID, the dev lane's revision and how many things it holds. |
| `name NAME` | The world's name. |
| `frame URL` | A picture of it, a path on this site. Left out for none. |

On `/chat`, when an agent on your computer uses a `realm_world` tool, the page reads the card
of the world the tool names and shows it after the run of tool calls: the name, how many
things and the revision, and **Open the editor**, which goes to `/worlds/ID?edit=agent`: the
world with its editor open and the same conversation in its foot. The world is named by the
tool's own typed name and its `slug` argument in the session's log, not by words in a message.

## The composer's voice

In a realm's editor the conversation's composer has **Talk** where the page has a voice. Talk
holds the page's voice (`coax/web/voice.js`, push to talk) for the conversation. The browser host
names whom the voice host faces, so it leaves the turns to the page, and while Talk holds the
voice its words go to the conversation, not to the realm. A turn's final words are your message,
sent as if typed, and the answer is spoken. Press Talk again to let go. Which tools a spoken
message uses is the model's typed answer under the schema, as for a typed one: a spoken "switch
the water off" is a `switch` step, a cell edit of the rows table.

## Not built

- The hosted agent on a Claude account is told the world in words but has no world tools of its
  own: its runner has no realm commands. It can say what to do; a Gemini model or an agent on
  your computer does it.
- A card has no picture yet. No route writes `frame`.
- The card appears for agents on your computer. A hosted turn on `/chat`, outside the editor,
  has no world to plan in and shows no card.
