# Clef M3/M4 validation bundle (2026-10-11)

The full evidence bundle from PR #11299 (3,104 files, about 48 MB) is stored
outside the repository:

- `gs://openagents-bench-artifacts/clef/m3-m4-2026-10-11/`

`MANIFEST.json` lists every file with its relative path, size, and SHA-256.
The same files are browsable at the PR's final source commit:
<https://github.com/OpenAgentsInc/openagents/tree/46ed6cdcf3fdb8b6d9db59a8e058745670d63f4f/crates/psionic/fixtures/clef/reports/m3-m4-2026-10-11>

Fetch and check the bundle (needs read access to the bucket):

```sh
gcloud storage rsync -r gs://openagents-bench-artifacts/clef/m3-m4-2026-10-11 ./m3-m4-2026-10-11
python3 -c 'import hashlib,json,sys
m=json.load(open(sys.argv[1]))
bad=[f["path"] for f in m["files"] if hashlib.sha256(open(sys.argv[2]+"/"+f["path"],"rb").read()).hexdigest()!=f["sha256"]]
print(len(m["files"])-len(bad),"ok",len(bad),"bad")' MANIFEST.json ./m3-m4-2026-10-11
```

The bundle keeps model identities, reference answers and logits, test
receipts, failed attempts, and the CUDA admission check. Results are summarized
in [`docs/inference/clef-native.md`](../../../../../../docs/inference/clef-native.md).
