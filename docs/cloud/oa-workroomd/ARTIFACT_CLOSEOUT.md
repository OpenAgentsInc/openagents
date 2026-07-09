# oa-workroomd Artifact Closeout

Status: Cloud MVP scaffold for `CND-016`

`oa-workroomd artifacts` models declared workroom outputs and closeout
manifests. The MVP stores artifact policy, content-addressed objects, receipts,
and the final manifest under the workroom state directory.

```bash
oa-workroomd artifacts policy init \
  --required transcript \
  --required summary \
  --json
oa-workroomd artifacts upload --name transcript --file ./transcript.txt --json
oa-workroomd artifacts status --json
oa-workroomd closeout submit --json
```

The state files are:

```text
artifact-state.json
artifact-receipts.jsonl
closeout-manifest.json
artifacts/sha256/<digest>
```

Uploads are content-addressed by `sha256:` digest. The object path is derived
from the digest, and the upload receipt records the artifact name, content
digest, and receipt digest. Uploading the same bytes under multiple declared
names points at the same content object.

Closeout fails closed when required artifacts are missing, when no required
artifact policy exists, or when a required artifact has no matching upload
receipt. A successful closeout writes a manifest with required artifact names,
artifact digests, upload receipt digests, status, and a manifest digest. A
`closeout_submitted` receipt cites the manifest digest so Forge can verify the
manifest against the upload receipts and content digests.

Artifact names are bounded names rather than paths. Names, receipts, and stored
metadata are validated against the shared raw secret, token, wallet, private
key, and private-topology marker filter.
