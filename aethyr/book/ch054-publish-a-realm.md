# 54. Publish a realm

> **Status: partly implemented.** Visitors of a made realm see the revision
> its owner published, not the latest save, and the owner reads which
> version is live. The frozen publish record below (document, module build
> and assets by hash, gated by checks) is not built: what is published is a
> document revision, played by the current engine build. Hand-built realms
> are published from one table (`apps/openagents-web/realms.json`).

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
separate checks. A published made realm's page plays the published
document with today's engine; pinning the module build and assets by hash is
still to do.

Checks: `test_realm_fork.py` (a save leaves visitors on the revision
published, their `/doc` is its document, a visitor's fork copies it) and
`coax/web/tool.sh forge-journey URL` (after SAVE the owner reads "not
published" and a visitor's spec has the published revision; after PUBLISH,
"live", and the visitor gets it).

## What it builds

- `aethyr/forge/publish.coax`: `Published`, its encoding, and the checks a
  version must pass before it replaces the last.
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
