# Private assets

Status: implemented on desktop, October 6, 2026 (#10769, #10770, #10771,
#10772).

Verse can draw licensed 3D assets that this public repository must never
hold. A licensed asset, such as a Fab listing under the standard license,
may be used in a product, but its raw files may not be redistributed. Every
other piece of Verse content is committed under `assets/verse/` and served
from public URLs, so a licensed asset needs its own path: a private registry
on the owner's Google Cloud project, a conversion command that uploads only
our compiled pack, a broker that hands a short-lived download link to the
keys a manifest names, and placements that live on the owner's computer
rather than in code.

The [GCP resources](../deployment/verse-private-assets.md) record what runs
and how to operate it.

## Rules

- Never commit a vendor file, converted geometry, a texture, or a pack of a
  licensed asset. Local builds go under `~/.openagents/verse/private-build/`,
  outside the repository; `.gitignore` also ignores `/private-assets/` and
  `*.private.vtp` in case a build is pointed inside a checkout.
- Committed code holds no reference to a private asset: no name, digest,
  length, or position. Placements live in owner-local configuration
  (`~/.openagents/verse/private-assets.json`).
- A private pack never enters the Everglade or Grid packs, and the web build
  has no private path. The Everglade web image copies only
  `assets/verse/everglade/*.vtp`.
- No API key or service-account key exists anywhere. The broker runs on
  Cloud Run under its own service account and signs through the IAM
  Credentials API with Google-managed keys. The upload command uses the
  owner's own `gcloud` login.

## The registry

One private bucket, `openagentsgemini-verse-private-assets` in us-central1,
with uniform bucket-level access, public access prevention enforced, object
versioning, and no public IAM binding. Objects:

| Object | What it holds | Who reads it |
| --- | --- | --- |
| `packs/<sha256>.vtp` | The compiled pack, named by its content digest. | The broker signs a URL for it. |
| `manifests/<name>.json` | The asset's manifest. | The broker, and the owner. |
| `vendor/<name>/<path>` | The raw vendor files, archived for reproducibility. | The owner only; never served. |

The broker signs URLs only under `packs/`, so the vendor archive and the
manifests are never reachable through it.

### The manifest

`openagents.verse.private-asset.v1`, checked by
[`verse_private::manifest`](../../crates/verse-private/src/manifest.rs):

| Field | Meaning |
| --- | --- |
| `name` | The asset's registry name: lowercase letters, digits, and `-`. |
| `title` | The listing's title. |
| `kind` | `character` today. |
| `source` | Where it came from: marketplace (`fab`), listing ID and URL, seller, and whether the listing says it is AI-generated. |
| `license` | The license ID (`fab-standard`) and a summary of its terms. |
| `readers` | The Nostr public keys (hex) that may load the pack. |
| `pack` | The pack's SHA-256, length, format (`VTP3`), its form names, triangles per level, texture edge, height, rig, and clips. |
| `vendor` | The archive prefix and the SHA-256 of every raw file. |
| `provenance` | The conversion script and its SHA-256, the Blender version, the repository commit, the parameters, and when it was compiled. |

The broker refuses a manifest whose name does not match its object, whose
pack is not under `packs/`, or whose digest is malformed.

## Conversion

`verse-private add` converts a vendor model into a private pack and uploads
it:

```sh
cargo run --release -p verse-zone-everglade --bin verse-private -- add \
  "/Users/Shared/UnrealEngine/Launcher/VaultCache/FabLibrary/<Listing>/glb" \
  --name NAME --license fab-standard --reader <hex pubkey>
```

It runs these steps:

1. Reads the Fab `metadata` file beside the model for the title, listing,
   seller, and AI-generated flag, and finds the one `.glb`, `.gltf`,
   `.fbx`, or `.blend` under the folder.
2. Runs `scripts/blender/private_character.py` headless. The script
   imports the model, joins its meshes, welds the vertices an exporter
   split along UV seams, stands it up (the longest axis becomes up, or
   `--up`) and scales it to the requested height with its feet at the
   origin. It then makes a near level (20,000 triangles, 1024 px) and a far
   level (5,000 triangles, 512 px): each is decimated from the full mesh,
   unwrapped afresh, and has the full mesh's base color baked into one
   image, because collapsing a dense AI mesh smears its own fragmented UV
   seams. Normal, roughness, and metallic maps are dropped. It adds a rig
   (below) and writes glTF with separate PNG textures and front, side, and
   face previews. The first asset took 12 seconds.
3. Compiles both levels with the Everglade pack compiler's private entry
   ([`compile::private`](../../crates/verse-zone-everglade/src/zones/everglade_pack/compile/private.rs))
   into a `VTP3` pack whose forms are `private/guest` and
   `private/guest_far`, under the private limits (`Limits::PRIVATE`: 8 MiB
   transfer, 1024 px textures, 16 MiB decoded, 30,000 triangles in all and 24,000 in a level).
4. Uploads the raw vendor files under `vendor/<name>/`, the pack under
   `packs/<sha256>.vtp`, and then the manifest, with `gcloud storage cp`
   as the owner. The manifest goes last, so a reader never sees a manifest
   whose pack is missing.

`verse-private list`, `show NAME`, `grant NAME KEY`, `revoke NAME KEY`, and
`remove NAME` manage the registry. `place NAME --at X,Z --yaw RADIANS`
writes the owner-local placement, and `place NAME --seat SEAT` seats it
instead (see [Seats](#seats)).

### Rigs

Most licensed characters from AI generators, like the first one, are one
fused mesh with no skeleton. A full humanoid rig with walking clips needs an
auto-rigger (Mixamo, AccuRIG, or Rigify with hand-placed markers) and a
person to check the deformation, which is future work. The pipeline does
what is safe without one: it adds a five-joint spine (root, hips, spine,
chest, and head) and weights every vertex by its height along the body with
smooth falloff, then keys a four-second breathing idle: the chest rises and
widens a little, the hips shift weight from side to side, and the head turns
slowly. Arms ride the chest. The result is an idle standing NPC; it can't
walk or gesture. A source that already has a rig, such as the Fab library's
rigged and animated fantasy female, is applied in its rest pose and gets the
same spine; keeping a source's own skeleton and clips is follow-up work.

`add --pose seated` converts a character sitting down. Before the levels are
made, the script bends the standing body: the shins swing back at the knees
and the legs swing forward at the hips, each vertex turned by an angle that
eases in across the joint, so the thighs lie level, the shins hang, and the
hands that hung beside the thighs rest on the lap. The spine's joints move
down with the body, and `idle` becomes an eight-second seated idle: two
breaths, the head bowed a little toward a desk, and once a cycle a glance up
and a little aside. The report records `seat_m`, the height of the seat
under the body (0.395 m for a 1.62 m body), which the chair it sits on
matches.

### Overlays

`add --pose seated --overlay jacket,heels` dresses a seated character in
garments of our own, built by
[`scripts/blender/outfit.py`](../../scripts/blender/outfit.py) around the
body before the levels are made. The garment code is committed; the dressed
body and its pack stay private like any other build.

- `jacket`: a tailored black jacket. The script lofts a torso shell and two
  sleeves over convex slices of the body, without the hair, joins them with
  a voxel remesh, relaxes the surface, and pushes it back off the body. It
  cuts the hem, the cuffs, the neck, and a single-breasted front that opens
  in a V above the waist button, then adds notched lapels and a collar and
  gives the edges a turned thickness. Hair the jacket would pass through is
  lifted onto it by a smooth field over the angle around the neck and the
  height, so curls keep their shape and the hair's layers never cross.
- `heels`: black pumps fitted to the feet as the seated pose leaves them,
  pointed with the heels lifted: an upper lofted over the foot's sections,
  an almond toe, a throat cut low over the instep, and a tapered heel to the
  floor. The soles raise the body 4 mm, which `seat_m` includes.

Skin and dress the garments cover from every side are removed, so the levels
spend their triangles on what shows. Each level then bakes the body's faces
only from the body and the garments' faces only from the garments, so a
jacket a centimeter over the dress never paints the dress. Only base color
reaches the pack, so the cloth's folds are baked into it from ambient
occlusion and the pumps' gloss as a soft highlight. The garments' islands get
0.75 and the head's 1.45 times the linear texture density of the rest. The
provenance records `overlay` and `outfit.py`'s digest.

## Access

The broker is `verse-assets`, a small Cloud Run service built from
[`crates/verse-private`](../../crates/verse-private/README.md):

1. Verse sends `POST /v1/private/url` with the body
   `{"name":"NAME","sha256":"DIGEST"}` and a NIP-98 `Authorization: Nostr
   ...` header: a kind 27235 event signed by the player's Verse key over the
   exact URL, the method, and the body's SHA-256.
2. The broker checks the event's signature, kind, URL, method, and payload
   digest, that it was signed within the last 60 seconds, and that its ID
   hasn't been used before.
3. It reads `manifests/NAME.json` with its own service account, checks the
   signer is one of the manifest's `readers` and the digest is the
   manifest's pack, and returns a V4 signed URL for `packs/DIGEST.vtp` that
   expires in 300 seconds, with the pack's length.
4. Verse fetches the URL under the pinned loader's rules (HTTPS only, no
   redirects, exact length and digest) and caches the pack in
   `~/.openagents/verse/private-cache/` (directory 0700, file 0600, named by
   digest).

Every refusal of a missing, stale, replayed, or unlisted signature, and of an unknown asset or digest, is the same `403` with no detail, so a stranger learns nothing
about which assets exist.

### Why this option

- **It uses an existing identity.** Each Verse install already has a Nostr
  key (`~/.openagents/verse/<profile>.key`, or the platform keychain on a
  phone). The manifest's `readers` list is the owner's keys, so no new
  credential is issued and no secret travels.
- **The client needs no Google credential.** The owner's paired phone can use
  the same path with its own Verse key. A `gcloud` login on the client, the
  other option, works only on a computer that has the Cloud SDK and expires
  in an hour.
- **The service is separate from openagents.com.** The website's service
  account never gets read access to licensed content, and a mistake in the
  broker can't change the public site.
- **Least privilege.** The broker's service account holds only
  `roles/storage.objectViewer` on this bucket and
  `roles/iam.serviceAccountTokenCreator` on itself (to sign). It never
  handles pack bytes: Cloud Storage serves them.

### The web build and other users

The web build compiles no private loader, and no committed file names a
private asset, so it has nothing to load. A client whose key is not a reader
gets `403` from the broker and draws nothing. The bucket is not public, so a
direct object URL returns `403` too.

## In Verse

On desktop, `~/.openagents/verse/private-assets.json` (mode 0600) names
the broker, the Verse profile whose key signs, and the placements:

```json
{
  "schema": "openagents.verse.private-placements.v1",
  "broker": "https://verse-assets-<hash>.us-central1.run.app",
  "profile": "default",
  "placements": [
    {
      "asset": "NAME",
      "sha256": "<pack digest>",
      "bytes": 1234567,
      "zone": "everglade",
      "at": [105.5, -31.2],
      "yaw": -1.571,
      "scale": 1.0
    }
  ]
}
```

`verse-private place` writes it. On Everglade entry, Verse starts a
background load of each placement; the town is drawn without waiting. When a
pack arrives, its character stands at its place as an idle NPC, drawn in the
frame's figure like the town's wildlife and lit by the baked probes. It
draws the near level within 18 m and the far level to 60 m, and not at all
beyond. When the file is missing, the key is not a reader, or the network
is down, nothing is drawn and the zone is unchanged; Verse logs one line.

The first asset sits at the reception in the owner's house, converted
seated. To see her, run desktop Verse, walk from Everglade's spawn east
along Library Way to the house, and go in through the bronze doors; she sits
to the left of the entry walk, facing the door. For an offline look, the capture example loads the placements the
same way:

```sh
VERSE_CAPTURE_PRIVATE=~/.openagents/verse \
  cargo run -p verse --features capture --example everglade_capture -- \
  out.png at:104.2,-33.4,0.15,-8
```

### Seats

A placement may name a seat instead of a place: `"seat": "reception"`,
with no `at` or `yaw`. The zone gives the seat's place, floor, and facing,
so a placement inside a building needs no coordinates. Everglade has one
seat today, the reception chair in the owner's house: west of the entry
walk a few strides in from the door, turned toward the doorway, with a
walnut reception desk before it (`layout::estate::RECEPTION`). The pack
must be converted with `--pose seated`. A seated character adds no block of
its own, since the chair and the desk block, and each seat takes one
character.

```json
{
  "asset": "NAME",
  "sha256": "<pack digest>",
  "bytes": 1234567,
  "zone": "everglade",
  "scale": 1.0,
  "seat": "reception"
}
```

### Phones

The phone builds share the loader, but the iOS and Android hosts don't yet
pass a placements file to the world runtime. The broker accepts a phone's
Verse key once it is listed as a reader (`verse-private grant`). Wiring the
phone's placements is follow-up work (#10797).

## Operations

- **Add an asset:** `verse-private add ...`, then `verse-private place ...`.
- **Remove an asset:** `verse-private remove NAME` deletes its manifest,
  pack, and vendor archive. Object versioning keeps noncurrent copies for 30
  days; delete them sooner with `gcloud storage rm --all-versions`.
- **Revoke a reader:** `verse-private revoke NAME KEY`. The broker reads the
  manifest on every request, so the next request is refused; a URL already
  signed stays valid for at most 300 seconds. A cached copy on that device
  stays until deleted.
- **Shut everything off:** remove the broker's bucket binding, or delete the
  Cloud Run service.
- **Rotate:** nothing to rotate by hand. The signing keys are Google-managed
  service-account keys that Google rotates; no user-managed key exists. To
  rotate a reader's Verse key, grant the new key and revoke the old one.
- **Costs:** a pack is about 2 to 5 MB. Standard storage in us-central1 is
  about $0.02 per GB-month, so ten assets with their vendor archives (about
  1 GB) cost about $0.02 a month. Egress to the internet is about $0.12 per
  GB, so a few hundred loads a month cost under $0.20, and the content
  cache means a computer downloads a pack once. The broker scales to zero.
