# Fantasy Props catalog

[#12548](https://github.com/OpenAgentsInc/openagents/issues/12548) prepares the
actual 94-model Fantasy Props Standard download. The
[metadata record](fantasy-props-catalog.json) contains stable IDs, source and
output hashes, resolved references, bounds, material-slot counts, collision
policy, and the selected runtime subset. The public Rust adapter validates this
record without engine source, credentials, meshes, or textures.

Quaternius assets use CC0-1.0. The reviewed license file has SHA-256
`edad12240087a33e08fc031e4e66c2b2b4b2a6d4f086339bde04f741b385fbda`.
OpenAgents original custom art uses project runtime distribution terms; it has
no CC0 dedication. The six Modular Medieval Town modules retain their separate
licensed-runtime restrictions. None of those binary assets enter public Git.

The review subset has 24 kit props, five original spooky models, and six town
modules. Shared trim textures are prepared once by content and role at up to
512 pixels. Base color is sRGB; normals and packed data are linear. Source
occlusion-red, roughness-green, metallic-blue becomes engine metallic-red,
roughness-green, occlusion-blue. Normals use the glTF OpenGL variant. UVs,
normalized vertex colors, transforms, material slots, and double-sided flags
survive import. Chest_Wood uses the baked **Chest_Closed** pose, without runtime
animation. Only the selected scene subset enters the startup payload.

The custom werewolf, ghost, pumpkin, coffin, and gravestone fill gaps in this
download. They use broad low-poly silhouettes, restrained painted detail, muted
warm fur and wood, worn stone, and rough surfaces. The werewolf is 2.25 meters
tall, with a separate `werewolf.face` material and UV target for localized scars.
It is original OpenAgents art. Instance collision must be explicit; a town
module does not imply an editable or traversable interior.

Private preparation publishes one accepted pointer after validating complete
outputs. Rejected inputs leave the previous catalog selected. The record names
the compiler and prepared manifests by hash for reproducible rebuilds from the
preserved inputs. Limits are 40 scene assets, 500,000 vertices, two million
indices, 32 MiB of geometry, 32 MiB of transfer, and 32 MiB of decoded textures.
The current subset has 37,936 vertices and about 9.3 MiB of prepared resources.
Private browser captures include an in-scene contact sheet and individual views
under shared lighting. This is asset-preparation evidence; the walkable house,
material tools, and complete tutorial have their own acceptance issues.
