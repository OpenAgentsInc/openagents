# Fairy Forest

A painterly Verse realm you walk in the browser at https://openagents.com/realms/fairy-forest. It is a first version, unlisted: the galaxy shows it only with `?realms=all`. The plan for the whole realm is in `verse/fairy-forest/README.md`.

## The valley

A valley about 160 metres across, ringed by soft hills, with the heart tree in the middle. Trails run from the heart tree to three glades: Mossbank in the south-west, where you wake; The Reeds by the pond in the east; and Lanternwood in the north-west. A stream winds down the valley into the pond. The valley starts faded, grey and pale, and colour spreads outward from everything you restore.

## You

You are a fairy with glowing wings that beat slowly when you stand and quickly when you fly. Move with WASD (on a phone, your left thumb); you run unless you hold Shift. Turn the camera with a mouse drag, the arrow keys or your right thumb. Space hops; press it again in the air to flap higher, up to three times a hop, and hold it while you fall to glide.

## Bloom

Faded patches of flowers lie along the trails and in the glades. Stand by one and press E (on a phone, the BLOOM button) to bloom it: it comes back to colour, and the colour spreads round it.

## The creatures

Each glade has a creature with a trouble: Bramble the hedgehog in Mossbank, Pip the frog in The Reeds and Clover the rabbit in Lanternwood. A glyph over each shows its mood, and when you come near it tells you what is wrong. Bloom the three patches of its glade and its mood turns happy. A moment later one of the heart tree's roots wakes, glows, and floods the valley round it with colour.

## The look

The painterly style (`docs/verse/art-styles.md`): banded toon shading with a warm rim light, blue and violet shadows, a blue haze that thickens with distance, a painted gradient sky with soft clouds, gentle bloom, swaying grass and broad flat colour on the ground. The water is painted too: bands of colour by depth, the sky in its reflection, sparkles and a white shore line. The shading functions are `verse/paint.coax` and `verse/style.coax`; the valley's layout, creatures and the restore loop are `verse/fairy-forest/forest.coax`, and the stream and pond are `verse/fairy-forest/stream.coax`.

## Not yet

Your progress is not saved, the creatures do not talk back, and only three of the heart tree's five roots can wake.
