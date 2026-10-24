# Medieval Town shot-by-shot reconstruction guide

Status: all 18 displayed references inspected, 2026-10-22. A local renderer and
initial camera hypotheses exist; no shot is accepted yet. Read the [parity plan](medieval-town.md) for source
limits, shared capabilities, implementation order, recipes, and acceptance policy.
The [gallery inventory](medieval-town-gallery.json) pins each source image's URL,
dimensions, byte length, and SHA-256. Images remain outside Git.

Observations describe visible pixels. Reconstruction choices and dependencies
are proposals, not recovered Unreal settings. Camera parameters, lighting
intensities, and original rendering techniques remain unknown unless exported
and verified. Water, foliage, particles, and temporal quality require moving
evidence in addition to a still match.

| ID | View | Type | First gate |
| --- | --- | --- | --- |
| MT-01 | Promotional day/night comparison | Composite presentation | REF-4 |
| MT-02 | Day square, steps, hero house, distant castle | Scenic daylight | REF-2 |
| MT-03 | Fountain courtyard and green-timbered house | Scenic daylight | REF-3 |
| MT-04 | Modular assets on an overview stage | Asset catalogue | REF-4 |
| MT-05 | Curved street, bunting, castle and pale ridges | Scenic daylight | REF-3 |
| MT-06 | Square through shaded furniture and railings | Scenic daylight | REF-3 |
| MT-07 | Blue entrance and ivy shadows | Scenic daylight | REF-1 |
| MT-08 | Warm lantern-lit entrance | Scenic night | REF-1 |
| MT-09 | Fountain column, streams and flowers | Scenic daylight | REF-3 |
| MT-10 | Cool fountain against warm windows | Scenic night | REF-3 |
| MT-11 | Moonlit square and castle | Scenic night | REF-2 |
| MT-12 | Gabled house behind dark railings | Scenic night | REF-3 |
| MT-13 | Lantern doorway and foreground produce | Scenic night | REF-3 |
| MT-14 | Night courtyard and golden fountain | Scenic night | REF-3 |
| MT-15 | Tavern square beneath cool sky | Scenic night | REF-3 |
| MT-16 | Cafe, ivy and cropped fountain | Scenic daylight | REF-3 |
| MT-17 | Castle behind layered roofs | Scenic daylight | REF-3 |
| MT-18 | Hero townhouse and mountain backdrop | Scenic daylight | REF-3 |

## MT-01: Promotional day/night comparison

[Source image](https://media.fab.com/image_previews/gallery_images/ae6b96e4-d2ab-4ea4-94ed-e6931b1b5550/a7197a03-0824-425e-9c77-c352256eb33f.jpg)

**Observed:** This is a designed promotional plate, not a single camera render.
Its main inset pair shows a cool moonlit town on the left and a bright daylight
square on the right. A desaturated town image, large heading, icons, small
silhouettes, and a lower banner surround them. Architecture repeats across the
two lighting views; the night image includes warm windows and bright floating
specks, while the day image exposes plaster, timber, tiled roofs, and distant
castle detail.

**Reconstruction proposal:** Use the accepted MT-11 and MT-02 recipes as the
corresponding visual targets, matching each framing. Produce an Aethyrforge-owned
side-by-side comparison with fixed output handling and clearly labeled day/night
presets. Marketing typography, logos, and graphic layout are excluded from
renderer parity; no single in-world camera can reproduce this composite.

**Implemented export:** Run `tools/capture-comparison.mjs` through the isolated
browser runner, with `AF_CAPTURE_DIR` pointing outside the repository. It captures
MT-11 and MT-02 at the same explicit animation age (three seconds by default),
then places both original 1280 × 720 renders side by side with owned labels.
The 2560 × 784 output preserves each render without cropping, grading, or
resampling. `AF_COMPARISON_TIME` selects another age from zero to 24 hours.
The original scene images and a verification record remain beside the comparison.
This export does not copy the vendor's promotional artwork or accept either scene.

**Pass/fail:** Both constituent scenes must pass independently. Fail if a polished
comparison layout conceals an unmatched scene, or if night is merely a blue filter
over daylight. This reference is a presentation gate rather than a seventeenth
scenic shot.

## MT-02: Day square, steps, hero house, and distant castle

[Source image](https://media.fab.com/image_previews/gallery_images/01c9ac1d-57fd-47bd-ad9e-bf20270bfe8e/12381eb0-2310-4c3e-a4d0-8e130bd566c2.jpg)

**Observed:** Broad stone steps lead from a dark market foreground into the
square. A tall timber-framed house with round attic window, flower boxes, brick
chimneys, and a conical-roofed tower anchors the right. Smaller houses and bunting
recede centrally. A pale castle occupies the upper left behind layered ridges;
large bright clouds fill the sky. Produce baskets, leaves, barrels, and iron
railings supply near detail. Sunlit stone and plaster contrast with deep shaded
facades and sharply shaped foreground shadows.

**Reconstruction proposal:** Start a low wide view below the steps. Solve stair
rise, hero-house silhouette, street vanishing point, and castle placement before
adjusting the sky. Preserve foreground occluders rather than centering a lone
building. Required systems include faithful town placement, PBR detail, direct
and indirect light, stable fine shadows, foliage, depth-aware atmosphere, and
cloud/mountain layers.

**Pass/fail:** The steps must lead into the same depth structure; the near house
stays crisp while the castle recedes. Fail on missing street levels, an empty
background, uniformly black shade, or clouds used to hide absent geometry.

## MT-03: Fountain courtyard and green-timbered house

[Source image](https://media.fab.com/image_previews/gallery_images/eb1e8706-940f-4fad-be0a-78b5edb5eb98/69c14118-13b4-4acc-a73c-3d726905157c.jpg)

**Observed:** A circular stone fountain fills the lower center. Water arcs rise
in front of a green-timbered, weathered plaster house with shutters and a small
balcony. A cafe awning, menu board, chairs, tables, lamps, and flowers dress the
left edge; a bench and tree extend the right. Bright directional light reveals
surface wear, with hard eave/leaf shadows and shaded ground-floor recesses.
Stone, plaster, wood, cloth, foliage, and water retain distinct appearances.

**Reconstruction proposal:** Use a close courtyard view that keeps the full
fountain rim, projected balcony, awning, and right-hand tree in their reference
relationships. Match geometry and material slots before tuning exposure. The
pipeline needs readable indirect shade, detailed normal/roughness response,
leaf coverage and shadows, transparent fountain jets/basin, contact grounding,
and stable fine edges.

**Pass/fail:** Water separates from the facade, the fountain sits on the paving,
and the shaded house stays textured. Fail on missing cafe dressing, flat walls,
an opaque white fountain, disappearing thin furniture, or excessive bloom on
bright stone. Use MT-14 as the related night courtyard target.

## MT-04: Modular assets on an overview stage

[Source image](https://media.fab.com/image_previews/gallery_images/fe1e5fa4-e6bd-4e36-ab56-4ed2022c716d/6611fc76-3118-4286-8ac1-5b927a661661.jpg)

**Observed:** An elevated oblique view shows component rows on a broad neutral
plane beneath a simple blue sky. Castle towers, arches, and wall sections stand
at the back. House modules, roof pieces, street parts, fountains, props, foliage,
flags, and flat decal-like items form organized rows below. Long directional
shadows communicate scale and shape. Drawn labels/arrows identify groups. This
is a catalogue layout rather than a populated town.

**Reconstruction proposal:** Build a private import-review stage with bounded
groups and original labels. Fit an elevated camera to the overall footprint,
then check each family in close views. Preserve pivots, transforms, units,
material assignments, alpha, UVs, and mesh detail. Use it to expose missing or
inverted pieces before assembling the scenic shots; exact original label artwork
is outside the renderer target.

**Pass/fail:** Required asset families and their silhouettes are represented and
correctly shaded, without mirrored geometry, wrong scale, or opaque leaf cards.
This image cannot prove walkable interiors, material parity at close range, or
the whole town's interactive performance.

## MT-05: Curved street, bunting, castle, and pale ridges

[Source image](https://media.fab.com/image_previews/gallery_images/dec84dc4-8590-48ee-959c-07529b1bc577/8b6b9745-8cec-442e-a5cd-855a2d325b85.jpg)

**Observed:** A curving paved street and central stair run pull the view through
closely packed houses. The near left market facade is dark; the right plaster
house and orange roofs catch strong sunlight. Several bunting strings cross the
street at different depths. Barrels, sacks, benches, carts, trees, and planters
keep the ground occupied. Castle towers rise left of center; pale rock/ridge
layers and distant hillside buildings fill the high background beneath clouds.

**Reconstruction proposal:** Solve the street curve, stair landings, foreground
facade crops, and stacked roof lines before dressing the view. Keep distant
terrain/castle as separate depth layers with explicit exposure and atmosphere.
Required systems include full scene placement, indirect fill under eaves,
directional/contact shadows, texture detail, thin pennant coverage, sky/clouds,
distance haze, and suitable LOD/streaming.

**Pass/fail:** The street remains legible through alternating shadow and light;
castle and ridges sit behind the town. Fail on flat terrain, an evenly lit street,
detached bunting, or a backdrop with the same contrast as the near houses.

## MT-06: Square through shaded furniture and railings

[Source image](https://media.fab.com/image_previews/gallery_images/dfb6201d-131a-40b1-ab4a-6c6d79cc08ad/e9507400-d610-4ccb-94d5-5c075db0b233.jpg)

**Observed:** A shaded chair crops the lower left and a black railing frames the
lower right. Beyond them, a broad sunlit fountain rim crosses the square. An
arched wooden door, flowering window boxes, and a hanging banner anchor the
left; a timbered house and tree occupy the right. The central background house
is cooler and darker beneath orange roofs. Near leaf shadows pattern the paving;
facade recesses, balcony rails, and timber divisions keep their shape.

**Reconstruction proposal:** Match a low view from the shaded square edge,
preserving the foreground frame and fountain-to-door proportions. Use actual
tree geometry and the correct sun direction to create ground shadows. Separate
direct sun, sky/indirect fill, and material color rather than flattening the
shaded middle house. Fine foliage, railings, contact shadows, texture filtering,
and controlled HDR response are essential.

**Pass/fail:** Near furniture remains dark but identifiable, and the bright rim
retains stone detail. Fail if foreground occlusion is removed to brighten the
image, the fountain floats, or shadowed architecture becomes uniformly black.

## MT-07 — Sunlit blue entrance and ivy shadows

[Source image](https://media.fab.com/image_previews/gallery_images/3849746b-95b4-4cc2-807e-5b4d078e0dd8/f8353ce7-aecd-4b0a-845e-2833923a75c2.jpg)

**Observed:** A blue double door fills the right-middle, raised above short stone steps between pale sloping parapets. Black railings and a flower bed occupy the lower left; timber-and-plaster facades recede leftward. Ivy crowns the upper edge and descends across the wall. Strong directional illumination leaves crisp leaf, lantern, and parapet shadows on bright plaster. The doorway recess stays dark. Reddish timber grain, chipped blue paint, stone roughness, and weathered lantern metal remain visible; the palette combines white, brown, green, and blue without a colored atmospheric wash.

**Reconstruction proposal:** Use an oblique pedestrian-height close shot with the stairs entering from the bottom and the door right of center. Match projected door/parapet dimensions before adjusting lighting. Required systems are fine material detail, stable directional shadows, alpha-tested foliage with correct shadowing, recess/contact shading, and controlled highlight response. Exact texture-map roles remain unknown.

**Pass/fail:** Preserve the door recess and leaf-shadow pattern while keeping sunlit plaster textured. Fail on clipped white walls, uniformly lit panels, floating ivy, or a roof/parapet silhouette that misses the composition.

**Current camera qualification:** The prototype uses a fixed-field-of-view,
zero-roll reconstruction fitted to the image. At 1280 × 720, 33 selected mutual
SIFT feature matches have a median displacement of 0.681 pixels, compared with
41.35 pixels for the previous fallback. Background discrepancies reach
110 pixels. This supports the near-facade composition hypothesis; it does not
establish complete camera, geometry, or rendering parity.

## MT-08 — Lantern-lit entrance with warm falloff

[Source image](https://media.fab.com/image_previews/gallery_images/fe8a1003-0a38-4bdb-ba66-d1820697dc41/c64463bd-423d-4579-bac5-f8b966817f84.jpg)

**Observed:** A closely related doorway composition appears at night: stairs and pale parapets rise toward a dark door, with ivy and timber framing above and railings at left. The right lantern and smaller lamps to the left contain bright warm cores. Amber illumination pools across plaster, timber, and the stair flanks; the door panels and left recess remain substantially darker. Highlighted wood has a glossy, worn appearance. A warm veil softens contrast, but edges and near-surface texture remain legible. There is no clear bright moonlit background in this crop.

**Reconstruction proposal:** Reuse a matched entrance layout and camera, then tune individual practical-light falloff and low ambient contribution. Lantern emission, local shadows, roughness variation, restrained glow, and optional localized haze are required candidates; this still image does not identify the original technique. Warm color grading alone cannot reproduce the spatial light pools.

**Pass/fail:** Bright sources must coexist with a dark door and readable stair geometry. Fail on uniformly orange surfaces, giant glow discs, missing shadow structure, or an entrance that becomes unreadably black.

**Current camera qualification:** The private gallery mapping selects the
night scenario's recovered `CineCameraActor16`. At 1280 × 720, 80 selected mutual
SIFT feature matches have a median displacement of 0.605 pixels; 68 are within
3 pixels. This is evidence for the composition mapping. Lighting, material,
exposure, glow, and moving-camera acceptance remain separate requirements.

## MT-09 — Daylight fountain, flowers, and tavern backdrop

[Source image](https://media.fab.com/image_previews/gallery_images/e9c03320-e9df-4411-96f4-5fce8f822650/4011434e-8fc9-45e0-8f04-d02d9d4a798b.jpg)

**Observed:** A cylindrical red-and-stone fountain column dominates the right-middle. Dark pipes extend sideways on thin supports; the left outlet pours a curved translucent stream into the foreground basin. Dense green leaves and blue flowers obscure the column base. Orange fish-like forms appear in the water. A barrel, hanging sign, bench, tree, and layered timber facades populate the middle/background. Strong daylight produces hard-edged shadows, while the stone cap, rough red cylinder, corroded pipework, foliage, and rippled water have distinct surface responses.

**Reconstruction proposal:** Position a low close camera near the basin edge, preserving the fountain/tree overlap and the left pipe's diagonal flow. Required systems include textured architecture/props, foliage cards or equivalent detailed leaves, directional/contact shadows, layered transparent water, a readable basin surface, and bounded splash detail. Continuous flow animation is a proposed runtime extension, not evidence established by the JPEG.

**Pass/fail:** Match column scale, pipe silhouette, flower density, and transparent stream separation. Fail on opaque white water, disappearing supports, uniformly glossy stone, or an empty backdrop.

## MT-10 — Cool fountain against warm night windows

[Source image](https://media.fab.com/image_previews/gallery_images/3d3371ef-938e-40cc-befa-0d224e8dd2a9/94abac99-5b08-4848-8407-e7a562e0411c.jpg)

**Observed:** The fountain is again near the right-middle, with its left outlet spilling into a foreground pool and flowers masking its base. Blue/cyan illumination picks out the stone cap, column, upper walls, and intense blue blossoms. Warm lanterns and door/window openings punctuate the left and rear facades. The tree bridges these light regions. A soft amber veil crosses the middle distance while foreground pipes remain dark and sharply outlined. White water and broken surface reflections stand out against the basin's dark blue surround; orange forms remain visible beneath or on the water.

**Reconstruction proposal:** Begin from the daylight fountain's matched scene, adjusting framing if needed rather than assuming identical cameras. Separate cool environmental/key illumination from warm practical lights. Retain source texture variation under darkness. Water transparency, moving highlights, local lighting/shadows, foliage shading, and depth-aware atmospheric treatment are required candidates; their original implementations are unknowable here.

**Pass/fail:** Keep cool fountain mass distinct from warm openings and the water readable. Fail on a global blue filter, flat orange fog, missing flower silhouettes, or a uniformly emissive stream.

## MT-11 — Moonlit square, castle silhouette, and floating lights

[Source image](https://media.fab.com/image_previews/gallery_images/ff6c73f6-a3cf-4131-8af3-792d37dfaed0/88eaacfb-f463-4c43-b5ea-0db97888cb82.jpg)

**Observed:** Broad steps lead from the dark foreground into a town square. A large half-timbered building anchors the right, smaller roofs cluster centrally, and a turreted castle silhouette rises far left. A bright moon sits above a pale ridge near the center. Cool clouded sky and layered blue haze separate the distances. Warm windows and lanterns punctuate lower buildings. Bright yellow floating specks cross foreground and distance; leaves and bird-like silhouettes add small-scale detail. Upper foliage and an overhanging roof frame the image. Thin cool highlights articulate the stair edges.

**Reconstruction proposal:** Use a low wide square view with the steps as a lead-in; arrange near architecture, mid-square, ridge, and castle as separately controlled depth layers. Required candidates include atmospheric depth, sky/moon placement, warm local lights, stable shadow silhouettes, bounded glowing particles, distant geometry/LOD, and restrained bloom. The specks' identity and animation cannot be established from one frame.

**Pass/fail:** Preserve near-dark framing, readable village, and softened distant castle. Fail on uniformly bright blue surfaces, oversized particle glow, absent depth separation, or a moon detached from the ridge composition.

## MT-12 — Low-angle gabled house behind dark railings

[Source image](https://media.fab.com/image_previews/gallery_images/d629e855-fccb-4ad0-bd7d-a7c1b7fc8a3c/1dda3151-fd4f-4cf7-a4c2-c898eb518814.jpg)

**Observed:** A tall gabled half-timbered house occupies the center-right, with a circular attic window, projecting upper facade, flower boxes, and a narrow roofed tower at right. Near-black railings cut across the bottom, while a leafy tree silhouettes the upper left. Neighboring roofs form a lower background band. Cool blue lighting defines plaster planes and roof edges; warm windows and the entrance draw attention to inhabited interiors. Hazy streaks cross the sky behind the building. The upper mass stays darker than the warmly accented lower facade, and sparse small silhouettes/specks appear against the sky.

**Reconstruction proposal:** Set a low upward-looking camera behind the railing, preserving roof peak, tower, tree border, and circular window proportions. Required systems include accurate facade/roof geometry, layered foliage, cool environmental light, window/entrance emission with nearby light spill, contact shadowing, and restrained atmospheric depth. Exact lens and moon direction remain unknown; do not substitute color grading for geometric lighting.

**Pass/fail:** Keep silhouette and timber pattern readable without flattening night contrast. Fail on missing turret/railing overlaps, oversized window glow, uniformly black upper floors, or misplaced sky haze.

## MT-13: Lantern-lit doorway and foreground produce

**Visible:** A close street-level view centers a recessed wooden double door
behind an iron railing. Blue-lit plaster and timber fill the left; two warm
wall lanterns illuminate the doorway and yellow-green right facade. A tree
trunk crops the left edge. Flowers span the lower middle, while barrels,
baskets, and produce become dark foreground silhouettes. Wood grain and worn
plaster remain visible near highlights; overhangs and recesses retain deep
shadow. This is spatially separated illumination, not one warm color grade.

**Reconstruct:** Match the doorway, railing, crop, and foreground depth before
tuning light. Combine a cool directional/environment contribution with small
warm sources positioned inside the visible lanterns. Preserve local falloff
and architectural occlusion. Add restrained bloom around bright glass; do not
blur the entire facade or brighten every prop equally.

**Dependencies:** Material normal/roughness maps, textured wood, emissive lamp
glass, local shadows, foliage coverage, and controlled exposure.

**Pass/fail:** Pass if blue plaster, warm lamp pools, door grain, and dark produce
contours coexist. Fail if lantern halos penetrate walls or the foreground
becomes an undifferentiated black rectangle.

**Source:** [Fab gallery position 13](https://media.fab.com/image_previews/gallery_images/f905a69b-718f-4068-abaf-904b03302c9f/eda413eb-f604-458b-9af9-593c03218c7b.jpg),
`ref-13.jpg`; SHA-256 `d3c6ffae95bb2af8086577911cfdf4f72b2599048880491be95c4d2f868bf6a8`.

## MT-14: Night courtyard with a golden fountain

**Visible:** A broad courtyard frames a circular fountain in the foreground.
Its stone rim is warm gold, while bright water arcs contrast with surrounding
dark pavement. A cyan-blue, green-timbered corner house occupies the middle
and right; the cafe awning, menu board, tables, and a street lantern anchor
the left. Roof overhangs and shutters make strong, shaped shadows. Warm
doorway lights punctuate the back wall. Soft haze is localized around the
lit courtyard and fountain rather than covering every surface uniformly.

**Reconstruct:** Establish the fountain-to-house scale and sightlines, then
balance cool facade lighting against warm practical sources. Animate narrow
water jets, receiving water, highlights, and small splashes. Add drifting mist
only after moving water and contact lighting work.

**Dependencies:** Directional and local shadows, rough stone shading, warm
indirect fill or an explicit approximation, transparent water, emissive lamps,
and stable edge antialiasing.

**Pass/fail:** Pass if fountain arcs stay distinct and the blue house remains
readable. Fail if water becomes an opaque white tube, a glowing basin, or
unstable sparkling pixels during motion.

**Source:** [Fab gallery position 14](https://media.fab.com/image_previews/gallery_images/23462732-cd02-4462-b8bb-c3465cf26a7c/d633dddf-e755-4471-b236-81d9130b2e7a.jpg),
`ref-14.jpg`; SHA-256 `b1d515c165fb6df3debec4ea35c82bca5679b56e9b548b3d7fb7d91a783f46c9`.

## MT-15: Tavern square under a cool night sky

**Visible:** A dark hanging tavern sign crops the upper left, a tree divides
the center, and an elevated blue-lit gable dominates the right. The low,
upward-looking composition includes stars and a patch of blue cloud between
roofs. Warm lights remain concentrated around doors, cafe seating, and the
distant fountain. The tree, cart, roof edges, and foreground pavement form
deep silhouettes. The image depends on separating architectural planes and
framing shapes, rather than uniformly exposing all the town's detail.

**Reconstruct:** Solve the upward camera angle and tree canopy first. Position
the cool directional light so the upper gable receives it while nearby
overhangs and branches occlude it. Add warm practical pools at pedestrian
height, retaining useful local structure. Use the sky as a silhouette background;
this view does not need heavy fog.

**Dependencies:** Coherent day/night environment settings, foliage shadows,
lamp placement, sky/cloud layers, exposure, and black-level control.

**Pass/fail:** Pass if the blue gable, central tree, sign, and warm destinations
read separately. Fail if ambient fill flattens those planes.

**Source:** [Fab gallery position 15](https://media.fab.com/image_previews/gallery_images/d84ed380-c954-42d6-b3d7-721a3115f7b8/586543fb-981d-4ccb-8aa8-4ec662f8a452.jpg),
`ref-15.jpg`; SHA-256 `b9cfc410ba801e043486d46f200cd936d312eac85c1703963da2791ebad8760e`.

## MT-16: Daylight cafe, ivy, and cropped fountain

**Visible:** The fountain is cropped into the lower left, with cafe tables,
chairs, a red awning, and a menu board across the center and right. Ivy
hangs from timbered plaster facades; bunting crosses the bright sky gap.
Strong daylight produces sharp eave, flag, and foliage shadows. Pale plaster
and stone are bright but retain surface variation. The open doorway and
right-hand passage are substantially darker. Green-painted window frames, worn brown beams, flower clusters, and the cloth awning provide distinct
material and color groups.

**Reconstruct:** Reuse courtyard geometry and materials for daylight, matching
this camera. Establish sun direction from facade
and awning shadows, then tune sky fill and exposure. Preserve dark interiors
without crushing all leaves or making sunlit plaster uniformly white. Match
foliage density and layered chairs, tables, and fountain crop.

**Dependencies:** Stable shadow maps, correct texture color space, material
roughness/normal detail, foliage edge coverage, and small-scale contact shading.

**Pass/fail:** Pass if shadow boundaries explain form. Fail if ivy looks
like solid cards or material detail disappears under broad ambient lighting.

**Source:** [Fab gallery position 16](https://media.fab.com/image_previews/gallery_images/60a39ac2-05f2-4d51-b630-52d6b73fdc40/d42088b3-83bc-47b5-bb20-2afa2258815a.jpg),
`ref-16.jpg`; SHA-256 `0b085202510468588cbdec16070a95eea401c9f03e7914125e373aa3e3376cc6`.

## MT-17: Distant castle above layered town roofs

**Visible:** A tall central castle tower and two lower turrets rise behind
town roofs. The castle is pale and low contrast against a large, cloud-filled
sky, while foreground timber gables and orange tiles remain sharper and
darker. Bunting cuts diagonally across the upper left. Flags and overlapping
roofs reinforce depth. Haze is strongest
through the intervening town and around the castle; it does not erase the
near roof silhouettes. Castle shapes remain recognizable despite their muted
surface detail.

**Reconstruct:** Stage three or more depth layers before adding atmosphere:
near roofs, middle town, and the castle. Match skyline heights and perspective.
Tune distance extinction and height fog with sky brightness so the castle
recedes naturally. Layered sky geometry/textures can establish cloud masses;
volumetric clouds are an unproven option.

**Dependencies:** Depth-aware atmosphere, sky/cloud composition, coherent
exposure, sufficient far-clip precision, and stable thin geometry.

**Pass/fail:** Pass if near-to-far contrast decreases convincingly. Fail if
uniform white fog washes over bunting and roofs or hides the castle outline.

**Source:** [Fab gallery position 17](https://media.fab.com/image_previews/gallery_images/e97dfd9a-58dc-42f8-8e68-6f3b285b217a/0fecd2e4-c31b-4dbd-89be-ce25a24c7f56.jpg),
`ref-17.jpg`; SHA-256 `bbde9a1354a8e33a41612ab1526c16061bf1e008cb99fb57bf2937af5e6ef056`.

## MT-18: Hero townhouse with mountain backdrop

**Visible:** A large timbered townhouse fills the center, with a circular
gable window, projecting flower boxes, brick chimneys, and an orange conical
turret to the right. Bright plaster alternates with textured brown framing.
Orange tiled roofs continue through nearby houses. Pale, striated mountains
span the background beneath blue sky and rounded clouds. Dense, dark foreground
leaves frame both sides and the bottom; the near building stays substantially
more detailed and saturated than the distant terrain. Banners and flowers
enrich the facade.

**Reconstruct:** Match house proportions, roof pitch, chimney spacing, and
foreground foliage crop before polishing materials. Establish the mountain
mass and atmospheric separation as actual background depth. Tune sun/sky light
and preserve tile repetition, beam grain, rough plaster, and leaf silhouettes.
Surface detail supports the composition.

**Dependencies:** Qualified PBR materials, directional shadows, distant terrain,
sky/clouds, foliage shading and coverage, mipmaps, and motion-stable antialiasing.

**Pass/fail:** Pass if the facade remains crisp against a softer mountain
layer. Fail if foliage shimmers, flowers become flat glow, or the backdrop
competes with the house at identical contrast.

**Source:** [Fab gallery position 18](https://media.fab.com/image_previews/gallery_images/380e94c4-6e33-4796-8d95-927530a0c471/c519bddc-d8bc-437d-aef2-f2ab0d58111a.jpg),
`ref-18.jpg`; SHA-256 `6c1741efb1d05a23702d7701b9ee6b3569232afdf44616c80786542d1950d461`.
