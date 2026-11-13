# 49. Rooms and presence

> **Status: partly implemented.** Rooms, signed seat admission, replication,
> interest and interpolation run in `coax/web/room/`. This overview directs
> you to the checked implementations in chapters 86–101. Presence and
> lockstep remain separate trust profiles.

A room groups players under one simulation owner. Everglade clients send
commands over a socket; the server steps the world and sends accepted state.
An HTTP presence poll is not Everglade's simulation path.

## Follow a player into a room

1. [Chapter 86](ch086-one-server-owns-the-scene.md) starts the headless
   server. [Chapter 88](ch088-a-seat-joins-with-a-signed-ticket.md) checks
   the API's signed ticket before assigning a seat.
2. [Chapter 89](ch089-intents-in-acknowledgements-out.md) queues numbered
   inputs and acknowledges the last accepted input.
3. [Chapters 91](ch091-one-record-for-every-replicated-entity.md) and
   [92](ch092-priority-under-a-budget.md) encode state within each seat's
   byte budget. [Chapter 97](ch097-interest-without-leaks.md) withholds
   blocked and hidden players across channels.
4. [Chapter 93](ch093-one-interpolation-buffer.md) draws remote entities
   from one interpolation buffer. [Chapter 98](ch098-versions-deploys-and-reconnect.md)
   keeps the seat through a short disconnect and rejects replaced sockets.

## Shared contracts and their limits

[`metaverse-engine/net/profile.coax`](../../metaverse-engine/net/profile.coax) distinguishes
server authority, self-asserted presence and lockstep. The room's `rm_profile`
selects timing, trust, durability and fairness; a presence profile cannot
qualify progression as authoritative.

[`metaverse-engine/session/party.coax`](../../metaverse-engine/session/party.coax) supplies
membership, invite, roster visibility and placement rules. The API persists
the party; the room applies its health-only visibility rules. A party is
not a seat and outlives a room. Travel between owners uses
[`travel.coax`](../../metaverse-engine/session/travel.coax), rather than making a
second room authoritative by opening another socket.

Run the contracts' checks from the repository root:

```shell
coax/bin/coax test metaverse-engine/net/profile.coax --library
coax/bin/coax test metaverse-engine/session/party.coax --library
```

The authoritative engine is partly extracted. Commands, accepted results and
replication still have Everglade-specific implementations in the room and
page. Chapter 101 records that integration; these profiles do not claim that
every realm uses the same networking implementation.
