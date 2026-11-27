# 21. A knowledge base

> **Status: partly implemented.** The `#authority` `openagents.knowledge.v1alpha1`
> record codec ([`openagents/knowledge/record.coax`](../../openagents/knowledge/record.coax),
> `#14058`), signed BIP-340 NIP-KB event ingestion bridge
> ([`openagents/knowledge/nipkb.coax`](../../openagents/knowledge/nipkb.coax)),
> workspace corpus indexer ([`openagents/knowledge/index.coax`](../../openagents/knowledge/index.coax)),
> and CLI/eval harness ([`openagents/knowledge/kb-tool.coax`](../../openagents/knowledge/kb-tool.coax))
> run on `cpu`, grounded by [`openagents/examples/knowledge-base-records.coax`](../examples/knowledge-base-records.coax)
> and [`openagents/examples/knowledge-base-records.out`](../examples/knowledge-base-records.out),
> and are mounted in [`openagents/runtime/oa-worker.coax`](../../openagents/runtime/oa-worker.coax)
> as `OA_PACK_KNOWLEDGE` (`KnowledgeSearch`, `KnowledgeGet`). Remaining: live
> multi-relay NIP-KB sync (`openagents/knowledge/relay-sync.coax`) and evidence-gated
> import of legacy benchmark method notes.

## Premise

Prepared answers cover the common questions. This chapter adds the store
behind everything else the agent knows: entries anyone can read, each signed,
versioned and tied to its source.

## Draft

[Episode 289](../../docs/transcripts/289.md) opens its demo with this: ask
about the knowledge base, and the whole of it is loaded. Promise P2 is a
shared knowledge base the agent answers from.

The material exists. The repository holds seven books, the protocol documents,
and transcripts of 290 episodes. In [Episode
290](../../docs/transcripts/290.md) the owner calls that archive one of the
project's main assets, because an agent can go back through three years of
positions and reasoning.

A knowledge entry is a small signed record: a statement, its source, a version
and a head that points at the current version. Anyone with a key can publish
one. The same store holds two kinds of entry. Some are facts about OpenAgents,
drawn from the documents. Others are method notes of the kind the old tree
kept: how to do a task, an edge case to watch for, a slip that made a check
pass when it should not have.

An entry is not trusted because it exists. The evidence rule of chapter 43 applies to knowledge
too. A knowledge entry is a candidate until a measurement shows that giving it
to the agent helps on some declared kind of question. The old tree's notes
were written during benchmark work, so they need that measurement on new tasks
before the agent leans on them.

This is the oldest thread in the archive. The first nineteen episodes are
about chatting with documents.

## What we build

A plan for what this chapter ships once the draft is approved:

- A knowledge store with signed entries, versions and heads.
- An importer that turns documents, book chapters and transcripts into entries
  that cite their source.
- A knowledge member that answers from entries and shows which ones it used.

It builds on what exists today:

- [`openagents/knowledge/record.coax`](../../openagents/knowledge/record.coax) (`#authority` `openagents.knowledge.v1alpha1` record codec, NIP-KB ID grammar, SHA-256 content digests, and YAML frontmatter, `#14058`)
- [`openagents/knowledge/nipkb.coax`](../../openagents/knowledge/nipkb.coax) (BIP-340 signed NIP-KB kinds `3190`/`30190`/`3191` verification, head/withdrawal/equivocation resolution, and reader trust policy)
- [`openagents/knowledge/index.coax`](../../openagents/knowledge/index.coax) and [`openagents/knowledge/kb-tool.coax`](../../openagents/knowledge/kb-tool.coax) (workspace corpus scanner, chunker, disk cache, and `KnowledgeGet`)
- [`coax/lib/nostr/kb`](../../coax/lib/nostr/kb)
- [`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Knowledge entry, head and withdrawal | `openagents.knowledge.v1` `Entry`, `Head`, `Withdrawal` ([spec 20.4](../../docs/protocol/openagents-protocol-spec.md#204-openagentsknowledgev1)) | Nostr kinds 3190, 30190 and 3191, public (NIP-KB) | Spec sketch; no `.proto` file. `coax/lib/nostr/kb` handles the NIP's JSON |

### The chapter's check

- **Source:** [`openagents/knowledge/record.coax`](../../openagents/knowledge/record.coax),
  [`openagents/knowledge/nipkb.coax`](../../openagents/knowledge/nipkb.coax),
  [`openagents/knowledge/index.coax`](../../openagents/knowledge/index.coax),
  [`openagents/knowledge/kb-tool.coax`](../../openagents/knowledge/kb-tool.coax),
  [`openagents/examples/knowledge-base-records.coax`](../examples/knowledge-base-records.coax),
  and [`openagents/examples/knowledge-base-records.out`](../examples/knowledge-base-records.out).
- **Trace:** derives the NIP-KB entry ID `docs.transcripts.289` (`kind=transcript`),
  computes its SHA-256 digest and YAML frontmatter (`97` bytes), ingests signed
  BIP-340 NIP-KB events (`v1`, `v2`, `head2`) alongside two tampered events,
  resolves `statistics.mmd-estimators` version `2` (`trust=admitted`, digest
  `92b7aeb9a48bdc3a`), and emits a round-trip-verified `openagents.knowledge.v1alpha1`
  JSON reply.
- **Refusal:** invalid NIP-KB IDs (`Bad_ID`) and version `0` are refused (`0`
  bytes); tampered signatures and tampered documents are refused (`refused=2`);
  untrusted authors are downgraded to `candidate`; a withdrawal (`kind 3191`)
  excludes `v2` so `v1` becomes current; equivocating `v2` events resolve `0`
  entries; and author-supplied vector fields are refused (`author_vec_refused=1`).
- **Independent check:** `coax/bin/coax run openagents/examples/knowledge-base-records.coax`
  and `--release` match [`openagents/examples/knowledge-base-records.out`](../examples/knowledge-base-records.out);
  `kb_rec_hash_hex("abc")` matches the published NIST SHA-256 vector
  `ba7816bf8f01cfea...`; and `openagents/knowledge/record.coax` (`#authority`),
  `openagents/knowledge/nipkb.coax`, and `coax/lib/nostr/kb/current.coax` pass
  their `#test` and mutation checks.

Shipped paths: [`openagents/knowledge/record.coax`](../../openagents/knowledge/record.coax), [`openagents/knowledge/nipkb.coax`](../../openagents/knowledge/nipkb.coax), [`openagents/knowledge/index.coax`](../../openagents/knowledge/index.coax), [`openagents/knowledge/kb-tool.coax`](../../openagents/knowledge/kb-tool.coax) (`#14058`).

Planned paths: `openagents/knowledge/relay-sync.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, the knowledge base
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 13:00, the transcripts as an asset
  - [Episode 11, *Chatting with a PDF*](../../docs/transcripts/011.md)
  - [Episode 19, *Chat with PDF, OpenAgents Edition*](../../docs/transcripts/019.md): document chat on our own stack
  - [Episode 40, *Agent Brain Design*](../../docs/transcripts/040.md): the knowledge-base model
  - [Episode 155, *Open Knowledge Graph*](../../docs/transcripts/155.md): an open knowledge graph with contributor incentives
  - [Episode 286, *System One in Coding Agents*](../../docs/transcripts/286.md): reusable knowledge with evidence
- **Old code at `coder-v1.0.0`, crates:** `knowledge`, `memory-stream`
- **NIPs:** [NIP-KB](../../nostr/nips/openagents/NIP-KB.md)
- **Also:**
  - `knowledge/` at `coder-v1.0.0`: about 150 method, tool, edge-case and slip notes
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P2

## Open questions for the owner

1. Which of the old tree's notes come back? Many were learned from benchmark
   tasks and need new evidence.
2. Is every entry about OpenAgents public, or are some for logged-in people
   only?

## Source listing

Run the Chapter 21 example in [`openagents/examples/knowledge-base-records.coax`](../examples/knowledge-base-records.coax) (committed transcript [`openagents/examples/knowledge-base-records.out`](../examples/knowledge-base-records.out)):

<!-- source: openagents/examples/knowledge-base-records.coax knowledge-base-records -->
```jai
kbr_run :: () -> bool {
  // 1. Deterministic NIP-KB ID derivation, source classification, SHA-256, and frontmatter.
  idb: [128] u8;
  id_n := kb_rec_path_to_id("docs/transcripts/289.md", idb[..]);
  eid := cast(string) idb[0..<id_n];
  kind := kb_rec_classify_source("docs/transcripts/289.md");
  hb: [64] u8;
  _ = kb_rec_hash_hex("abc", hb[..]);
  sha := cast(string) hb[0..<64];

  fm: [256] u8;
  fm_ok := kb_rec_write_nipkb_frontmatter(fm[..], eid, 1, kind, "Episode 289", KB_TRUST_ADMITTED);
  fm_bad_id := kb_rec_write_nipkb_frontmatter(fm[..], "Bad_ID", 1, KB_KIND_METHOD, "T", KB_TRUST_ADMITTED);
  fm_bad_v := kb_rec_write_nipkb_frontmatter(fm[..], eid, 0, KB_KIND_METHOD, "T", KB_TRUST_ADMITTED);
  print("derived id=% kind=% valid=% sha256=% fm_bytes=% refuse_id=% refuse_ver=%\n",
    eid, kb_rec_kind_name(kind), kb_rec_id_valid(eid), sha[0..<16], fm_ok, fm_bad_id == 0, fm_bad_v == 0);

  // 2. Signed BIP-340 NIP-KB ingestion: v1 + v2 + head2 + tampered sig + tampered doc.
  st: [1] KbNipkbStore;
  trusted := [1] string.[KBV_AUTHOR];
  evs := [5] string.[KBV_E1, KBV_E2, KBV_HEAD2, KBV_TAMPERED_SIGNATURE, KBV_TAMPERED_DOCUMENT];
  n1 := kb_nipkb_ingest(st[..], evs[..], KBV_OTHER, trusted[..]);
  e0 := st[0].entries[0];
  v2_ver := e0.version;
  v2_trust := e0.trust_status;
  v2_match := e0.digest == KBV_DOC2_SHA;
  print("ingest admitted=% refused=% resolved=% id=% version=% kind=% trust=% digest=%\n",
    st[0].admitted, st[0].refused, n1, e0.entry_id, v2_ver,
    kb_rec_kind_name(e0.kind), kb_rec_trust_name(v2_trust), e0.digest[0..<16]);

  // Emit and round-trip parse versioned `openagents.knowledge.v1alpha1` JSON reply.
  hit := kb_nipkb_to_hit(e0, 31, 2500, 820);
  hits := [1] KbRecHit.[hit];
  jb: [4096] u8;
  o: [1] Jo;
  jo_start(o[..], jb[..]);
  kb_rec_write_reply_json(o[..], "mmd estimators", 4, 1, 1, hits[..]);
  nodes: [256] JvNode;
  d: [1] Jv;
  sb: [64] u8;
  json_ok := !o[0].bad && jv_parse(d[..], jo_text(o[..]), nodes[..]) &&
             jv_str(d[0], jv_get(d[0], 0, "schema"), sb[..]) == KB_V1ALPHA1_SCHEMA.count &&
             cast(string) sb[0..<KB_V1ALPHA1_SCHEMA.count] == KB_V1ALPHA1_SCHEMA &&
             !jv_bool(d[0], jv_get(d[0], 0, "refused"));
  print("wire_schema=% json_ok=%\n", KB_V1ALPHA1_SCHEMA, json_ok);

  // 3. Untrusted author downgrade, withdrawal fallback to v1, equivocation & author-vector refusal.
  no_trust: [1] string;
  n_untrust := kb_nipkb_ingest(st[..], evs[0..<2], KBV_OTHER, no_trust[0..<0]);
  untrust_status := kb_rec_trust_name(st[0].entries[0].trust_status);

  evs_w := [4] string.[KBV_E1, KBV_E2, KBV_HEAD2, KBV_WITHDRAW2];
  n_w := kb_nipkb_ingest(st[..], evs_w[..], KBV_AUTHOR, trusted[..]);
  w_ver := st[0].entries[0].version;
  w_match := st[0].entries[0].digest == KBV_DOC1_SHA;

  evs_eq := [2] string.[KBV_E2, KBV_EQUIVOCATION];
  n_eq := kb_nipkb_ingest(st[..], evs_eq[..], KBV_AUTHOR, trusted[..]);

  evs_vec := [1] string.[KBV_VECTOR_FIELD];
  n_vec := kb_nipkb_ingest(st[..], evs_vec[..], KBV_AUTHOR, trusted[..]);
  vec_ref := st[0].refused;

  print("untrusted=% withdrawn_ver=% equivocation_resolved=% author_vec_refused=%\n",
    untrust_status, w_ver, n_eq, vec_ref);

  return eid == "docs.transcripts.289" && fm_ok > 0 && fm_bad_id == 0 && fm_bad_v == 0 &&
         n1 == 1 && v2_ver == 2 && v2_trust == KB_TRUST_ADMITTED && v2_match &&
         json_ok && n_untrust == 1 && untrust_status == "candidate" &&
         n_w == 1 && w_ver == 1 && w_match && n_eq == 0 && n_vec == 0 && vec_ref == 1;
}
```

Output (`coax/bin/coax run openagents/examples/knowledge-base-records.coax`, identical in `--release`):

```text
derived id=docs.transcripts.289 kind=transcript valid=true sha256=ba7816bf8f01cfea fm_bytes=97 refuse_id=true refuse_ver=true
ingest admitted=3 refused=2 resolved=1 id=statistics.mmd-estimators version=2 kind=method trust=admitted digest=92b7aeb9a48bdc3a
wire_schema=openagents.knowledge.v1alpha1 json_ok=true
untrusted=candidate withdrawn_ver=1 equivocation_resolved=0 author_vec_refused=1
ok=true
```
