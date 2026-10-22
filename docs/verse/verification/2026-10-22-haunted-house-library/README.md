# The Haunted House's prepared library (#12475)

Every asset in `crates/haunt-assets/catalog.json` drawn by the house's own
shell: `contact-sheet.jpg` is the ten pages of `/#library=N` (cards, then
meshes, then effects), each named under it. The sixteen cards are drawn in
code (`haunt_assets::cards`) in palette colours with a paper edge and a
licorice border; the meshes are made in code (`haunt_assets::meshes`); the
effects animate in the shell (fog drifts, a drop falls, eyes blink, a cup
floats, footprints appear). Lightning, flicker and door slam are light and
sound only, so their places on the last page are empty.

`sound-test.json` is `/#library&sounds`: the page played every one of the
36 library sounds in turn (29 effects, 6 ambience beds, and the
`bassline-minor` music), each made in the page with Web Audio through its
bus, with the audio running. Every sound's source is in the catalog; none
is a recording.

`haunt describe --library` (`cargo run -p haunt-rules --bin haunt --
describe --library`) lists the same catalog for an agent.

Still to do: Kiki (Creative Director) reviews and signs off the cards.
