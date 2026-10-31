# Fairy Forest

A Verse realm, drawn flat, you walk in the browser at https://openagents.com/realms/fairy-forest. It is a first version, unlisted: the galaxy shows it only with `?realms=all`. The plan for the whole realm is in `verse/fairy-forest/README.md`.

## The park

Central Park at a quarter of its size: a long rectangle about 1 km from 59th Street in the south to 110th Street in the north and 208 m across, between Central Park West and Fifth Avenue, with low blocks of the avenues round it. Gentle rolling ground with Umpire Rock, Vista Rock and other outcrops; the Pond with Gapstow Bridge, the Lake with Bow Bridge and the Loeb Boathouse, Turtle Pond, the Reservoir, Harlem Meer and Conservatory Water; the drives' loop, footpaths and the Mall's Literary Walk to Bethesda Terrace and the Angel of the Waters. Simple low-poly trees and bushes, and as plain shapes the Carousel, the Dairy, Wollman Rink, the Bandshell, Strawberry Fields' Imagine circle, Alice in Wonderland, Belvedere Castle, the Delacorte Theater, Cleopatra's Needle and the Conservatory Garden's gate. Each landmark's name shows over it as you come near. You walk over Gapstow and Bow Bridges on their decks, and over the footbridge on the Loch once it is mended.

Press M (or MAP by the title) to see the whole park from above with every landmark's and glade's name and a ring where you are; M, Esc or WALK takes you back. `/realms/fairy-forest#map` opens on it.

The heart tree stands in the Ramble. Five glades: Mossbank by the Pond, where you wake; The Reeds at the Pool, where the Loch runs into a pond on its way to Harlem Meer; Lanternwood in the North Woods; Thornfall at the north edge, brambles round a fallen giant tree; and the Old Ring by Cleopatra's Needle, nine standing stones. The glades and the heart tree start faded, grey and pale, and colour spreads outward from everything you restore.

## You

You are a fairy with glowing wings that beat slowly when you stand and quickly when you fly. Move with WASD (on a phone, your left thumb); you run unless you hold Shift. Turn the camera with a mouse drag, the arrow keys or your right thumb. Every Space press is a wing beat up, with no limit; hold Space to keep climbing, let go to glide down, and hold Shift in the air to dive. On a phone, tap the right side to flap and hold it to climb. 

## Bloom

Faded patches of flowers lie along the trails and in the glades. Stand by one and press E (on a phone, the BLOOM button) to bloom it: it comes back to colour, and the colour spreads round it. The Old Ring's dark stones are lit the same way (LIGHT). While you fly the prompt hides; land to bloom.

## The creatures

Each glade has a creature with a trouble: Bramble the hedgehog in Mossbank, Pip the frog in The Reeds, Clover the rabbit in Lanternwood, Burdock the badger in Thornfall and Hollow the owl in the Old Ring. A glyph over each shows its mood, it says something as you pass, and when you come near it tells you what is wrong. Bloom the patches of its glade and its mood turns happy. A moment later one of the heart tree's roots wakes, glows, and floods the ground round it with colour. Wake all five and the heart tree blooms: colour comes back to the whole park, and the forest stays open to wander.

## Talking

Press E by a creature, or click or tap it (TALK on a phone), to talk to it. Type and press Enter to say something; Esc or BYE ends the talk. It answers in character, knows its trouble and how far you have helped, thanks you once its glade is green, and remembers what you told it on earlier visits (the character chat, `POST /api/playtest/npc/ask`). Who each creature is lives in `verse/fairy-forest/talk.coax`.

## The look

The flat style (`docs/verse/art-styles.md`, Flat): flat-shaded low poly in a cosy pastel palette of olive and lime, cattail orange, pale mint, lavender and cream, three flat tones of light a shape, flat translucent shadows, a flat sky over a soft horizon band, no grass, the stream in flat bands with a cream shore line, and the lakes in a few flat tones by depth with light bands along the shore and slow ripple lines. The shading functions and the palette are `verse/style.coax`; the layout, creatures and the restore loop are `verse/fairy-forest/forest.coax` and `verse/fairy-forest/park.coax`, and the stream and pond are `verse/fairy-forest/stream.coax`.

## Saving

Where you are and what you restored (the bloomed patches and the woken roots) is kept in this browser and, with an account, on the server; a reload puts it back.

## Not yet

No voice talk yet.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
