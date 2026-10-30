# Fairy Forest

A Verse realm, drawn flat, you walk in the browser at https://openagents.com/realms/fairy-forest. It is a first version, unlisted: the galaxy shows it only with `?realms=all`. The plan for the whole realm is in `verse/fairy-forest/README.md`.

## The valley

A valley about 160 metres across, ringed by soft hills, with the heart tree in the middle. Trails run from the heart tree to five glades: Mossbank in the south-west, where you wake; The Reeds by the pond in the east; Lanternwood in the north-west; Thornfall in the north, brambles round a fallen giant tree; and the Old Ring in the south, nine standing stones. A stream winds down the valley into the pond. The valley starts faded, grey and pale, and colour spreads outward from everything you restore.

## You

You are a fairy with glowing wings that beat slowly when you stand and quickly when you fly. Move with WASD (on a phone, your left thumb); you run unless you hold Shift. Turn the camera with a mouse drag, the arrow keys or your right thumb. Every Space press is a wing beat up, with no limit; hold Space to keep climbing, let go to glide down, and hold Shift in the air to dive. On a phone, tap the right side to flap and hold it to climb. You can stand on the tree crowns.

## Bloom

Faded patches of flowers lie along the trails and in the glades. Stand by one and press E (on a phone, the BLOOM button) to bloom it: it comes back to colour, and the colour spreads round it. The Old Ring's dark stones are lit the same way (LIGHT). While you fly the prompt hides; land to bloom.

## The creatures

Each glade has a creature with a trouble: Bramble the hedgehog in Mossbank, Pip the frog in The Reeds, Clover the rabbit in Lanternwood, Burdock the badger in Thornfall and Hollow the owl in the Old Ring. A glyph over each shows its mood, it says something as you pass, and when you come near it tells you what is wrong. Bloom the patches of its glade and its mood turns happy. A moment later one of the heart tree's roots wakes, glows, and floods the valley round it with colour. Wake all five and the heart tree blooms: colour comes back to the whole valley, and the forest stays open to wander.

## Talking

Press E by a creature, or click or tap it (TALK on a phone), to talk to it. Type and press Enter to say something; Esc or BYE ends the talk. It answers in character, knows its trouble and how far you have helped, thanks you once its glade is green, and remembers what you told it on earlier visits (the character chat, `POST /api/playtest/npc/ask`). Who each creature is lives in `verse/fairy-forest/talk.coax`.

## The look

The flat style (`docs/verse/art-styles.md`, Flat): flat-shaded low poly in a cosy pastel palette of olive and lime, cattail orange, pale mint, lavender and cream, three flat tones of light a shape, flat translucent shadows, a flat sky over a soft horizon band, a few short tufts of grass and the water in flat bands with a cream shore line. The shading functions and the palette are `verse/style.coax`; the valley's layout, creatures and the restore loop are `verse/fairy-forest/forest.coax`, and the stream and pond are `verse/fairy-forest/stream.coax`.

## Saving

Where you are and what you restored (the bloomed patches and the woken roots) is kept in this browser and, with an account, on the server; a reload puts it back.

## Not yet

No voice talk, no home or garden, and no chat room yet.
