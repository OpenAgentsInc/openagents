# 11. One address

> **Status: partly implemented.** everglade.world is live. Its pages are
> written by [`coax/web/realms.ts`](../../coax/web/realms.ts) from the
> `domain` block of the `everglade2` entry in
> [`apps/openagents-web/realms.json`](../../apps/openagents-web/realms.json),
> into `apps/openagents-web/realms/everglade2/domain/`. The page side is in
> `everglade2-create.coax` and `everglade2.coax`. The load balancer's routes
> are set by hand and written down in
> [`docs/verse/everglade-domain.md`](../../docs/verse/everglade-domain.md).
> Not built yet: the release smoke and golden for the domain, and
> `lb.sh everglade-domain`.

This chapter answers the player's question: "Where do I go to play?"

The answer is one address: [everglade.world](https://everglade.world). Its
front door is a standard MMORPG landing page with PLAY NOW. Playing happens at
`everglade.world/play`, and the address bar says `/play` all the way in. A new
player goes to Character Create. A returning player goes back to where they
stood.

## What this chapter teaches

- How one address decides between Character Create and the world.
- What the browser keeps, what the room keeps and why.
- How the load balancer maps the domain's paths onto one bucket.
- How a missing route broke the spell bar, and what checks it now.

## What exists today

- **The front door, `/`.** A hero capture, the EVERGLADE wordmark, PLAY NOW,
  four features, three screenshots and a footer. As soon as it has painted, it
  opens `/play?via=portal` out of sight under itself. PLAY NOW then fades into
  the game in about half a second, with no page load.
- **`/play`.** Everglade II with Character Create's arrival and font
  (`"create": "character-creator"` in the domain block). `e2c_play_page`
  recognizes it.
  - With no character kept in this browser (`aethyr.player.appearance.v1`),
    it opens Character Create in the glade.
  - With one, it opens the world with that look, class and level, where you
    stood last.
- **Where you stood.** The page keeps your place in the browser as
  `coaxial.everglade2.place` (host action 11) and tells the room in its hello
  (#13057). The room's character checkpoint brings back your class, level and
  health.
- **Old addresses.** On this host, `/worlds/character-creator` and
  `/worlds/everglade2` return a 301 to `/play`, with the query dropped. Any
  other unknown path shows Everglade's own 404.

## The method

### Decide on the page, not on the server

The front door and `/play` are static pages in one bucket. The server doesn't
know who you are when it serves `/play`. The page decides, from what this
browser kept:

1. Read the kept character. If there's none, start Character Create.
2. If there's one, read the kept place and start there. Never start in the
   studio, deep in the woods or from a broken record.
3. Tell the room where you start, so other players see you there.

### Keep each fact where it belongs

| Fact | Kept by | Why |
| --- | --- | --- |
| Your look | The browser | It is yours, and the page needs it before the room answers |
| Where you stood | The browser, then the room | The page places you at once; the room shares it with others |
| Class, level and health | The room's checkpoint | Rules state is the server's, so a page can't change it |

### Map every path the page loads

On everglade.world, the load balancer rewrites paths onto the shared bucket.
`/` goes to the front door, `/play` to the play page, the room and API paths to
their services, and the realm's asset paths under `/worlds/...` to `/realms/...`.
Anything else goes to Everglade's 404.

So every path the page loads needs its own rule. That includes paths that
belong to other realms, when the page borrows their files.

## A worked example: one visit, two times

**First visit.** You open `everglade.world`. The front door paints, then loads
`/play?via=portal` under itself. You press PLAY NOW. The page finds no kept
character, so the portal's light opens onto Character Create. You make a
Ranger, choose level 3 and press USE IN EVERGLADE. The record is kept, you run
down the path ([chapter 7](ch007-character-create-in-the-world.md)), and you
walk to 150 m down the path. The page keeps that place.

**Second visit.** You open `everglade.world/play`. The page finds your Ranger
and your place. It starts you 150 m down the path, tells the room, and the
room's checkpoint restores your class, level and health. The address bar says
`/play` the whole time.

## The checks

In `apps/openagents-web/everglade2-create.coax`:

- `#test "/play is Character Create without a kept character, the world with
  one"`. It also asserts that `/player` and openagents.com's
  `/worlds/everglade2` are not the play page.

In `apps/openagents-web/everglade2.coax`:

- `#test "/play keeps where you stand, once you have moved"`.
- `#test "/play starts you where you stood, told to the room; never in the
  studio, the woods or a broken text"`.

In `apps/openagents-web/experimental-combat.coax`:

- `#test "experimental combat: every class's level 1 bar holds its Character
  Create ability first, each slot with its picture"`. It asserts that every
  slot has a picture, and that every slot after the first loads its picture
  from `XC_MMO_URL`.

## Mistakes

### The spell bar with no pictures

The class bar's spell pictures are shared with the 5E MMO realm. They load from
`/worlds/5e-mmo/hotbar/` (`XC_MMO_URL` in
[`apps/openagents-web/class-kit.coax`](../../apps/openagents-web/class-kit.coax)).
On openagents.com that path works. On everglade.world, the load balancer's
matcher had no rewrite for `/worlds/5e-mmo/`, so each picture's request fell
through to the catch-all rule and got Everglade's 404 page. The bar showed its
slots with no pictures.

The fix adds the `/worlds/5e-mmo/` rewrite to the matcher (priority 155), and
`docs/verse/everglade-domain.md` now lists every live route. The doc states
the rule: a new `/worlds/<x>/` path that `/play` loads needs its own rewrite.

What checks it today, and what doesn't yet:

- The combat test above pins the URL that each slot uses, so a change to the
  pictures' path shows up in a test.
- Nothing yet loads `everglade.world/play` and checks that every asset it asks
  for comes back. That is the domain's release smoke, which isn't built. Until
  it is, read the route table before you add a path.

## What comes next

Part IV adds other players. [Chapter 12](ch012-the-room.md) builds the room:
one simulation that every player sees the same way.
