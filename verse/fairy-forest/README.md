# Fairy Forest

Status: first crack live and unlisted, October 29, 2026, at
https://openagents.com/realms/fairy-forest (task ff-first-crack): the valley,
the fairy with flight and a glide, Bloom, three creatures and three of the
heart tree's roots. Since then (ff-roots): Thornfall (brambles round a fallen
giant tree, with Burdock the badger) and the Old Ring (nine standing stones,
with Hollow the owl), so all five roots can be woken; then the heart tree
blooms, colour comes back to the whole valley, and the forest stays open to
wander. What you restored is saved in this browser and, with an account, on
the server. Since then too (ff-talk): the creatures talk, typed, through the character chat Everglade's people use: each knows its trouble and how far you have helped, says things as you pass, thanks you once its glade is green and remembers you (verse/fairy-forest/talk.coax). No home or crafting yet. The
designer's notes on panes, chat and messages were added the same day. It is drawn
from what players told us when asked what game they would make: a magical
fairy forest came up again and again, unprompted, along with caring for
things until they change, helping people, building a home and a community,
making your own character, and making things to share with friends. This
page puts as much of that as fits sensibly into one realm.

Style: **painterly** ([art-styles.md](../../docs/verse/art-styles.md)). It is
the realm that style is for.

## The story in one line

You are a newly woken fairy in a forest that is fading. You learn what you
are good at, use it to help the forest's people with their troubles, grow a
home and a community, and bring the heart of the forest back. When the
heart blooms the story ends, and the forest stays yours to live in.

The story has a real ending. The forest is open to wander before and after
it.

## The forest

- **Size:** a valley of about 300 × 300 m, walkable end to end in a couple of
  minutes. Five glades round a central heart tree, joined by trails, with a
  stream running through and a pond under a waterfall (the engine's water:
  [water.md](../../docs/verse/water.md)).
- **The glades:** each has its own character and its own trouble.
  - **Mossbank:** soft hills and mushrooms; where you wake and where your home
    grows.
  - **Lanternwood:** tall trees whose fruit glows at night.
  - **The Reeds:** the stream's marsh, frogs and dragonflies.
  - **Thornfall:** brambles and a fallen giant tree, the forest's wildest edge.
  - **The Old Ring:** standing stones and the forest's lore.
- **The fade:** at the start, much of the forest is grey and quiet: dull
  colours, bare branches, few animals. Wherever you bring it back, colour,
  sound, flowers and wildlife return. The fade is drawn, not described: the
  painterly palette drained to grey, with blooms of colour spreading out from
  what you have restored.
- **Day and night:** a real cycle. Some things only happen at night (the
  Lanternwood fruit, moths, the stars over the Old Ring).
  Built (ff-night, `verse/fairy-forest/night.coax`,
  `apps/openagents-web/fairynight.coax`): the realm's clock is `real`, the
  visitor's own time of day; dawn and dusk are warm and low, and the night
  is moonlit blue with stars. Lanternwood's fruit glows at night (bright once
  its glade is green, an ember before) and moths gather at the lights. The
  lantern flower opens only at night; by day E there rests you till night
  (and in the night till morning). With LIGHT, in the dark, a path of stones
  shows from the flower to the moon hollow, a ring of glowcaps round a moon
  pool; find it and two lantern moths keep to your light. Goldens are noon.

## You: a fairy you make

- **Make your fairy** before you wake: body shape, skin, hair, face, outfit,
  and wings (shape, colour, pattern, glow). As much choice as the engine's
  figures allow; more pieces are added over time.
- **Change it any time** at the mirror pool in your home.
- Built (ff-maker, `apps/openagents-web/fairymaker.coax`,
  `verse/fairy-forest/life.coax`): the first visit opens YOUR FAIRY, a pane
  of rows (body, skin, hair, style, eyes, outfit, wings, wing colour, glow)
  with choices to tap while she turns to face you; DONE wakes you. The
  mirror pool in Mossbank, west of where you wake (E or MIRROR), opens it
  again. The look is kept in the save's tail, so a reload or another device
  with an account keeps it.
- **Grows with you:** wings and outfit gain detail as you learn skills, so
  how you look says how far you have come. Built so far: a gem on her upper
  wings for each skill (ff-skills).
- **Moving:** walk, run, and fly in short hops that get longer as your wings
  grow; glide down from heights. The same controller as every realm
  (`verse/player.coax`), with flight added.

## What you learn: skills

You discover your skills one at a time, each from a moment in the story, and
each opens new ways to help:

| Skill | You learn it by | It lets you |
| --- | --- | --- |
| **Bloom** | Waking a single flower | Make plants grow and flower; restore faded ground |
| **Speak** | Being answered by a creature for the first time | Talk with animals and hear what they need |
| **Mend** | Helping a hurt bird | Heal creatures and repair broken things |
| **Gather** | Your first forage | Find and carry more; know what each find is for |
| **Craft** | Being given an axe by a woodcutter | Cut deadwood, build and make tools |
| **Light** | A night in Lanternwood | Carry light; open places that only show in the dark |
| **Water** | Clearing the stream | Move water, fill the pond, water a whole glade |

Built (ff-skills, `apps/openagents-web/fairyskills.coax`): each skill is
learned once and named over the foot with what it lets you do. BLOOM is your
first bloom; SPEAK the first time a creature answers you; MEND the hurt
bluebird in Mossbank's south-west (E: MEND, and it flies off well); GATHER
your first find (berries, acorns and seed pods in each glade, each spot
growing again the next day; a seed is a seed for your garden, and once you
gather the finds sparkle); CRAFT the woodcutter's axe in a stump east of your
home (E: TAKE THE AXE), and then E: CUT clears a fallen log of deadwood;
LIGHT the lantern flower at Lanternwood's north edge, which opens only at
night (E: LIGHT; by day E: REST TILL NIGHT), and then you carry a little
light, a warm pool round you in the dark; WATER the sticks dammed across the stream above the
pond (E: CLEAR), and then WATER at one bed waters your whole garden. Each one
learned puts a gem on her upper wings. Kept in the save's tail. Not yet: a
woodcutter as the moment, making tools, mending
broken things, moving water or filling
the pond.

## The core: helping people

The forest's people are animals and small folk who live in each glade. They
remember you (the character memory the engine already has: they recall what
you told them and what you did for them).

- **Each one has a trouble:** a hedgehog's burrow flooded, a frog choir lost
  its voice, a badger can't find the way home, a beetle family needs a new
  log. Each is solved with skills, finds and things you make.
- **You can see how they feel.** Every creature shows its mood at a glance:
  a small expression and a mood glyph over its head (happy, worried, lost,
  hungry, tired), and it says little things as you pass ("the stream smells
  wrong today", "is it far to the reeds?"). Visitors to your home do the same.
  Making the forest's people happier is the point; the moods make that
  visible.
  Built (ff-moods): five feelings, happy, worried, lost, hungry and tired.
  Grey, each creature feels what its trouble gives it (Bramble hungry, Pip
  and Hollow tired, Clover worried, Burdock lost) and shows it in its face
  (brows, mouth, half-shut eyes), how it moves (a shiver, looking about,
  sniffing low, a slow sag) and its glyph (a ?, a z, an acorn, a drop, or
  sparkles once happy); paler while partly helped. Passing within 12 m it
  says one of three remarks, the next each time you come by, two once happy.
- **Helping changes the place.** Solving a glade's troubles restores it: the
  fade lifts there, its people move in closer, and new creatures arrive.
- **The story's spine:** each glade restored wakes one of the heart tree's
  five roots. All five, and the heart blooms: the ending.

## Your home and the community

- **Your home** starts as a hollow under a root in Mossbank. You grow it room
  by room with what you forage and make: a bed of moss, a table, shelves for
  your finds, a garden, the mirror pool.
- **Build** with the edit tools the Haunted House and the Sandbox already
  have: place, move, turn, size, undo, by hand or in one sentence.
- Built (ff-home, `apps/openagents-web/fairyhome.coax`): the hollow sits
  under a great root in Mossbank's west (about -46, -44), with a bed of
  moss, a lamp and the mirror pool. Each root you wake opens one more room:
  the garden, the table room (a table and stool), the shelves, the green.
  E (BUILD) in your home puts a bar along the bottom: eight pieces (bed,
  table, stool, shelf, lamp, rug, flowers, toadstool) and PLACE, MOVE, TURN,
  SIZE, UNDO for the piece in front of you, the edit tools' verbs. Pieces go
  only in open rooms. It is kept in the save's tail, yours alone: the shared
  edit layer's placing is realm-wide and the owner's, so the home keeps the
  verbs but not that path. Not yet: building by one sentence.
- **The community:** creatures you help can come to live near you. Each one
  that moves in builds its own small home on the plot you give it, and comes
  by to visit. The village around your home grows with how much you have
  helped.
  Built (ff-village, `apps/openagents-web/fairyvillage.coax`): by a creature
  whose glade is green, E is INVITE (name) HOME; it takes the next of five
  plots round your home's clearing and builds its house there in its own
  colours over two minutes (frame, walls, roof), then sits at its door while
  you are home. Kept in the save's tail.

## Caring for something until it changes

- **Your garden:** plant seeds you forage; water, weed and sing to them.
  Plants grow over real days, and what you care for most changes into
  something rarer: a sapling into a glowing tree, a flower into one that
  attracts a rare visitor.
- Built (ff-garden, `apps/openagents-web/fairygarden.coax`): six beds in
  the garden room (open with the first root). A seed comes for each patch
  you bloom and one a day you come. E at a bed: PLANT, then WATER and SING,
  each counting once in six hours. A sapling, a flower and a mushroom are
  planted in turn; by real time a plant sprouts in two hours, is young in a
  day and grown in two; with four cares it grows rare: a glowing tree, a
  moon flower with a lantern moth, a glowcap. Kept in the save's tail.
- **Visitors to the garden:** certain plants draw certain creatures. Grow the
  right mix and new animals come to stay; care for them and they change too
  (a moth into a lantern moth, a frog into a singing frog).
- **Rags to riches:** you begin with nothing but a flower and end with a
  blooming forest, a full home and a village of friends.

## Making things to share

- **Trails and courses:** build a course through the trees (hoops of vines to
  fly through, stepping stones, glide drops) with the edit tools, time
  yourself on it, and share a link. Friends play your course and their times
  show on it.
  Built (ff-courses, `apps/openagents-web/fairycourse.coax`): COURSE, by the
  title, opens a bar: HOOP (or E, in the air too) hangs a vine hoop ahead of
  you, up to twelve; UNDO, FLY (the clock starts at the first hoop and stops
  at the last; the next one glows), SHARE (copies a link,
  `#course=<id>`) and DONE. Whoever opens the link finds the same hoops, and
  the ten fastest runs show beside them with names (AGAIN, CLOSE). Kept on
  the server (`/api/playtest/course/<id>`,
  `coax/web/api/migrations/0057_fairy_courses.sql`); your own course and
  best are in the save's tail. Stepping stones and glide drops are not built
  yet.
- **Your forest is shareable:** a link to your home and garden, as a visit
  (the playtest's published worlds).
- **Visitors:** other players can visit your forest.

## Talking with other players

- **The chat room:** one room for everyone in the forest, in the style of
  Horse Isle's: a chat box always on screen, every player's lines in order
  with their name, open to anyone in the realm. Built (ff-chat-room,
  `apps/openagents-web/fairyroom.coax`): on a computer the box sits at the
  bottom left and Enter types; on a phone it is the latest line along the
  top, and a tap opens the last six with a field to type in. The others in
  the forest fly where they are, with their names over them. BLOCK on a
  line hides that player's lines and messages for you; an admin sees
  REMOVE on every line.
- **The inbox:** direct messages between players. Send a message to any
  player you've met; it waits in their inbox until they read it, whether
  they're online or not, and they can answer from there. Built (ff-inbox):
  INBOX on the chat box, with how many are new, opens a pane of the players
  you have met (been in the forest with at once); OPEN shows what you said
  to each other and a field to write. Messages are kept on the server
  (`coax/web/api/migrations/0053_verse_social.sql`): at most 500
  characters, five a minute and a hundred a day, only between players who
  have met, and never from someone you block.

## Making the world: panes

The designer's notes, October 29. They hold for building any world on the
Verse, and Fairy Forest is where they are tried first.

- **No wall of tools.** Edit mode does not open on every pane at once. The
  agent brings up the pane that fits what you are working on, and
  only that.
- **A game from one voice prompt.** Describe a game out loud and the agent
  answers with its design document as a board: a list of features, each a
  few words. You check on the ones you want built and check off the ones you
  don't. The board is what the agent builds from.
- **Lore.** Working on a character's story, a pane opens for that
  character's lore. What you write is saved with the character, and the pane
  comes back the next time you work on them.
- **Quest design.** Working on a quest, a pane opens with a form for it: who
  gives it, what is asked, the steps, what you get. Saving the form makes
  the quest.
- **Other panes** follow the same rule: one pane for the thing in front of
  you, short words, saved and brought back later.
- **Accept or decline.** Every change someone else proposes to a world (a
  player's, a contributor's or the agent's) reaches its owner as a clear
  card: what changes, where, and what it looks like before and after, with
  ACCEPT and DECLINE. Nothing changes the world until the owner accepts.

Built (ff-panes): in Fairy Forest's EDIT, the board is the shared edit
layer's (verse/edit/edit.coax, docs/verse/realm-edit.md), as are the
settings and the cards. Coming to a creature with EDIT open brings up its
LORE; QUEST turns it into the form (ask, three steps, reward) and SAVE QUEST
makes the quest. The creature knows its lore when you talk to it, and gives
its quest when you come near: the quest shows at the top left and you tick
its steps (apps/openagents-web/fairyquest.coax).

## Lore and wandering

- **The Old Ring** holds the forest's history: stones that tell, when you
  bring light to them, how the fade began and what the heart tree is.
- **Every glade has a story** its people tell piece by piece as they come to
  trust you.
- **After the ending** the forest is open: keep gardening, building, helping
  newcomers, and making courses.

## What it reuses

| Need | From |
| --- | --- |
| Painterly look | The art-style presets (art-styles.md) |
| Walking, flight, camera | `verse/player.coax`, `verse/camera.coax` |
| People who remember you | The character chat and memory Everglade's people use |
| Quests and the story's steps | Everglade's quest board and quest state |
| Building, editing, one-sentence pieces | The Haunted House and Sandbox edit tools, `verse/setpiece.coax` |
| Voice | The voice engine and push-to-talk (voice edits in Everglade) |
| Stream, pond, waterfall | `verse/water.coax`, `verse/water-look.coax` |
| Wind-swept grass, glowing fruit, particles | The painterly grass and wind tasks, `verse/particles.coax`, the glow material |
| Saving where you are and what you made | Player progress and published worlds |

## What it needs that isn't built yet

- Flight and gliding in the controller.
- The fade: per-area colour restoration that spreads, saved per player.
- Panes: the agent bringing up the right pane, the design board from a voice
  prompt, the lore and quest-design panes, and accept-or-decline cards for
  the world's owner.

## Not in the first version

- Playing with your body through a camera (moving to make your character
  move) is an idea worth trying later; it is not needed for the forest.
- Careers, money and shops.
- Combat. Troubles are solved by helping, not fighting.
