# Private assets

Licensed source assets and their derivatives stay in owner-local packs
under `~/.openagents/`. This includes Epic sample content from Valley of
the Ancient and Slay, MetaHuman data, exported meshes and motion, face
tracks, source screenshots and rendered comparisons.

Do not put those payloads in git, public test fixtures, logs, AI training
data or datasets. Test fixtures are small files we authored. Verification
records can contain counts, timings and SHA-256 hashes without embedding
the licensed content.

## Storage and identity

Use `~/.openagents/aethyrforge/animation-lab/` for Animation Lab's private
imports, exports, receipts and review captures. Existing Valley Lab and
MetaHuman packs keep their current paths. Do not overwrite another task's
pack or change a shared private-build symlink while testing.

Documents reference binary parts by SHA-256 and range. Verify the complete
pack before admitting a document and retain the verified bytes for the
whole playback lifetime. A content hash proves identity; it does not prove
ownership, permission to publish or permission to train a model.

The browser's local `/private/` route resolves files from
`~/.openagents/verse/private-build/web/`. It is available only on local
development hosts. A production build must not include private packs or
captures. Pushing source code does not authorize a production deployment.

## Original assets

Our own extras, props, keyed motion and faces also start in private packs.
Keep an import or authoring receipt identifying their creator, inputs and
hashes. Move an original asset into a public pack only after checking every
source and recording that the asset is cleared for publication. This keeps
the publication decision separate from the renderer's ability to load it.

For the current MetaHuman pipeline, see
[MetaHuman support](metahuman-support.md). For animation document admission,
see [Animation documents v1](anim-documents-v1.md).
