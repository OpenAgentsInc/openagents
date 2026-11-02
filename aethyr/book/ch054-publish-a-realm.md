# 54. Publish a realm

> **Status: partly implemented.** Visitors of a made realm see the revision
> its owner published, played by the engine build and assets it was
> published with: the publish record pins the document's hash and the
> engine's manifest (host script, wasm, shaders, voice engine, render
> package and asset packs, each named by its content, and each served file's
> own SHA-256) and its hash
> ([GitHub #12629](https://github.com/OpenAgentsInc/openagents/issues/12629)).
> The whole route (edit, publish, reload from the record, a later release,
> fork, edits kept apart, refusals) is one checked walkthrough below.
> Not built yet: the release gates per version (golden, 60 FPS) and the page
> text's hash in the record. Hand-built realms are published from one table
> (`apps/openagents-web/realms.json`) and are not pinned this way.

Publishing turns a realm someone is working on into one other people visit.
That needs three promises: what visitors get is a fixed version (a document,
a module build and its assets, each by hash); the version passed the same
checks every realm passes (it draws, it holds 60 FPS, it matches its golden);
and a release of the engine never overwrites what someone made.

## What this chapter teaches

- A realm as four things: its world on the galaxy, its module, its assets,
  its save slot.
- A publish record: document hash, module build identity, asset hashes,
  golden and page text.
- Release gates per realm, compared against production first, so a release
  is stopped only by a regression it causes.
- Makers: who a realm belongs to and where its tips go.

## The method

A publish record is the unit a release reasons about:

```jai
Published :: struct {
    slug: Slug; version: u32;
    doc_sha256: Hash;            // chapter 51's document (made realms)
    module: BuildId;             // the module's wasm, by hash
    assets: [MAX_ASSETS] Hash;   // chapter 53's packs and files
    golden: GoldenRule;          // lit share, settle time, blocks allowed to differ
    page_md_sha256: Hash;        // the page as text, for agents and readers
}
```

Hand-built realms are published by a reviewed change to the table and their
module; made realms and forks by their owner, with no deploy. Both end as the
same record. A new version is checked before it replaces the old: the page
draws (enough lit pixels after it settles), its shot matches its golden
within the allowed blocks, and its frame times hold the 60 FPS rule. A made
realm lives only in the bucket, so the release copies engine code, never
another environment's realms.

## What exists today

- [docs/verse/realm-host.md](../../docs/verse/realm-host.md): the table
  entry (`id`, `kind`, `orbit`, `realm.slug`, `module`, `slot`, `assets`,
  `golden`, `style`, `clock`), the required hand-written `index.md`,
  `bun coax/web/realms.ts --check` and `coax/web/realms.test.ts`.
- [coax/web/realms.ts](../../coax/web/realms.ts) writes
  `apps/openagents-web/realms.coax` (the galaxy's table),
  `realms/<slug>/index.html` and `/realms`' list.
- Made realms: `POST /api/playtest/realms` writes `realms/<slug>`,
  `index.html`, `index.md` and `realms/made-slugs.txt` into the bucket
  (`drops_site_write`); `upload.py` never removes a listed one
  ([realm-builder.md](../../docs/verse/realm-builder.md), "Releases").
- A made realm's owner publishes a revision:
  `POST /api/playtest/realms/<slug>/publish {"rev": N, "listed": true|false}`.
  The server refuses a revision that is not the current one with `409` and
  the current revision, records `published_rev`, `published_sha256` and the
  time, and keeps the revision in `verse.realm_revisions`
  ([0093_realm_documents.sql](../../coax/web/api/migrations/0093_realm_documents.sql)).
  `listed: false` keeps the page `noindex` and off `/og`'s rows. In the
  realm, PUBLISH is shown to the owner only when the saved revision is not
  yet the published one.
- Visitors get the revision published
  ([0094_realm_live_and_undo.sql](../../coax/web/api/migrations/0094_realm_live_and_undo.sql)).
  Making or forking a realm publishes its first revision. The spec, `/doc`
  and a fork by anyone but the owner are of the published revision; the
  owner and site admins get the latest. Under the realm's name the owner
  reads "LIVE: VERSION 3", "LIVE: VERSION 3. NOT PUBLISHED: 5" (saved, not
  published) or "LIVE: VERSION 3. NOT SAVED".
- Realms made or forked by a tool (the `verse-automation` cookie or the
  `X-Verse-Automation` header) are unlisted from the start.
- Forks stay `noindex` until their owner publishes
  ([realm-fork.md](../../docs/verse/realm-fork.md)).
- Goldens in [coax/web/goldens/](../../coax/web/goldens/everglade.png),
  [golden-check.sh](../../coax/web/golden-check.sh) naming the commit that
  changed a page, and [perf.ts](../../coax/web/perf.ts) for frame times.
- Makers and tips ([makers.md](../../docs/verse/makers.md)): a one-time
  claim code, a Lightning address, "MADE BY NAME · TIP" drawn in Coax.
- An agent publishes a hand-built realm by pull request
  ([verse/SKILL.md](../../verse/SKILL.md)); the template is
  [verse/realm-template/](../../verse/realm-template/index.md).

The made-realm write and the hand-built table are separate code paths with
separate checks.

### The engine a made realm is published with

A made realm's page used to fetch `/realms/built` on every visit and copy its
app script, so a release changed every published realm the moment it
landed. That script is the engine: every file it names is named by its
content (`coax/web/deploy.sh`) and never deleted from the site's bucket. So
publishing keeps the script itself, as an engine manifest, one attribute a
line in the page's order:

<!-- source: aethyr/forge/publish.coax engine -->
```jai
ENGINE_HEADER :: "aethyr-engine 1\n";
ENGINE_TEXT :: 4096;      // a manifest's bytes at most (the server's limit too)
ENGINE_ATTRS :: 24;       // attributes at most
ENGINE_NAME :: 32;        // an attribute name's letters at most
ENGINE_PATH :: 200;       // a path's characters at most
ENGINE_HASH :: 12;        // hex digits in a content name: deploy.sh's sha256 prefix
ENGINE_APP :: "myrealm";  // the made-realm engine; nothing else is pinned here
ENGINE_OPEN :: "<script type=\"module\" src=\"";

// A manifest being written into a caller's buffer, and what it has seen.
EngineAcc :: struct {
  n: int; bad: bool;
  names: [ENGINE_ATTRS] string; count: int;
  src: bool; app: bool; wasm: bool; wgsl: bool;
}
```

A value may name only paths named by their content: `NAME.<12 hex>.EXT`
somewhere along it, or a 64-hex directory (a render package's):

<!-- source: aethyr/forge/publish.coax path -->
```jai
// A path the manifest may name: from the site's root, plain characters, no empty or dot-dot
// segments, and named by its content somewhere along it.
engine_path_ok :: (p: string) -> bool {
  return p.count <= ENGINE_PATH && engine_at(p, 0, "/") && engine_segments(p[1..<p.count], false);
}
// The segments after a slash: none empty, "." or "..", plain characters, one of them (or one
// before, `named`) a content name.
engine_segments :: (rest: string, named: bool) -> bool {
  cut := engine_upto(rest, '/');
  seg := rest[0..<cut];
  if seg.count == 0 || seg == "." || seg == ".." || !engine_all(seg, ENGINE_PATH_CHARS) { return false; }
  now := named || engine_named_segment(seg);
  return ifx cut == rest.count then now else engine_segments(rest[cut + 1..<rest.count], now);
}
```

Each attribute goes through one rule, which both reads a page and checks a
manifest read back:

<!-- source: aethyr/forge/publish.coax add -->
```jai
// One attribute into the manifest: its name, then its value with every run of white space one
// space and none at either end, checked once written. A name seen before, more than
// ENGINE_ATTRS, or a value naming anything not named by its content spoils the manifest.
engine_add :: (buf: [] u8, a: [] EngineAcc, name: string, raw: string) {
  if !engine_name_ok(name) || a[0].count == ENGINE_ATTRS { a[0].bad = true; return; }
  for k: 0..<a[0].count { if a[0].names[k] == name { a[0].bad = true; } }
  a[0].names[a[0].count] = name; a[0].count += 1;
  engine_put(buf, a, name); engine_put(buf, a, " ");
  start := a[0].n; gap := false;
  for c: raw {
    if engine_in(c, ENGINE_SPACE) { gap = a[0].n > start; continue; }
    if gap { engine_put(buf, a, " "); gap = false; }
    one := [1] u8.[c]; engine_put(buf, a, cast(string) one[..]);
  }
  if a[0].bad { return; }
  value := cast(string) buf[start..<a[0].n];
  if !engine_value_ok(value) || (name == "data-app" && value != ENGINE_APP) { a[0].bad = true; return; }
  if name == "src" { a[0].src = engine_host_ok(value); }
  if name == "data-app" { a[0].app = true; }
  if name == "data-wasm" { a[0].wasm = engine_path_ok(value); }
  if name == "data-wgsl" { a[0].wgsl = engine_path_ok(value); }
  engine_put(buf, a, "\n");
}
// The manifest, or "" when anything spoiled it, it is over ENGINE_TEXT bytes, or the host script,
// engine, wasm or shaders are missing.
engine_finish :: (buf: [] u8, a: [] EngineAcc) -> string {
  if a[0].bad || a[0].n > ENGINE_TEXT || !a[0].src || !a[0].app || !a[0].wasm || !a[0].wgsl { return ""; }
  return cast(string) buf[0..<a[0].n];
}
```

<!-- source: aethyr/forge/publish.coax manifest -->
```jai
// The made-realm engine's script in a page: what follows "<script " in the one tag that opens like
// the engine's and names data-app="myrealm", to before its ">". "" when there is none, two, or one
// that doesn't end.
engine_tag :: (t: string) -> string {
  found := ""; seen := false;
  for i: 0..<t.count {
    if !engine_at(t, i, ENGINE_OPEN) { continue; }
    end := i + engine_upto(t[i..<t.count], '>');
    if end == t.count { return ""; }
    if engine_has(t[i..<end], " data-app=\"myrealm\"") {
      if seen { return ""; }
      found = t[i + 8..<end]; seen = true;
    }
  }
  return found;
}
// The made-realm engine's manifest from a page (the template /realms/built): every attribute of its
// script in the page's order, NAME="VALUE" each. "" when the page is refused: no such script, an
// attribute malformed or repeated, a value naming anything not named by its content, no host
// script, wasm or shaders, or more than the buffer or ENGINE_TEXT holds.
engine_manifest :: (template: string, buf: [] u8) -> string {
  out := buf;
  a: [1] EngineAcc;
  engine_put(out, a[..], ENGINE_HEADER);
  rest := engine_tag(template);
  while rest.count > 0 {
    if engine_in(rest[0], ENGINE_SPACE) { rest = rest[1..<rest.count]; continue; }
    eq := engine_upto(rest, '=');
    if !engine_at(rest, eq, "=\"") { return ""; }
    value := rest[eq + 2..<rest.count];
    close := engine_upto(value, '"');
    if close == value.count { return ""; }
    engine_add(out, a[..], rest[0..<eq], value[0..<close]);
    rest = value[close + 1..<value.count];
  }
  return engine_finish(out, a[..]);
}
```

<!-- source: aethyr/forge/publish.coax check -->
```jai
// A manifest read back (from the server's publish record) holds when it is exactly what
// engine_manifest writes: written again line by line through the same rule, the same bytes.
engine_check :: (m: string) -> bool {
  if !engine_at(m, 0, ENGINE_HEADER) { return false; }
  buf: [ENGINE_TEXT] u8; a: [1] EngineAcc;
  engine_put(buf[..], a[..], ENGINE_HEADER);
  at := ENGINE_HEADER.count;
  for i: ENGINE_HEADER.count..<m.count {   // each line; text after the last newline is never written again
    if m[i] != '\n' { continue; }
    line := m[at..<i]; at = i + 1;
    sp := engine_upto(line, ' ');
    if sp == line.count { return false; }
    engine_add(buf[..], a[..], line[0..<sp], line[sp + 1..<line.count]);
  }
  return engine_finish(buf[..], a[..]) == m;
}
```

The server keeps the same rule in SQL
([0107_realm_engine_pins.sql](../../coax/web/api/migrations/0107_realm_engine_pins.sql)):
`POST .../<slug>/publish` reads `realms/built/index.html` from the bucket,
and in one transaction publishes the revision and pins the manifest, or,
when the page can't be read or names anything not by content, publishes
nothing and answers `503`. Every publish is a row of
`verse.realm_publications` (revision, document hash, manifest, engine hash,
why), and `GET .../<slug>/published` answers the latest. The spec gains
`engine SHA256`. The realm's page carries the manifest (`data-engine`) and
builds the engine's script from it, so a later release can't change it;
`?engine=current` tries today's engine. A fork starts on its parent's engine
and its own publishes leave its parent's record alone. A realm's first
revision, and every realm published before pins, is pinned to the engine
served when its page is next written.

<!-- source: aethyr/forge/publish.coax record -->
```jai
// A publish record (GET .../<slug>/published) holds together: a revision, the document's 64-hex
// hash, a manifest this rule writes, and the engine hash is that manifest's.
engine_record_ok :: (rev: int, doc_sha: string, engine: string, want: string) -> bool {
  if rev < 1 || doc_sha.count != 64 || !engine_all(doc_sha, ENGINE_HEXES) || !engine_check(engine) { return false; }
  h: [64] u8;
  return engine_sha(engine, h[..]) && cast(string) h[..] == want;
}
```

A file's name is the hash of its bytes before `deploy.sh` strips the
comments from scripts and shaders, so the bytes served for `host.*.js` and
`*.wgsl` hash to something else (the wasm and packs match their names). After
the strip, `coax/web/tool.sh engine-served write SITE`
([engine-served.coax](../../coax/web/engine-served.coax)) adds the served
bytes' own hashes to the engine's script, `data-served="SHA256=PATH ..."`,
so the manifest a publish pins carries them and its hash covers them.
`engine-served check MANIFEST SITE` holds a pinned record to a site's files:
every path the script names listed once, each file's bytes hashing to what
the record says.

### The route, checked end to end

`coax/web/tool.sh coax/web/api/tests/realm_walkthrough.coax` (`PGURL`, `PSQL`,
`WORK`) runs Aethyrforge's route against PostgreSQL with every migration
applied, and works out again from the bytes every digest the server states:

1. A release is staged (its five engine files, `/realms/built`), and
   `engine-served` writes the served hashes.
2. The realm is read into the edit model; a move, a turn, a resize, a removal
   and an undone move are one save (revision 2).
3. Publish pins revision 2 and the engine; the engine hash it states is the
   manifest's, computed here.
4. Reload from the record: the manifest holds by the rule and hashes to the
   record's engine hash; every served file is the bytes the record says; a
   visitor's document hashes here to the record's document hash and reads
   back ([doc.coax](../forge/doc.coax)) as exactly the moves, turns, sizes and
   removals the edits made; the realm's page plays the pinned engine.
5. A second release with a new engine: the record and the realm's page still
   name the first.
6. The owner saves revision 3 unpublished: a visitor's document is revision
   2's, byte for byte. A visitor's fork starts on the parent's engine and its
   published moves and shapes; the fork's edits leave the parent's published
   and current documents untouched.
7. Refusals keep the last valid document: a stale save (409), a publish whose
   engine can't be pinned (503, revision 2 stays published), and a newer
   document version the reader doesn't know.

It ran on a fresh PostgreSQL 17 with this checkout's migrations: 24 checks.

What the pin does not cover yet: the API is shared, so a pinned page talks
to today's server (as a pinned demo does, `coax/web/pin.sh`); a realm pinned
before served hashes were written (an older `/realms/built`) has names only,
and `engine-served check` says so; built-in realms' forks are not pinned; the
walkthrough stages its own site instead of reading the production bucket.

Checks: `coax/bin/coax test aethyr/forge/publish.coax --library` (the rule,
the live page's fixture and its hash, which `sha256sum` agrees with; `coax
mutate` with no survivors), `coax/web/tool.sh
coax/web/api/tests/realm_engine_pin.coax` (the same fixture through the SQL
rule gives the same hash; first pin, publish, refused publishes keeping
nothing, a fork on its parent's engine, a server without a bucket),
`coax/bin/coax test coax/web/engine-served.coax --library`, the walkthrough
above,
`test_realm_fork.py` (a save leaves visitors on the revision
published, their `/doc` is its document, a visitor's fork copies it) and
`coax/web/tool.sh forge-journey URL` (after SAVE the owner reads "not
published" and a visitor's spec has the published revision; after PUBLISH,
"live", and the visitor gets it).

## What it builds

- `aethyr/forge/publish.coax`: the engine manifest and the record's check
  exist; the checks a version must pass before it replaces the last (golden,
  frame times) and the page text's hash do not yet.
- The realm table's rules (unique slots, required `index.md`, style and
  clock fields) as Coax checks shared by `realms.ts` and the made-realm
  path.

## Acceptance checks and oracles

- `bun coax/web/realms.ts --check` and `realms.test.ts` pass; a table entry
  without `index.md`, with a duplicate slot or an open world without a realm
  is refused.
- Every published realm's golden passes on the release walk; a page whose
  look changed without its golden is named with the commit that changed it.
- The 60 FPS rule (95th percentile frame at most 17.5 ms, none over 33 ms)
  holds for each realm's recorded walk.
- A release run against a bucket with made realms leaves every made realm's
  files byte-identical.
- A publish record's hashes match the served files; a mismatch refuses.

## Dependencies

[Chapter 16](ch016-budgets-that-refuse.md) (identities),
[chapter 37](ch037-measure-the-engine-honestly.md) (measurement),
[chapter 51](ch051-a-realm-is-a-document.md) and
[chapter 53](ch053-bring-in-assets.md).
