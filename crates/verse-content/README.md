# Verse content

This crate owns portable compiled-content identity, outfit/equipment admission,
and original furnishing collision admission. The dedicated host and Verse
clients use the same checks. Default features include no renderer, font library,
private reader, or agent.

The `compiler` feature adds Rust procedural assets, spell geometry, licensed
character/prop import, SVG rasterization, inventory provenance, and original
world recipes. The `verse-content` command compiles `ritual` or `observatory`
into a new directory. See the [dedicated host instructions](../verse-host/README.md).

The compiler retains the existing original source-to-world coordinate basis.
Collision admission for furnishings retains the original model whitelist;
authored social geometry uses the validated `verse-world` profile contract.
This boundary does not define arbitrary scripts or a general world editor.

The `authoring` feature adds a local workbench over compiled content. It uses the
runtime's scene, animation, material, collision, character, and campaign checks.
The `compiler` feature includes it and exposes `verse-content author`:

```sh
verse-content ritual /tmp/ritual
verse-content author init /tmp/ritual /tmp/outpost chamber-outpost
verse-content author inspect /tmp/outpost
verse-content author apply /tmp/outpost assets/verse/authoring/chamber-outpost.transaction.json
verse-content author preview /tmp/outpost 1 24
verse-content author build /tmp/outpost
verse-content author undo /tmp/outpost 2
verse-content author redo /tmp/outpost 3
```

The sample adds a friendly quest giver, changes a guardian's health, supplies its
quest objective reward, builds and places a barricade mesh with matching
collision, and edits dialogue timing. No renderer
change is required. `preview` advances an isolated authority, validates its
`RenderWorld`, and writes `preview.svg` with actor positions, placements, actual
collision geometry, optional compiled capsule-clearance navigation, and a cue
timeline. Navigation uses the runtime compiler at one-meter resolution within a
2–64 meter half extent, with a one-million work-unit limit. Multiple spans can
occupy the same ground position. A preview does not imply a frame-time target.

`author edit WORKSPACE` keeps a Rust editor open and processes one JSON command
per line. It returns one JSON result or a source/field diagnostic per command.
Supported actions are `inspect`, `transaction`, `undo`, `redo`, `preview`, and
`build`. For example, after initialization:

```json
{"action":"transaction","transaction":{"expected_revision":1,"label":"Rename zone","edits":[{"op":"zone","name":"outpost"}]}}
```

Transactions can change scene properties, add or remove actors, placements, and
stable-ID cues, create static box meshes, map existing animation clips, edit
material channels, tune the
primary character's existing ability catalog, add collision boxes, edit social profiles, and change
quest, reward, item, outfit, and equipment catalogs. Use `inspect` to get the
current document, model keys, surface indices, clip IDs, sockets, texture slots,
retained asset identities, and undo counts. IDs stay stable when an author moves
an actor, reorders cues, or undoes an edit. Cue playback sorts by time and ID.
Primitive keys name static box meshes, with local bounds in meters, a retained
texture slot, and tint channels. Place them with a stable placement ID; pack
placement transforms retain source coordinates. Scene actor positions use world
meters. Set matching collision boxes in `authored.blockers`. When a combat scene
has no named collision profile, supply `authored.navigation` with `min`, `max`,
and `cell` to compile walkable geometry from those boxes. It accepts 0.5–2 meter
cells, up to 16,384 ground cells, and one million work units. Navigation rebuilds
from the same authored source after checkpoint recovery or encounter reset.
Author social terrain and interaction objects in `social_profile`; custom combat
character tuning and blocker settings cannot be combined with a social profile.
New primitive meshes retain a local author's source declaration and stable model
ID. Their default `owner_supplied_local` license makes no redistribution claim.

A quest giver must be a friendly NPC, and every authored quest objective needs
a reward source in this scene. Quest prerequisites retain the runtime's sorted
ID order. Authoring changes data over the implemented rules; a new ability
implementation, rig importer, or shader still requires engine development.

A transaction supplies the expected journal revision and 1–64 edits, which are
admitted together. Failed admission leaves the journal, undo history, published
generation, and local preview intact. Undo and redo persist across editor
restarts, advance the revision, and retain up to 32 documents within a 16 MiB
journal. Each document and command is limited to 2 MiB. A new edit clears redo.
An OS file lock excludes concurrent writers and releases on process exit.
The editor limits an input session to 4,096 commands. Inputs and destinations
must be regular files and directories; symbolic links are refused.

Builds reuse the snapshotted compiled meshes and images; they do not rerun the
Rust asset recipes. The document, source bundle, and compiled content identity select an immutable
`generations/DIGEST` directory. A repeated build verifies and reuses it. Files,
including the document, pack, scene, preview, and host template, have sealed
hashes. Changed model mappings and materials retain source declarations and
asset IDs and receive new model fingerprints. Source snapshots are copied once;
texture contents remain digest-checked.

Builds persist the runtime's color, masked-alpha, normal, and scalar mip recipes
in `mips.json` and `mips.rgba`. Existing authored mip variants remain intact;
changed materials reuse matching variants and cook the missing ones. Archives
require every material variant, a contiguous full-resolution-to-one-texel chain,
matching source image hashes, and a sealed payload. Metadata is limited to 8 MiB
and RGBA data to 256 MiB. The loader counts retained pixels and archived levels
against its texture memory budget. Content identity includes all mip bytes.
The physical renderer uploads the verified levels directly and retains them
for device recreation. Portable byte loading follows the same admission path.
These archives use RGBA8; compressed GPU formats require a separate cook target.

The `current.json` pointer changes atomically after
all files and their parent directories are synced and admitted. An interrupted
`generations/building` directory remains unadmitted; inspect and remove it
before retrying. Generation cleanup is an explicit operator action.

The build's `host-template.json` loads through the dedicated host's existing
configuration path. Copy it outside the immutable generation, then set the
copy's instance, TLS paths, enrollment or guest policy, and storage directory
before launching it. Its `authored` settings select the
generic authored combat timeline, include collision and ability tuning, and
bind gameplay catalogs into content identity. The default ritual profile keeps
its scripted behavior when `authored` is absent. A changed durable generation
still requires the host's reviewed migration path. This workbench provides
command-based editing and standalone SVG diagnostics; it does not install a
windowed 3D editor or grant remote editing rights.
