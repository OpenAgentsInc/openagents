# Verse Haunted House documents

`openagents.metareal.haunt.v1alpha1`, selected by the `haunt` descriptor
profile, defines the native scene and its editable documents. This is a new
alpha namespace. No published engine, Nostr, HOST or REACH contract changes.

`Resource` names a world revision, rigid meshes, instances, walkable areas,
lights, people and zones/actions. Geometry is in two separate immutable bundle
resources: 48-byte rigid vertices and little-endian uint32 triangle indices.
The descriptor gives their exact counts. The bounded generated protobuf
reader never needs to ingest multi-megabyte GPU blobs as a protobuf message.

`Save` is a complete snapshot of editable overrides identified by stable
entity IDs; static structure and seeded trees come from the exact matching
base revision. People retain their dialogue, taught knowledge and behaviours.
Zone shape/once edits, scare bits, quest progress, personal-copy identity and
feedback travel with it. Sharing is an actual `.openagents-world.pb` file.
Reset/undo restores the base before applying overrides, so absent additions
are removed. Strings borrow retained input memory, not temporary host strings.

Limits: resource/save at most 262,144 bytes in this consumer, 256 meshes,
4,096 scene instances, 64 areas, 256 walk rectangles, 63 world lights,
64 people, 32 zones, 256 actions/events, 512 editable overrides; names/IDs
128 UTF-8 bytes, taught knowledge/feedback 4,096 bytes. Unsupported versions,
wrong base revisions, duplicate override IDs, non-finite transforms, invalid
mesh references and oversized input fail before the current world changes.
The native host admits GPU spans through the checked Coax buffer gate, then
checks every draw range against the actual index allocation. No decoded
record proves ownership or authorizes a public-world publish or payment.

The retired web account/public-world publication/paid-change-offer service and
cloud NPC endpoint are not restored by this scene port. Local saving, world
copies, file invitations/import, and deterministic character conversation
work without those services; these must not be described as authenticated
public publication or cloud-generated answers.

Checks: `verse/test_haunt.sh` under ASan/UBSan; set `PROTOC` and
`PROTO_PYTHON` to also run `haunt_oracle.py` against official protoc and the
original content. Native UI tests enter from the Grid, converse, edit, save,
leave/reenter, rotate and open the map. The normal iOS build remains Python
and protoc free.
