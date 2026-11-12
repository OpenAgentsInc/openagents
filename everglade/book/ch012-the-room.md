# 12. The room

> **Status: partly implemented.** Everglade II runs as rooms on the room
> server (#13053 to #13058). The page's side is
> [`everglade/client/net.coax`](../client/net.coax),
> on the shared client half in
> [`everglade/client/net-core.coax`](../client/net-core.coax).
> The server's side is
> [`coax/web/room/room-everglade.coax`](../../coax/web/room/room-everglade.coax)
> and `world2_*` in `coax/web/room/room-world.c`. Both step you with
> [`everglade/shared/move.coax`](../shared/move.coax).
> The measurements below are from headless Chrome against a local room.
> Real Chrome on two machines through the staging path is not checked yet.
> This chapter names functions and has no source listings yet.

This chapter answers the player's question: "Is that another person?"

When you leave Character Create, other heroes run down the path beside you.
Each one looks the way its player made it, and each of you sees the others in
the same place. One program decides where everyone is. That program is the
room.

## What this chapter teaches

- Why one server owns the world, and what the page still decides.
- How the page and the room step you on the same code, so the room agrees
  with what you see.
- How the page joins a room: a signed seat ticket, a hello and your
  Character Create record.
- How the page draws the other players a short time behind, so they move
  smoothly.
- How the room sends you only the players near you.

## What exists today

| Part | How it works | Where |
| --- | --- | --- |
| The room's world | Everglade II's land, path, woods, colliders and pace. The room picks it by the room code `everglade2-N`. | `eg_e2_code` in `room-everglade.coax`; `world2_prepare`, `world2_place`, `world2_step` in `room-world.c` |
| The shared step | Where you may stand, the trunk and rock colliders, and the pace. The page and the room both load it. | `everglade/shared/move.coax` |
| Joining | The page asks the API for a seat ticket for the realm `everglade2`. The API places you in an instance on a shard. The socket opens with the ticket. | `everglade/client/net.coax`; [Aethyr chapter 88](../../aethyr/book/ch088-a-seat-joins-with-a-signed-ticket.md) |
| The hello | The page sends its build and a hello with your Character Create record and your name. | `everglade/client/net.coax` |
| Your own movement | The page steps you at the room's rate, 30 steps a second, and sends each step's input. When the room's answer differs, the page re-runs your inputs from the room's position. | `everglade/client/net.coax`, `everglade2-face.c` |
| The other players | The room's rows go into a ring. The page draws each player a jitter buffer behind. | `everglade/client/net-core.coax` |
| Their bodies | One draw path draws you and the others: each player's own character, gait, jumps and facing. | `e2_remotes_draw` and `e2_body` in `everglade/client/client.coax` |
| Casts | The room checks the slot, the aim, a 0.8 s cooldown and the range. Then it sends the cast to every page within 120 m. | `everglade/client/cast.coax`, `room-everglade.coax` |
| Interest | The room sends you the players within 110 m and drops them past 130 m. The page fades a body in and out over 0.6 s. | `room-interest.coax`, `everglade/client/client.coax` |

A room seats 16 players (`EG_SEATS` in `room-everglade.coax`, `WIRE_SEATS` on
the page). When the room you ask for is full, the API places you in another
room, and the page says so over the bar: `ROOM 3 HAS 16 PLAYERS · JOINING
ROOM 5`. When every room is full, the page says
`EVERY ROOM HAS 16 PLAYERS · TRYING AGAIN` and asks again.

## The method

### The room decides, and the page predicts

Anything a page sends, a player can change. So the room owns every fact that
matters to another player: where you stand, whether your cast happened and
who it hit. [Aethyr chapter 86](../../aethyr/book/ch086-one-server-owns-the-scene.md)
builds that server.

If the page waited for the room before it moved you, every key press would
lag by a round trip. So the page moves you at once and sends the input. This
is safe because the page and the room run the same step on the same
colliders ([Aethyr chapter 87](../../aethyr/book/ch087-server-and-client-share-one-step.md)).
Most of the time, the room reaches the answer the page already drew.

When they disagree, the room wins. The page takes the room's position, runs
your later inputs again and draws the result
([Aethyr chapter 90](../../aethyr/book/ch090-predict-your-own-movement.md)).

### Join before the run ends

The join takes a few network round trips. Everglade hides them inside the
run from the glade:

1. You press USE IN EVERGLADE. The run down the path starts.
2. The page asks for a seat ticket and opens the socket.
3. The room seats you. The page sends its hello.
4. The room's rows arrive, and the other players appear.

The run holds your feet for at most 4 seconds (`E2N_HOLD_MS`) while it waits
for a seat. If no seat comes, you walk on alone, and the page keeps trying.

### Draw the others a little late

The room's rows arrive unevenly. If the page drew each row as it arrived, the
other players would stutter. The page keeps a short buffer and draws each
player where they were a moment ago, between two rows it already has
([Aethyr chapter 93](../../aethyr/book/ch093-one-interpolation-buffer.md)).
A cast that the room sends carries the room's time. The page plays it when
the drawn picture reaches that time, from where that player is drawn.

### Send only what is near

A page can't draw a player 400 m away through the woods, so the room doesn't
send them. A player comes into your view within 110 m and leaves it past
130 m. The gap between the two distances stops a player at the edge from
flickering in and out ([Aethyr chapter 97](../../aethyr/book/ch097-interest-without-leaks.md)).

## A worked example: three players leave the glade

Three headless pages take the whole journey in one room: Character Create,
USE IN EVERGLADE, the run and the clearing. The check makes each page a
different character.

| Measure | Result |
| --- | --- |
| Time from USE IN EVERGLADE to a seat | 0.5 to 1.6 s, while the run plays |
| Each page draws the other two | During the run and after the walk, with their names |
| Speed of the others as drawn | 4.0 to 4.1 m/s during the run |
| Stalls while moving | 0 in about 1,150 moving frames a page |
| Largest correction when the page re-runs your inputs | 9 to 43 mm |

The correction is the number to watch. The page and the room agree to within
5 cm while you walk and run. A larger number means the two steps have
drifted apart.

With 13 other players in the room, a page held 60 frames a second: the
99th-percentile frame gap was 18.6 ms and no frame took longer than 25 ms.

## The checks

- **The crowd check.** `coax/web/tool.sh everglade2-crowd URL --players 3`
  runs the journey above and reports seat time, smoothness, corrections,
  fades and pops for each page
  ([`coax/web/everglade2-crowd.coax`](../../coax/web/everglade2-crowd.coax)).
- **The soak.** `load.coax everglade2` seats bots in a room. Twelve bots ran
  for 11 minutes with two pages joining partway: no state frame was dropped,
  the 99th-percentile gap between states was 35 ms, and the room's memory
  stayed flat.
- **A bad network.** [`net-proxy.coax`](../../coax/web/room/net-proxy.coax)
  sits between the pages and the room and adds 100 ms of delay, 20 ms of
  jitter and 1% loss. The journey still holds, and corrections stay under
  5 cm.
- **Interest.** Fourteen bots spread along the path for 200 seconds. The
  room withheld every far pair and never missed a near pair.

These checks ran headless against a local room. They don't establish what a
player sees in a real browser through the production path.

## Mistakes

### Players who popped in

When the room first sent only nearby players, a player who crossed the
110 m line appeared at once, fully drawn, in the middle of the path. A
player who left vanished the same way.

Commit `9379d66b5f` fades each body in over 0.6 s and fades it out where it
was last drawn. The crowd check now counts fades and pops. A page that
walked up the path among 14 bots saw 14 fade in and 0 pop.

### Casts the room refused

Everglade II's room first refused every cast, because only the v1 town's
room knew the class bars. During the first soak, bots sent 1,520 casts and
the room refused them all. The bar looked as if it worked, and nothing
reached the other players.

Commit `4fab48af86` sends casts through the room. The class kits are now one
shared module that the page and the room both load, so the room can check
that a slot is on your bar.

## What comes next

A room forgets you when you close the tab unless it saves you first.
[Chapter 13](ch013-coming-back.md) covers what the server keeps, what the
browser keeps and how you return to where you stood.
