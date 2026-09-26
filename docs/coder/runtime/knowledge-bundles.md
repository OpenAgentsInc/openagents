# Pinned knowledge snapshots and private delivery

The knowledge library can create and verify immutable NIP-EXT guidance
snapshots and encrypted private-entry bundles. Microcoder can load either
as an explicit run input. These operations do not establish useful transfer,
a live artifact-storage service, remote revocation freshness, or automatic
entry admission. [Knowledge evidence](knowledge-evidence.md) remains a
separate measurement contract.

## Curated snapshots

```sh
microcoder kb snapshot-create statistics.mmd-estimators \
  --package reviewed-methods --snapshot-version 1 --license CC0-1.0 \
  --output reviewed-methods-v1.json
microcoder kb snapshot-check reviewed-methods-v1.json
```

Use the actual rights of the selected entries; `--license` records the
publisher's assertion and does not establish ownership. Creation accepts
explicitly chosen admitted entries, or all entries in `--dir` when no IDs
are named. It refuses candidate and withdrawn entries.

The bundle holds an original signed EXT `3184` release, its exact manifest,
and every listed Markdown file. Each entry is an inert `guidance` component
whose `openagents.kb-entry.v1` artifact reference pins the raw document.
The manifest preserves the files, version, license, and curation provenance.
The original document preserves its author label, citations, source runs,
evidence, and digest. The release signer identifies the curator; a label
inside an entry is not independently authenticated authorship.

Loading checks signatures, package/version agreement, the manifest digest,
every component and file, normalized paths, unique identities, and bounded
byte sizes. This initial loader refuses dependencies, executable components,
archives, descriptors, missing files, and unlisted content. It does not run
scripts, probes, models, or external fetches. It writes a new private bundle
atomically and refuses to overwrite an existing output path. A release ID
is the pin; a human version label is not the pin.

```sh
microcoder TASK --kb candidates --kb-snapshot reviewed-methods-v1.json
```

This selection replaces ambient local and cached relay knowledge. Entries
load as candidates even if the curator declared them admitted. The explicit
`--kb candidates` switch permits their use in this run; it does not promote
them in the operator's registry. The run retains the full bundle in
`knowledge-input.json` and records the release, manifest digest, and exact
entry inventory. `--kb-cache FILE` selects a separate embedding cache for
controlled studies. `--kb-lexical` disables embedding calls explicitly; it
does not disable Jev or generation. The started record and summary retain
the effective retrieval mode and the configured decision endpoint and model.
These model labels describe requested configuration, not an independently
verified provider artifact.

The loader uses an explicit offline release pin. It does not discover the
latest release, prove absence of unseen revocations, or implement the full
EXT installer/update service. A later file cannot replace a recorded run's
bytes. Changes require a new signed release and explicit selection.

## Private entry delivery

```sh
microcoder kb private-seal ENTRY_ID --recipient RECIPIENT_PUBLIC_KEY \
  --retain-until UNIX_SECONDS --output private-entry.json
```

The sender uses the existing knowledge signing key, or `--key-file FILE`.
One bundle has exactly one recipient. Send that encrypted file through an
explicitly selected channel. This command does not publish to a relay, and
ordinary `kb sync` does not fetch its document storage.

The bundle contains two encrypted pieces:

1. An original NIP-44 `3188` declaration with the shared artifact envelope.
   Its artifact has schema `openagents.kb-entry.v1`, media type
   `text/markdown`, and the exact raw Markdown digest and size.
2. The document encrypted separately with independent fresh nonce entropy.
   The receiver authenticates the original declaration, decrypts the
   separately stored document, and checks its exact bytes against that
   reference.

This follows the shared envelope's `inline: null` rule for raw non-JSON
artifacts. It does not disguise Markdown as a JSON artifact or give inert
metadata authority. The file bundle is the supported local artifact-storage
transport; a network blob store remains separate work. Mailboxes are random,
never derived from private text, paths, or digests. Visible recipient and
traffic metadata remains visible. The requested retention instant does not
prove storage availability or revoke an already disclosed copy.

The recipient can inspect a selected delivery locally:

```sh
microcoder kb private-show private-entry.json --key-file RECIPIENT_KEY_FILE
```

The key file is never an output. The command prints the document because
local inspection was explicitly requested; treat terminal capture as a
private record. Neither decryption nor the author's admitted status grants
model disclosure. Private entries load as candidates.

## Explicit model disclosure

Create a local operator permission before using a private bundle with models:

```sh
microcoder kb private-grant private-entry.json \
  --key-file RECIPIENT_KEY_FILE \
  --model-recipient 'codex:GENERATION_BASE_URL#MODEL' \
  --model-recipient 'typesafe:JEV_BASE_URL#JEV_MODEL' \
  --output private-entry-model-grant.json
```

Replace each label with the actual configured client base URL and normalized
model. The default Codex route uses `codex:` and
`https://chatgpt.com/backend-api/codex`; `--provider openrouter` uses
`openrouter:` and that client's configured base URL. Codex removes any provider
prefix from the model; OpenRouter supplies `openai/` when the supplied model
has no prefix.
If the stronger generator can run, add its exact recipient label too.
Microcoder computes the required labels from its constructed clients and
configuration; a changed endpoint or model refuses before any inference.
Wildcard grants are unsupported. The local permission pins the original
3188 event, signer, and exact document digest. A fresh envelope requires a
fresh permission even when its document bytes are identical.

```sh
microcoder TASK --kb candidates --kb-private private-entry.json \
  --kb-key-file RECIPIENT_KEY_FILE \
  --kb-private-grant private-entry-model-grant.json
```

Private input replaces ambient knowledge, and private mode disables
embeddings. Jev and generation can receive the entry only after every
required model recipient matches. The original encrypted bundle and local
permission are retained with the run; they do not become public NIP-KB
`3190` events. The run directory is private on supported Unix hosts. Its
model prompts, responses, and other evidence can contain private knowledge;
do not republish them without separate disclosure authority.

The permission file is a local operator decision protected by the local
filesystem, not a signed portable POL grant, sender-issued reuse license,
or automatic permission to train, redistribute, or publish. Removing the
permission file prevents a later load; it does not interrupt an already
admitted run or erase its evidence. This initial
feature has no private remote withdrawal/head protocol. Revocation cannot
erase retained copies, and a sender's signature establishes attribution,
not quality. Controlled admission and contribution rights remain separate.

## Verification boundary

The local fixtures cover exact signed snapshot round trips, source provenance,
immutable private output files, duplicate-key JSON, signature/file tampering,
missing or extra files, traversal, unsupported executable components and
dependencies, encrypted sender/recipient round trips, unrelated readers,
mismatched document bytes, and missing or changed model-disclosure permissions.
Microcoder fixtures verify selected inputs replace ambient sources and retain
original pins without model calls.

These fixtures exercise actual library and client loading behavior. They do
not establish a deployed relay artifact store, provider behavior, hosted
policy enforcement, paid task outcomes, or positive out-of-sample transfer.
